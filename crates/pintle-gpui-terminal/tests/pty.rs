//! Real PTY tests run unchanged on all three desktop platforms.
use gpui_kit::{self as gpui, AppContext};
use pintle_gpui_terminal::{TerminalOptions, TerminalSession, TerminalSize, TerminalStatus};
use std::{
    io::{BufRead, Write},
    time::{Duration, Instant},
};

// Run the same test binary as a deterministic child process, without external utilities.
#[test]
fn pty_child_fixture() {
    if std::env::var_os("PINTLE_PTY_FIXTURE").is_none() {
        return;
    }
    println!(
        "FIXTURE_READY:{}:{}",
        std::env::var("PINTLE_PTY_ENV").unwrap(),
        std::env::current_dir().unwrap().display()
    );
    println!(
        "FIXTURE_CWD_OK:{}",
        std::env::current_dir().unwrap().canonicalize().unwrap()
            == std::env::temp_dir().canonicalize().unwrap()
    );
    std::io::stdout().flush().unwrap();
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line).unwrap();
    println!("FIXTURE_RECEIVED:{}", line.trim());
    if line.trim() == "burst" {
        for n in 0..20_000 {
            println!("OUTPUT_{n:05}");
        }
        println!("BATCH_DONE");
    }
    std::io::stdout().flush().unwrap();
}
fn options() -> TerminalOptions {
    TerminalOptions {
        program: Some(std::env::current_exe().unwrap().into_os_string()),
        args: vec![
            "--exact".into(),
            "pty_child_fixture".into(),
            "--nocapture".into(),
        ],
        cwd: Some(std::env::temp_dir()),
        env: [
            ("PINTLE_PTY_FIXTURE".into(), "1".into()),
            ("PINTLE_PTY_ENV".into(), "works".into()),
        ]
        .into(),
        ..Default::default()
    }
}
fn wait_until(
    cx: &mut gpui::TestAppContext,
    session: &gpui::Entity<TerminalSession>,
    predicate: impl Fn(&TerminalSession) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if cx.update(|cx| predicate(session.read(cx))) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "PTY timeout: {}\n{}",
            cx.update(|cx| session.read(cx).status()),
            cx.update(|cx| session.read(cx).text())
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[gpui_kit::test]
fn real_pty_io_cwd_environment_exit_and_independent_sessions(cx: &mut gpui::TestAppContext) {
    let (a, b) = cx.update(|cx| {
        (
            cx.new(|cx| TerminalSession::new(options(), cx)),
            cx.new(|cx| TerminalSession::new(options(), cx)),
        )
    });
    for session in [&a, &b] {
        wait_until(cx, session, |s| s.text().contains("FIXTURE_READY:works:"));
    }
    assert!(cx.update(|cx| a.read(cx).text().contains("FIXTURE_CWD_OK:true")));
    cx.update(|cx| {
        a.read(cx)
            .resize(TerminalSize {
                rows: 31,
                cols: 101,
                ..Default::default()
            })
            .unwrap()
    });
    wait_until(cx, &a, |s| s.size().rows == 31 && s.size().cols == 101);
    cx.update(|cx| a.read(cx).write(b"first\r".to_vec()).unwrap());
    wait_until(cx, &a, |s| {
        matches!(s.status(), TerminalStatus::Exited { code: 0, .. })
    });
    assert!(cx.update(|cx| a.read(cx).text().contains("FIXTURE_RECEIVED:first")));
    assert_eq!(cx.update(|cx| b.read(cx).status()), TerminalStatus::Running);
    cx.update(|cx| b.read(cx).write(b"second\r".to_vec()).unwrap());
    wait_until(cx, &b, |s| {
        matches!(s.status(), TerminalStatus::Exited { code: 0, .. })
    });
    assert!(!cx.update(|cx| a.read(cx).text().contains("second")));
}
#[gpui_kit::test]
fn spawn_failure_and_restart_are_observable(cx: &mut gpui::TestAppContext) {
    let session = cx.update(|cx| {
        cx.new(|cx| {
            TerminalSession::new(
                TerminalOptions {
                    program: Some("/nonexistent/pintle-shell".into()),
                    ..Default::default()
                },
                cx,
            )
        })
    });
    wait_until(cx, &session, |s| {
        matches!(s.status(), TerminalStatus::Failed(_))
    });
    cx.update(|cx| session.update(cx, |s, cx| s.restart(cx)));
    wait_until(cx, &session, |s| {
        matches!(s.status(), TerminalStatus::Failed(_))
    });
}

#[gpui_kit::test]
fn continuous_output_is_drained_before_exit_and_history_stays_bounded(
    cx: &mut gpui::TestAppContext,
) {
    let session = cx.update(|cx| {
        cx.new(|cx| {
            TerminalSession::new(
                TerminalOptions {
                    scrollback_lines: 100,
                    ..options()
                },
                cx,
            )
        })
    });
    wait_until(cx, &session, |s| s.text().contains("FIXTURE_READY:works:"));
    cx.update(|cx| session.read(cx).write(b"burst\r".to_vec()).unwrap());
    wait_until(cx, &session, |s| {
        matches!(s.status(), TerminalStatus::Exited { code: 0, .. })
    });
    let text = cx.update(|cx| session.read(cx).text());
    assert!(text.contains("OUTPUT_19999"), "last output was lost");
    assert!(
        text.contains("BATCH_DONE"),
        "exit notification overtook output"
    );
    assert!(text.lines().count() <= 124);
}
