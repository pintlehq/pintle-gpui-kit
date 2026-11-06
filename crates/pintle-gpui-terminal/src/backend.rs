//! Blocking platform work lives here; no GPUI objects cross into these threads.
use crate::{TerminalExitStatus, TerminalOptions, TerminalPalette, TerminalStatus};
use alacritty_terminal::{
    Term,
    event::{Event, EventListener, WindowSize},
    grid::{Dimensions, Scroll},
    term::{Config, Osc52},
    vte::ansi::{Processor, Rgb},
};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::{
    io::{Read, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

pub(crate) const MAX_INPUT: usize = 1024 * 1024;
const COMMAND_CAPACITY: usize = 32;

#[derive(Clone)]
pub(crate) struct Listener(Arc<Mutex<Vec<Event>>>);
impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        // Drained after each 8 KiB parse; never a growing history of events.
        if matches!(
            event,
            Event::PtyWrite(_)
                | Event::Title(_)
                | Event::ResetTitle
                | Event::ColorRequest(..)
                | Event::TextAreaSizeRequest(_)
        ) {
            self.0.lock().unwrap().push(event);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub rows: u16,
    pub cols: u16,
    pub cell_width: u16,
    pub cell_height: u16,
}
impl Default for Size {
    fn default() -> Self {
        Self {
            rows: 24,
            cols: 80,
            cell_width: 8,
            cell_height: 20,
        }
    }
}
impl Dimensions for Size {
    fn columns(&self) -> usize {
        self.cols as usize
    }
    fn screen_lines(&self) -> usize {
        self.rows as usize
    }
    fn total_lines(&self) -> usize {
        self.rows as usize
    }
}
impl Size {
    fn pty(self) -> PtySize {
        PtySize {
            rows: self.rows,
            cols: self.cols,
            pixel_width: self.cell_width,
            pixel_height: self.cell_height,
        }
    }
}

pub(crate) struct Model {
    pub term: Term<Listener>,
    parser: Processor,
    events: Arc<Mutex<Vec<Event>>>,
    pub status: TerminalStatus,
    pub exit_status: Option<TerminalExitStatus>,
    pub title: String,
    pub size: Size,
    pub palette: TerminalPalette,
}
impl Model {
    pub fn new(options: &TerminalOptions, size: Size) -> Self {
        let events = Arc::new(Mutex::new(Vec::new()));
        let config = Config {
            scrolling_history: options.scrollback_lines,
            osc52: Osc52::Disabled,
            ..Default::default()
        };
        Self {
            term: Term::new(config, &size, Listener(events.clone())),
            parser: Processor::new(),
            events,
            status: TerminalStatus::Starting,
            exit_status: None,
            title: String::new(),
            size,
            palette: TerminalPalette::dark(),
        }
    }
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        self.parser.advance(&mut self.term, bytes);
        let events = std::mem::take(&mut *self.events.lock().unwrap());
        let mut replies = Vec::new();
        for event in events {
            match event {
                Event::PtyWrite(text) => replies.push(text.into_bytes()),
                Event::Title(title) => self.title = title.chars().take(256).collect(),
                Event::ResetTitle => self.title.clear(),
                Event::ColorRequest(index, format) => {
                    let color =
                        self.term.colors()[index].unwrap_or_else(|| self.palette.indexed(index));
                    replies.push(format(color).into_bytes());
                }
                Event::TextAreaSizeRequest(format) => replies.push(
                    format(WindowSize {
                        num_lines: self.size.rows,
                        num_cols: self.size.cols,
                        cell_width: self.size.cell_width,
                        cell_height: self.size.cell_height,
                    })
                    .into_bytes(),
                ),
                _ => {}
            }
        }
        replies
    }
    pub fn resize(&mut self, size: Size) {
        self.size = size;
        self.term.resize(size);
    }
    pub fn text(&self) -> String {
        self.term.bounds_to_string(
            alacritty_terminal::index::Point::new(
                self.term.topmost_line(),
                alacritty_terminal::index::Column(0),
            ),
            alacritty_terminal::index::Point::new(
                self.term.bottommost_line(),
                self.term.last_column(),
            ),
        )
    }
}

pub(crate) enum Command {
    Input(Vec<u8>),
    Resize(Size),
}

pub(crate) struct Backend {
    pub model: Arc<Mutex<Model>>,
    commands: mpsc::SyncSender<Command>,
    stopped: Arc<AtomicBool>,
    shutdown: mpsc::SyncSender<()>,
    pub done: async_channel::Receiver<()>,
    #[cfg(feature = "test-support")]
    capture: Option<Arc<Mutex<Vec<Vec<u8>>>>>,
}
impl Backend {
    pub fn start(options: TerminalOptions, size: Size, wake: async_channel::Sender<()>) -> Self {
        let (done_tx, done) = async_channel::bounded::<()>(1);
        let model = Arc::new(Mutex::new(Model::new(&options, size)));
        let stopped = Arc::new(AtomicBool::new(false));
        let (commands, receiver) = mpsc::sync_channel(COMMAND_CAPACITY);
        let (shutdown, shutdown_rx) = mpsc::sync_channel(1);
        let state = model.clone();
        let stop = stopped.clone();
        let replies = commands.clone();
        thread::spawn(move || {
            let _done = done_tx;
            let result = run(
                options,
                &state,
                &stop,
                receiver,
                replies,
                shutdown_rx,
                &wake,
            );
            if let Err(error) = result {
                state.lock().unwrap().status = if stop.load(Ordering::Acquire) {
                    TerminalStatus::Closed
                } else {
                    TerminalStatus::Failed(error)
                };
            }
            let _ = wake.try_send(());
        });
        Self {
            model,
            commands,
            stopped,
            shutdown,
            done,
            #[cfg(feature = "test-support")]
            capture: None,
        }
    }
    #[cfg(feature = "test-support")]
    pub fn mock(options: &TerminalOptions, size: Size) -> Self {
        let (commands, _) = mpsc::sync_channel(COMMAND_CAPACITY);
        let (shutdown, _) = mpsc::sync_channel(1);
        let (_, done) = async_channel::bounded::<()>(1);
        let mut model = Model::new(options, size);
        model.status = TerminalStatus::Running;
        Self {
            model: Arc::new(Mutex::new(model)),
            commands,
            stopped: Arc::new(AtomicBool::new(false)),
            shutdown,
            done,
            capture: Some(Arc::new(Mutex::new(Vec::new()))),
        }
    }
    pub fn is_mock(&self) -> bool {
        #[cfg(feature = "test-support")]
        {
            self.capture.is_some()
        }
        #[cfg(not(feature = "test-support"))]
        {
            false
        }
    }
    #[cfg(feature = "test-support")]
    pub fn take_input(&self) -> Vec<u8> {
        self.capture
            .as_ref()
            .map(|c| std::mem::take(&mut *c.lock().unwrap()).concat())
            .unwrap_or_default()
    }
    pub fn send(&self, command: Command) -> Result<(), String> {
        if self.stopped.load(Ordering::Acquire) {
            return Err("Terminal is closed".into());
        }
        #[cfg(feature = "test-support")]
        if let Some(capture) = &self.capture {
            match command {
                Command::Input(bytes) => capture.lock().unwrap().push(bytes),
                Command::Resize(size) => self.model.lock().unwrap().resize(size),
            }
            return Ok(());
        }
        self.commands
            .try_send(command)
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => {
                    "Terminal input queue is full; wait before trying again".into()
                }
                mpsc::TrySendError::Disconnected(_) => {
                    "Terminal process is no longer running".into()
                }
            })
    }
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        let _ = self.shutdown.try_send(());
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        self.stop();
    }
}

fn command(options: &TerminalOptions) -> CommandBuilder {
    let program = options.program.clone().unwrap_or_else(|| {
        #[cfg(windows)]
        {
            std::env::var_os("COMSPEC")
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "cmd.exe".into())
        }
        #[cfg(not(windows))]
        {
            std::env::var_os("SHELL")
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "/bin/sh".into())
        }
    });
    let mut command = CommandBuilder::new(program);
    if options.program.is_none() && options.args.is_empty() {
        #[cfg(unix)]
        command.arg("-i");
    }
    command.args(&options.args);
    if let Some(cwd) = &options.cwd {
        command.cwd(cwd);
    }
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    for (key, value) in &options.env {
        command.env(key, value);
    }
    command
}

fn run(
    options: TerminalOptions,
    model: &Arc<Mutex<Model>>,
    stopped: &Arc<AtomicBool>,
    commands: mpsc::Receiver<Command>,
    replies: mpsc::SyncSender<Command>,
    shutdown: mpsc::Receiver<()>,
    wake: &async_channel::Sender<()>,
) -> Result<(), String> {
    if stopped.load(Ordering::Acquire) {
        model.lock().unwrap().status = TerminalStatus::Closed;
        return Ok(());
    }
    let pair = native_pty_system()
        .openpty(model.lock().unwrap().size.pty())
        .map_err(|e| format!("Open PTY: {e}"))?;
    let mut child = ChildGuard(
        pair.slave
            .spawn_command(command(&options))
            .map_err(|e| format!("Start shell: {e}"))?,
    );
    drop(pair.slave);
    let mut reader = Reader::new(pair.master.as_ref()).map_err(|e| format!("Read PTY: {e}"))?;
    let mut writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("Write PTY: {e}"))?;
    let mut killer = child.0.clone_killer();
    #[cfg(unix)]
    let shell_pid = child.0.process_id();
    let master = Arc::new(Mutex::new(Some(pair.master)));
    let cancel_master = master.clone();
    let stop = stopped.clone();
    // Separate cancellation can interrupt a blocked write_all (e.g. paste into sleep).
    let (finished, finish_rx) = mpsc::sync_channel(1);
    let cancellation = thread::spawn(move || {
        loop {
            if finish_rx.try_recv().is_ok() {
                break;
            }
            match shutdown.recv_timeout(Duration::from_millis(20)) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    stop.store(true, Ordering::Release);
                    #[cfg(unix)]
                    {
                        use nix::{
                            sys::signal::{Signal, killpg},
                            unistd::Pid,
                        };
                        let foreground = cancel_master
                            .lock()
                            .unwrap()
                            .as_ref()
                            .and_then(|m| m.process_group_leader());
                        for group in [foreground, shell_pid.map(|pid| pid as i32)]
                            .into_iter()
                            .flatten()
                        {
                            if group > 0 {
                                let _ = killpg(Pid::from_raw(group), Signal::SIGHUP);
                            }
                        }
                        thread::sleep(Duration::from_millis(50));
                        for group in [foreground, shell_pid.map(|pid| pid as i32)]
                            .into_iter()
                            .flatten()
                        {
                            if group > 0 {
                                let _ = killpg(Pid::from_raw(group), Signal::SIGKILL);
                            }
                        }
                    }
                    let _ = killer.kill();
                    // Closing ConPTY terminates attached processes and unblocks pipe I/O.
                    drop(cancel_master.lock().unwrap().take());
                    break;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    });
    if !stopped.load(Ordering::Acquire) {
        model.lock().unwrap().status = TerminalStatus::Running;
    }
    let _ = wake.try_send(());
    let state = model.clone();
    let notify = wake.clone();
    let reader_stop = Arc::new(AtomicBool::new(false));
    let stop = reader_stop.clone();
    let output = thread::spawn(move || {
        let mut buffer = [0; 8192];
        loop {
            match reader.read(&mut buffer, &stop) {
                Ok(0) => break,
                Ok(count) => {
                    let responses = state.lock().unwrap().feed(&buffer[..count]);
                    let _ = notify.try_send(());
                    for response in responses {
                        if replies.send(Command::Input(response)).is_err() {
                            // The child may already have exited. Keep draining its output.
                            break;
                        }
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                // Unix PTYs report EIO when their slave closes.
                Err(error) if cfg!(unix) && error.raw_os_error() == Some(5) => break,
                Err(_) if stop.load(Ordering::Acquire) => break,
                Err(error) => {
                    let message = format!("Read terminal output: {error}");
                    state.lock().unwrap().status = TerminalStatus::Failed(message.clone());
                    let _ = notify.try_send(());
                    return Err(message);
                }
            }
        }
        Ok(())
    });
    let mut failure = None;
    let exit = loop {
        if stopped.load(Ordering::Acquire) {
            break None;
        }
        if let TerminalStatus::Failed(error) = &model.lock().unwrap().status {
            failure = Some(error.clone());
            break None;
        }
        match child.0.try_wait() {
            Ok(Some(status)) => break Some(status),
            Err(error) => {
                failure = Some(format!("Wait for shell: {error}"));
                break None;
            }
            _ => {}
        }
        let result = match commands.recv_timeout(Duration::from_millis(20)) {
            Ok(Command::Input(bytes)) => writer
                .write_all(&bytes)
                .map_err(|e| format!("Write terminal input: {e}")),
            Ok(Command::Resize(size)) => master
                .lock()
                .unwrap()
                .as_ref()
                .ok_or_else(|| "Terminal is closed".to_owned())
                .and_then(|m| {
                    m.resize(size.pty())
                        .map_err(|e| format!("Resize terminal: {e}"))
                })
                .map(|()| {
                    model.lock().unwrap().resize(size);
                    let _ = wake.try_send(());
                }),
            Err(mpsc::RecvTimeoutError::Disconnected) => break None,
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(()),
        };
        if let Err(error) = result {
            failure = Some(error);
            break None;
        }
    };
    let _ = finished.try_send(());
    let _ = cancellation.join();
    if exit.is_none() {
        #[cfg(unix)]
        {
            // Also cover a close racing with the cancellation worker's completion.
            let foreground = master
                .lock()
                .unwrap()
                .as_ref()
                .and_then(|m| m.process_group_leader());
            for group in [foreground, shell_pid.map(|pid| pid as i32)]
                .into_iter()
                .flatten()
                .filter(|pid| *pid > 0)
            {
                let _ = nix::sys::signal::killpg(
                    nix::unistd::Pid::from_raw(group),
                    nix::sys::signal::Signal::SIGKILL,
                );
            }
        }
        let _ = child.0.kill();
    }
    reader_stop.store(true, Ordering::Release);
    drop(writer);
    drop(master.lock().unwrap().take());
    // Dropping the command receiver also unblocks any protocol reply producer.
    drop(commands);
    let output_result = output
        .join()
        .unwrap_or_else(|_| Err("Terminal reader stopped unexpectedly".into()));
    let status = exit.or_else(|| child.0.wait().ok());
    let mut model = model.lock().unwrap();
    model.exit_status = status.as_ref().map(|s| TerminalExitStatus {
        code: s.exit_code(),
        signal: s.signal().map(str::to_owned),
    });
    model.status = if let Some(error) = failure.or_else(|| output_result.err()) {
        if stopped.load(Ordering::Acquire) {
            TerminalStatus::Closed
        } else {
            TerminalStatus::Failed(error)
        }
    } else if stopped.load(Ordering::Acquire) {
        TerminalStatus::Closed
    } else if let Some(status) = status {
        TerminalStatus::Exited {
            code: status.exit_code(),
            signal: status.signal().map(str::to_owned),
        }
    } else {
        TerminalStatus::Closed
    };
    Ok(())
}

pub(crate) fn rgb_value(value: u32) -> Rgb {
    Rgb {
        r: (value >> 16) as u8,
        g: (value >> 8) as u8,
        b: value as u8,
    }
}

pub(crate) fn scroll(model: &mut Model, lines: i32) {
    model.term.scroll_display(Scroll::Delta(lines));
}

struct ChildGuard(Box<dyn portable_pty::Child + Send + Sync>);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}
#[cfg(unix)]
struct Reader(filedescriptor::FileDescriptor);
#[cfg(unix)]
impl Reader {
    fn new(master: &dyn portable_pty::MasterPty) -> Result<Self, String> {
        use std::os::fd::AsRawFd;
        struct BorrowedMaster(i32);
        impl AsRawFd for BorrowedMaster {
            fn as_raw_fd(&self) -> i32 {
                self.0
            }
        }
        let raw = master.as_raw_fd().ok_or("PTY has no readable descriptor")?;
        // Duplicate while master is borrowed and alive; never take ownership of its fd.
        filedescriptor::FileDescriptor::dup(&BorrowedMaster(raw))
            .map(Self)
            .map_err(|e| e.to_string())
    }
    fn read(&mut self, buffer: &mut [u8], stop: &AtomicBool) -> std::io::Result<usize> {
        use std::os::fd::AsRawFd;
        loop {
            let mut fds = [filedescriptor::pollfd {
                fd: self.0.as_raw_fd(),
                events: filedescriptor::POLLIN,
                revents: 0,
            }];
            match filedescriptor::poll(
                &mut fds,
                Some(if stop.load(Ordering::Acquire) {
                    Duration::ZERO
                } else {
                    Duration::from_millis(20)
                }),
            ) {
                Ok(0) if stop.load(Ordering::Acquire) => return Ok(0),
                Ok(0) => continue,
                Ok(_) => return self.0.read(buffer),
                Err(error) => return Err(std::io::Error::other(error)),
            }
        }
    }
}
#[cfg(windows)]
struct Reader(Box<dyn Read + Send>);
#[cfg(windows)]
impl Reader {
    fn new(master: &dyn portable_pty::MasterPty) -> Result<Self, String> {
        master
            .try_clone_reader()
            .map(Self)
            .map_err(|e| e.to_string())
    }
    fn read(&mut self, buffer: &mut [u8], _: &AtomicBool) -> std::io::Result<usize> {
        self.0.read(buffer)
    }
}
