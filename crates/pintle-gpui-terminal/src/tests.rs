use super::*;
use crate::backend::{Model, Size};
use alacritty_terminal::{
    grid::{Dimensions, Scroll},
    index::{Column, Line, Point, Side},
    selection::{Selection, SelectionType},
    term::{TermMode, cell::Flags},
    vte::ansi::Color,
};

#[test]
fn streaming_parser_preserves_split_utf8_and_escape_sequences() {
    let mut model = Model::new(&TerminalOptions::default(), Size::default());
    for byte in "\x1b[31mสวัสดี e\u{301} 界\x1b[0m".as_bytes() {
        model.feed(&[*byte]);
    }
    let text = model.text();
    assert!(text.contains("สวัสดี e\u{301} 界"), "{text:?}");
    assert_eq!(
        model.term.grid()[Point::new(Line(0), Column(0))].fg,
        Color::Named(alacritty_terminal::vte::ansi::NamedColor::Red)
    );
    assert!(
        model
            .term
            .grid()
            .display_iter()
            .any(|c| c.cell.flags.contains(Flags::WIDE_CHAR))
    );
}

#[test]
fn cursor_erase_and_alternate_screen_restore_primary_buffer() {
    let mut model = Model::new(&TerminalOptions::default(), Size::default());
    model.feed(b"main\x1b[?1049hfull-screen\x1b[2;3HXY\x1b[?1049l");
    assert!(model.text().contains("main"));
    assert!(!model.text().contains("full-screen"));
    model.feed(b"\r\x1b[2Kdone");
    assert!(model.text().starts_with("done"));
    assert!(!model.text().contains("main"));
}

#[test]
fn history_is_bounded_selection_and_resize_are_retained() {
    let options = TerminalOptions {
        scrollback_lines: 10,
        ..Default::default()
    };
    let mut model = Model::new(
        &options,
        Size {
            rows: 3,
            cols: 8,
            ..Default::default()
        },
    );
    for i in 0..50 {
        model.feed(format!("{i:02}\r\n").as_bytes());
    }
    assert!(model.term.history_size() <= 10);
    model.term.scroll_display(Scroll::Top);
    assert!(model.term.grid().display_offset() > 0);
    model.term.scroll_display(Scroll::Bottom);
    model.term.selection = Some(Selection::new(
        SelectionType::Simple,
        Point::new(Line(0), Column(0)),
        Side::Left,
    ));
    model
        .term
        .selection
        .as_mut()
        .unwrap()
        .update(Point::new(Line(0), Column(1)), Side::Right);
    assert_eq!(model.term.selection_to_string().unwrap().len(), 2);
    model.resize(Size {
        rows: 5,
        cols: 20,
        ..Default::default()
    });
    assert_eq!(model.term.columns(), 20);
    assert_eq!(model.term.screen_lines(), 5);
}

#[test]
fn truecolor_and_device_queries_are_answered() {
    let mut model = Model::new(&TerminalOptions::default(), Size::default());
    let replies = model.feed(b"\x1b[38;2;1;2;3mX\x1b[6n\x1b[c\x1b]10;?\x07");
    assert_eq!(
        model.term.grid()[Point::new(Line(0), Column(0))].fg,
        Color::Spec(backend::rgb_value(0x010203))
    );
    assert!(replies.iter().any(|r| r.starts_with(b"\x1b[1;2R")));
    assert!(replies.len() >= 3);
}

#[test]
fn keyboard_modes_modifiers_and_paste_encoding() {
    let bytes =
        |key| input::key_bytes(&gpui::Keystroke::parse(key).unwrap(), TermMode::empty()).unwrap();
    for (key, expected) in [
        ("ctrl-b", 2),
        ("ctrl-j", 10),
        ("ctrl-k", 11),
        ("ctrl-c", 3),
        ("ctrl-q", 17),
    ] {
        assert_eq!(bytes(key), [expected]);
    }
    assert_eq!(bytes("shift-left"), b"\x1b[1;2D");
    assert_eq!(bytes("tab"), b"\t");
    assert_eq!(bytes("shift-tab"), b"\x1b[Z");
    assert_eq!(bytes("f12"), b"\x1b[24~");
    assert_eq!(
        input::key_bytes(&gpui::Keystroke::parse("up").unwrap(), TermMode::APP_CURSOR).unwrap(),
        b"\x1bOA"
    );
    assert!(input::key_bytes(&gpui::Keystroke::parse("a").unwrap(), TermMode::empty()).is_none());
    assert_eq!(
        input::paste_bytes("a\nb", TermMode::BRACKETED_PASTE),
        b"\x1b[200~a\nb\x1b[201~"
    );
    assert_eq!(input::paste_bytes("a\r\nb", TermMode::empty()), b"a\rb");
}

#[test]
fn mouse_sgr_press_release_and_motion_encoding() {
    let mode = TermMode::SGR_MOUSE;
    assert_eq!(
        input::mouse_bytes(0, 4, 2, false, Default::default(), mode),
        b"\x1b[<0;5;3M"
    );
    assert_eq!(
        input::mouse_bytes(0, 4, 2, true, Default::default(), mode),
        b"\x1b[<0;5;3m"
    );
    assert_eq!(
        input::mouse_bytes(32, 4, 2, false, Default::default(), mode),
        b"\x1b[<32;5;3M"
    );
}

#[gpui_kit::test]
fn tab_keys_reach_terminal_without_moving_focus(cx: &mut gpui::TestAppContext) {
    use gpui::prelude::*;
    use gpui::test::TestWindowExt;
    use gpui::{AppContext, FocusHandle, Focusable, TestSupportExt};

    struct Content {
        first: FocusHandle,
        second: FocusHandle,
        terminal: gpui::Entity<TerminalView>,
    }
    impl gpui::Render for Content {
        fn render(
            &mut self,
            _: &mut gpui::Window,
            _: &mut gpui::Context<Self>,
        ) -> impl IntoElement {
            let first = self.first.clone();
            gpui::div()
                .size_full()
                .flex()
                .flex_col()
                .child(
                    gpui::div()
                        .id("outside-terminal")
                        .test_support()
                        .track_focus(&self.first)
                        .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                            first.focus(window, cx);
                        })
                        .child("First control"),
                )
                .child(
                    gpui::div()
                        .track_focus(&self.second)
                        .child("Second control"),
                )
                .child(self.terminal.clone())
        }
    }
    let (handle, content, session) = cx.update(|cx| {
        gpui::init(cx);
        pintle_gpui_theme::install(Default::default(), cx);
        let session = cx.new(|cx| TerminalSession::new_test(Default::default(), cx));
        let s = session.clone();
        let (handle, content) = gpui::open_window(Default::default(), cx, |window, cx| {
            let terminal = cx.new(|cx| TerminalView::new(s, window, cx));
            window.focus(&terminal.focus_handle(cx), cx);
            cx.new(|cx| Content {
                first: cx.focus_handle().tab_stop(true),
                second: cx.focus_handle().tab_stop(true),
                terminal,
            })
        })
        .unwrap();
        (handle, content, session)
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        for _ in 0..3 {
            window.press("tab", cx);
            window.press("shift-tab", cx);
            assert!(content.read(cx).terminal.read(cx).is_focused(window));
            assert_eq!(session.read(cx).take_test_input(), b"\t\x1b[Z");
        }
        window.click("outside-terminal", cx);
        assert!(content.read(cx).first.is_focused(window));
        window.press("tab", cx);
        assert!(content.read(cx).second.is_focused(window));
        window.press("shift-tab", cx);
        assert!(content.read(cx).first.is_focused(window));
        assert!(session.read(cx).take_test_input().is_empty());
    })
    .unwrap();
}

#[gpui_kit::test]
fn native_input_ime_clipboard_and_restart_with_mock_backend(cx: &mut gpui::TestAppContext) {
    use gpui::test::TestWindowExt;
    use gpui::{AppContext, EntityInputHandler};
    let (handle, session, view) = cx.update(|cx| {
        gpui::init(cx);
        pintle_gpui_theme::install(Default::default(), cx);
        let session = cx.new(|cx| TerminalSession::new_test(Default::default(), cx));
        let s = session.clone();
        let mut retained = None;
        let (window, _) = gpui::open_window(Default::default(), cx, |window, cx| {
            let view = cx.new(|cx| TerminalView::new(s, window, cx));
            window.focus(&gpui::Focusable::focus_handle(view.read(cx), cx), cx);
            retained = Some(view.clone());
            view
        })
        .unwrap();
        (window, session, retained.unwrap())
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("terminal-view").visible());
        for key in ["ctrl-b", "ctrl-j", "ctrl-k", "ctrl-c", "ctrl-q", "left"] {
            window.press(key, cx);
        }
        assert_eq!(
            session.read(cx).take_test_input(),
            b"\x02\n\x0b\x03\x11\x1b[D"
        );
        view.update(cx, |view, cx| {
            view.replace_and_mark_text_in_range(None, "ก้😀", Some(2..4), window, cx);
            assert_eq!(view.marked_text_range(window, cx), Some(0..4));
            assert_eq!(
                view.selected_text_range(false, window, cx).unwrap().range,
                2..4
            );
        });
        assert!(session.read(cx).take_test_input().is_empty());
        view.update(cx, |view, cx| {
            view.replace_text_in_range(None, "ก้😀", window, cx)
        });
        assert_eq!(session.read(cx).take_test_input(), "ก้😀".as_bytes());
        view.update(cx, |view, cx| {
            view.replace_and_mark_text_in_range(None, "a😀b", None, window, cx);
            view.replace_text_in_range(Some(1..3), "界", window, cx);
        });
        assert_eq!(session.read(cx).take_test_input(), "a界b".as_bytes());
        session.update(cx, |s, cx| s.feed_test_output(b"\x1b[?2004h", cx));
        view.update(cx, |view, cx| view.paste("hello\nworld", cx));
        assert_eq!(
            session.read(cx).take_test_input(),
            b"\x1b[200~hello\nworld\x1b[201~"
        );
        session.update(cx, |s, cx| {
            s.feed_test_output(b"\x1b[?2004l\x1b[2J\x1b[Hhello world", cx)
        });
        window.render_frame(cx);
        let bounds = window.find("terminal-view").bounds();
        let cell = session.read(cx).size();
        let start = gpui::point(
            bounds.left() + gpui::px(1.),
            bounds.top() + gpui::px(cell.cell_height as f32 / 2.),
        );
        let end = gpui::point(
            bounds.left() + gpui::px(cell.cell_width as f32 * 4.9),
            start.y,
        );
        window.drag(start, end, cx);
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-c"
            } else {
                "ctrl-shift-c"
            },
            cx,
        );
        assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), "hello");
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-v"
            } else {
                "ctrl-shift-v"
            },
            cx,
        );
        assert_eq!(session.read(cx).take_test_input(), b"hello");
        assert!(
            session
                .read(cx)
                .write(vec![0; backend::MAX_INPUT + 1])
                .is_err()
        );
        assert!(session.read(cx).take_test_input().is_empty());
        session.update(cx, |s, cx| {
            s.close(cx);
            s.restart(cx);
        });
        assert_eq!(session.read(cx).status(), TerminalStatus::Running);
        assert!(session.read(cx).text().trim().is_empty());
    })
    .unwrap();
}

#[cfg(unix)]
#[test]
fn real_backend_resizes_and_cancels_foreground_jobs_and_blocked_paste() {
    use crate::backend::{Backend, Command};
    use std::{
        thread,
        time::{Duration, Instant},
    };
    let options = TerminalOptions {
        program: Some("/bin/sh".into()),
        args: vec![
            "-c".into(),
            "stty -echo; printf READY; while IFS= read -r line; do stty size; done".into(),
        ],
        ..Default::default()
    };
    let (wake, _) = async_channel::bounded(1);
    let backend = Backend::start(options, Size::default(), wake);
    let wait = |predicate: &dyn Fn() -> bool| {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !predicate() {
            assert!(Instant::now() < deadline, "Backend timed out");
            thread::sleep(Duration::from_millis(10));
        }
    };
    wait(&|| backend.model.lock().unwrap().text().contains("READY"));
    backend
        .send(Command::Resize(Size {
            rows: 37,
            cols: 93,
            ..Default::default()
        }))
        .unwrap();
    backend.send(Command::Input(b"size\r".to_vec())).unwrap();
    wait(&|| backend.model.lock().unwrap().text().contains("37 93"));
    backend.stop();
    wait(&|| backend.done.is_closed());
    assert_eq!(backend.model.lock().unwrap().status, TerminalStatus::Closed);

    let (wake, _) = async_channel::bounded(1);
    let backend = Backend::start(
        TerminalOptions {
            program: Some("/bin/sh".into()),
            args: vec!["-ic".into(), "printf READY; sleep 60".into()],
            ..Default::default()
        },
        Size::default(),
        wake,
    );
    wait(&|| backend.model.lock().unwrap().text().contains("READY"));
    backend
        .send(Command::Input(vec![b'x'; backend::MAX_INPUT]))
        .unwrap();
    thread::sleep(Duration::from_millis(50));
    backend.stop();
    wait(&|| backend.done.is_closed());
}

#[gpui_kit::test]
fn visible_view_resizes_the_grid_from_font_metrics(cx: &mut gpui::TestAppContext) {
    use gpui::test::TestWindowExt;
    use gpui::{AppContext, px, size};
    let (handle, session) = cx.update(|cx| {
        gpui::init(cx);
        pintle_gpui_theme::install(Default::default(), cx);
        let session = cx.new(|cx| TerminalSession::new_test(Default::default(), cx));
        let s = session.clone();
        let (handle, _) = gpui::open_window(Default::default(), cx, |window, cx| {
            cx.new(|cx| TerminalView::new(s, window, cx))
        })
        .unwrap();
        (handle, session)
    });
    cx.simulate_window_resize(handle, size(px(800.), px(400.)));
    let first = cx
        .update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            session.read(cx).size()
        })
        .unwrap();
    cx.simulate_window_resize(handle, size(px(600.), px(600.)));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let second = session.read(cx).size();
        assert!(second.rows > first.rows);
        assert!(second.cols < first.cols);
        assert_eq!(second.cell_width, first.cell_width);
        assert_eq!(second.cell_height, first.cell_height);
    })
    .unwrap();
}

#[test]
fn native_default_shell_stops_and_reaps_on_close() {
    use std::{
        thread,
        time::{Duration, Instant},
    };
    let (wake, _) = async_channel::bounded(1);
    let backend = backend::Backend::start(Default::default(), Size::default(), wake);
    let deadline = Instant::now() + Duration::from_secs(15);
    while backend.model.lock().unwrap().status == TerminalStatus::Starting {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        backend.model.lock().unwrap().status,
        TerminalStatus::Running
    );
    backend.stop();
    while !backend.done.is_closed() {
        assert!(
            Instant::now() < deadline,
            "Terminal shutdown did not finish"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let model = backend.model.lock().unwrap();
    assert_eq!(model.status, TerminalStatus::Closed);
    assert!(model.exit_status.is_some());
}
