//! Native, retained PTY terminals. Initialize Kit before constructing a view.
//!
//! `TerminalSession` owns a shell independently of its view. Keep the session
//! entity alive while a panel is hidden; call `close` or drop it to terminate it.
//! `TerminalView` accepts native text/IME and paints only the visible grid.
use gpui_kit::{self as gpui, App, Context, EventEmitter, Subscription, Task};
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf, time::Duration};

mod backend;
mod input;
mod view;
/// Terminal grid and cell dimensions used for PTY resize requests.
pub use backend::Size as TerminalSize;
pub use view::TerminalView;

/// Shell launch options. Explicit executables receive exactly the supplied args.
#[derive(Clone, Debug)]
pub struct TerminalOptions {
    pub program: Option<OsString>,
    pub args: Vec<OsString>,
    /// None inherits the application's working directory.
    pub cwd: Option<PathBuf>,
    /// Overrides the inherited environment, including TERM/COLORTERM if supplied.
    pub env: BTreeMap<OsString, OsString>,
    pub scrollback_lines: usize,
}
impl Default for TerminalOptions {
    fn default() -> Self {
        Self {
            program: None,
            args: vec![],
            cwd: None,
            env: BTreeMap::new(),
            scrollback_lines: 10_000,
        }
    }
}

/// The child exit result, retained even after an explicit close.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalExitStatus {
    pub code: u32,
    pub signal: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalStatus {
    Starting,
    Running,
    Exited { code: u32, signal: Option<String> },
    Failed(String),
    Closed,
}
impl std::fmt::Display for TerminalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Starting => f.write_str("Starting shell…"),
            Self::Running => f.write_str("Running"),
            Self::Exited { code, signal } => match signal {
                Some(signal) => write!(f, "Exited: {signal}"),
                None => write!(f, "Exited with code {code}"),
            },
            Self::Failed(error) => f.write_str(error),
            Self::Closed => f.write_str("Closed"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalEvent {
    StatusChanged(TerminalStatus),
    TitleChanged(String),
}

/// Terminal colors as 0xRRGGBB. Color cube and grayscale entries are generated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalPalette {
    pub ansi: [u32; 16],
    pub foreground: u32,
    pub background: u32,
    pub cursor: u32,
    pub selection: u32,
}
impl TerminalPalette {
    pub fn dark() -> Self {
        Self {
            ansi: [
                0x1e1e1e, 0xf14c4c, 0x23d18b, 0xf5f543, 0x3b8eea, 0xd670d6, 0x29b8db, 0xe5e5e5,
                0x666666, 0xf14c4c, 0x23d18b, 0xf5f543, 0x3b8eea, 0xd670d6, 0x29b8db, 0xffffff,
            ],
            foreground: 0xededed,
            background: 0x171717,
            cursor: 0xededed,
            selection: 0x3b526d,
        }
    }
    pub fn light() -> Self {
        Self {
            ansi: [
                0x202020, 0xa31515, 0x087f23, 0x795e00, 0x0451a5, 0xaf00db, 0x006f7b, 0xcccccc,
                0x666666, 0xc72e2e, 0x16825d, 0x8c7100, 0x005fb8, 0x9c36b5, 0x007f91, 0xffffff,
            ],
            foreground: 0x202020,
            background: 0xfafafa,
            cursor: 0x202020,
            selection: 0xc4dcf5,
        }
    }
    pub(crate) fn indexed(&self, index: usize) -> alacritty_terminal::vte::ansi::Rgb {
        let value = match index {
            0..=15 => self.ansi[index],
            16..=231 => {
                let n = index - 16;
                let component = |n: usize| if n == 0 { 0 } else { 55 + n as u32 * 40 };
                (component(n / 36) << 16) | (component((n / 6) % 6) << 8) | component(n % 6)
            }
            232..=255 => {
                let c = 8 + (index as u32 - 232) * 10;
                (c << 16) | (c << 8) | c
            }
            257 | 268 => self.background,
            258 => self.cursor,
            259..=266 => {
                let c = self.ansi[index - 259];
                ((c & 0xfefefe) >> 1) & 0x7f7f7f
            }
            _ => self.foreground,
        };
        backend::rgb_value(value)
    }
}

/// A retained shell, emulator and bounded I/O queue, independent of panel visibility.
pub struct TerminalSession {
    options: TerminalOptions,
    pub(crate) backend: backend::Backend,
    status: TerminalStatus,
    title: String,
    _notifications: Task<()>,
    _quit: Subscription,
}
impl EventEmitter<TerminalEvent> for TerminalSession {}
impl TerminalSession {
    pub fn new(options: TerminalOptions, cx: &mut Context<Self>) -> Self {
        Self::with_size(options, backend::Size::default(), cx)
    }
    fn with_size(options: TerminalOptions, size: backend::Size, cx: &mut Context<Self>) -> Self {
        Self::with_backend_mode(options, size, false, cx)
    }
    fn with_backend_mode(
        options: TerminalOptions,
        size: backend::Size,
        mocked: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let (wake, receiver) = async_channel::bounded(1);
        #[cfg(feature = "test-support")]
        let backend = if mocked {
            backend::Backend::mock(&options, size)
        } else {
            backend::Backend::start(options.clone(), size, wake)
        };
        #[cfg(not(feature = "test-support"))]
        let backend = {
            debug_assert!(!mocked);
            backend::Backend::start(options.clone(), size, wake)
        };
        let timer = cx.background_executor().clone();
        let notifications = cx.spawn(async move |entity, cx| {
            while receiver.recv().await.is_ok() {
                timer.timer(Duration::from_millis(16)).await;
                // Coalesce output into at most one entity invalidation per frame.
                while receiver.try_recv().is_ok() {}
                if entity
                    .update(cx, |session, cx| session.changed(cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        let quit = cx.on_app_quit(|session, _| {
            session.backend.stop();
            let done = session.backend.done.clone();
            async move {
                let _ = done.recv().await;
            }
        });
        Self {
            options,
            backend,
            status: TerminalStatus::Starting,
            title: String::new(),
            _notifications: notifications,
            _quit: quit,
        }
    }
    /// Construct a process-free session for deterministic UI tests.
    #[cfg(feature = "test-support")]
    pub fn new_test(options: TerminalOptions, cx: &mut Context<Self>) -> Self {
        Self::with_backend_mode(options, backend::Size::default(), true, cx)
    }
    /// Feed recorded PTY output into a process-free test session.
    #[cfg(feature = "test-support")]
    pub fn feed_test_output(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        assert!(
            self.backend.is_mock(),
            "test output requires a test session"
        );
        self.backend.model.lock().unwrap().feed(bytes);
        self.changed(cx);
    }
    /// Drain bytes written through the view, without starting a process.
    #[cfg(feature = "test-support")]
    pub fn take_test_input(&self) -> Vec<u8> {
        self.backend.take_input()
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        let (status, title) = {
            let model = self.backend.model.lock().unwrap();
            (model.status.clone(), model.title.clone())
        };
        if self.status != status {
            self.status = status.clone();
            cx.emit(TerminalEvent::StatusChanged(status));
        }
        if self.title != title {
            self.title = title.clone();
            cx.emit(TerminalEvent::TitleChanged(title));
        }
        cx.notify();
    }
    pub fn status(&self) -> TerminalStatus {
        self.backend.model.lock().unwrap().status.clone()
    }
    pub fn exit_status(&self) -> Option<TerminalExitStatus> {
        self.backend.model.lock().unwrap().exit_status.clone()
    }
    pub fn title(&self) -> String {
        self.backend.model.lock().unwrap().title.clone()
    }
    pub fn options(&self) -> &TerminalOptions {
        &self.options
    }
    /// Visible screen and retained history as plain text, without escape sequences.
    pub fn text(&self) -> String {
        self.backend.model.lock().unwrap().text()
    }
    /// Queue a complete input block without blocking. Rejection queues no bytes.
    pub fn write(&self, bytes: impl Into<Vec<u8>>) -> Result<(), String> {
        let bytes = bytes.into();
        if bytes.len() > backend::MAX_INPUT {
            return Err("Terminal input exceeds 1 MiB".into());
        }
        self.backend.send(backend::Command::Input(bytes))
    }
    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.backend.stop();
        self.backend.model.lock().unwrap().status = TerminalStatus::Closed;
        self.changed(cx);
    }
    /// Replace the process and emulator; retained views follow the new session.
    pub fn restart(&mut self, cx: &mut Context<Self>) {
        let size = self.backend.model.lock().unwrap().size;
        self.backend.stop();
        *self = Self::with_backend_mode(self.options.clone(), size, self.backend.is_mock(), cx);
        cx.emit(TerminalEvent::StatusChanged(TerminalStatus::Starting));
        cx.notify();
    }
    pub fn size(&self) -> TerminalSize {
        self.backend.model.lock().unwrap().size
    }
    /// Queue a resize. Visible views calculate this automatically from their bounds.
    pub fn resize(&self, size: TerminalSize) -> Result<(), String> {
        if size.rows == 0 || size.cols < 2 {
            return Err("Terminal requires at least one row and two columns".into());
        }
        self.backend.send(backend::Command::Resize(size))
    }
}

struct Initialized;
impl gpui::Global for Initialized {}
/// Install scoped clipboard and tab bindings once. Also called by TerminalView::new.
pub fn init(cx: &mut App) {
    if cx.has_global::<Initialized>() {
        return;
    }
    cx.set_global(Initialized);
    let (copy, paste) = if cfg!(target_os = "macos") {
        ("cmd-c", "cmd-v")
    } else {
        ("ctrl-shift-c", "ctrl-shift-v")
    };
    cx.bind_keys([
        gpui::KeyBinding::new("tab", input::Tab, Some("PintleTerminal")),
        gpui::KeyBinding::new("shift-tab", input::BackTab, Some("PintleTerminal")),
        gpui::KeyBinding::new(copy, input::Copy, Some("PintleTerminal")),
        gpui::KeyBinding::new(paste, input::Paste, Some("PintleTerminal")),
    ]);
}

#[cfg(test)]
mod tests;
