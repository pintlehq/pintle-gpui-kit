# Native terminal integration

Enable the facade's `terminal` feature. It enables theme and the native terminal
crate, independently of `shell`; neither terminal nor its PTY dependencies are
part of the default features.

```toml
[dependencies]
pintle-gpui-kit = { path = "../pintle-gpui-kit/crates/pintle-gpui-kit", default-features = false, features = ["terminal"] }
```

Initialize Foundation before constructing a session or view. In a window builder:

```rust,ignore
use pintle_gpui_kit::{kit as gpui, terminal::{TerminalOptions, TerminalSession, TerminalView}};
use gpui::AppContext;

let session = cx.new(|cx| TerminalSession::new(TerminalOptions::default(), cx));
let view = cx.new(|cx| TerminalView::new(session.clone(), window, cx));
```

Direct consumers of `pintle-gpui-terminal` initialize Kit and install the Pintle
theme themselves. `TerminalView::new` installs scoped clipboard and Tab bindings once.
It implements Render and Focusable and can live in any caller-owned panel.

## Launch and lifecycle

`TerminalOptions` accepts an optional `program`, an argument vector, optional
`cwd`, environment overrides and `scrollback_lines` (default 10,000). Unix defaults
to `$SHELL`, falling back to `/bin/sh`, with `-i` when no explicit program/args are
provided. Windows defaults to `%COMSPEC%`, falling back to `cmd.exe`. Explicit
programs receive exactly the supplied arguments; arguments are never shell-joined.
An omitted cwd inherits the application's current working directory. TERM defaults
to `xterm-256color` and COLORTERM to `truecolor`; explicit environment entries win.

Construction starts asynchronously. Observe `TerminalEvent::StatusChanged` and
`TitleChanged`, or inspect `status()`, `title()`, `exit_status()` and `size()`.
The status is Starting, Running, Exited (with code/signal), Failed (with error text)
or Closed. Exit text remains visible. `restart(cx)` creates a fresh emulator and
process using the same options and most recent size. `close(cx)` initiates
cancellation without waiting on the UI thread; exit status is retained once reaped.
A dropped session also cancels its backend, and app quit awaits worker completion.

Retain the session entity to keep a shell running while hidden. Multiple sessions
have independent processes, selections, history and scroll positions. A session
should normally have one retained view; create a separate session for an independent
terminal. No process or history is persisted across application launches.

`write(bytes)` accepts a complete input block of up to 1 MiB. Its bounded queue
returns an error if full or disconnected, without partially queuing that block.
`resize(TerminalSize)` queues a grid/cell resize; views do this automatically from
font metrics and layout bounds. It requires at least one row and two columns.
`text()` exports retained history and screen as plain text and may allocate in
proportion to that history; avoid calling it every frame.

Closing terminates attached terminal jobs. Deliberately detached daemons are not
managed as persistent services by this library.

## Rendering and input

The native GPUI surface supports ANSI 16/256 colors and truecolor, bold/italic/dim,
underlines/strikethrough, inverse text, cursor shapes, Unicode combining marks and
wide cells, alternate screens and terminal query replies. `set_font` and
`set_palette` customize the view; the default palette follows the Pintle theme.
Gallery uses JetBrains Mono at 13 px. Images, ligatures across terminal cells,
terminal search, shell integration and clickable hyperlinks are not implemented.

Text and IME preedit use `EntityInputHandler`; composition is displayed locally
and only committed text reaches the PTY. Control keys, arrows and F1–F12 use
xterm encoding, including application cursor mode and modifiers. Copy/paste uses
Cmd+C/V on macOS and Ctrl+Shift+C/V on Windows/Linux. Ctrl+C always reaches the
shell. Paste honors bracketed-paste mode. OSC 52 clipboard requests are disabled.
Tab and Shift+Tab send their terminal sequences without moving focus to another
control; normal focus traversal still applies outside the terminal.

Drag selects text; double/triple click selects a word/line. Wheel/trackpad input
scrolls history, or sends mouse/alternate-screen scroll events when requested.
Mouse reporting supports legacy, UTF-8 and SGR encodings. Hold Shift to select or
scroll locally instead. The caller owns application shortcuts: exclude the
`PintleTerminal` key context from Ctrl bindings that the shell should receive.

## Gallery and testing

Gallery adds Terminal to the existing resizable inspector. Ctrl+` opens/focuses
it, and hides the panel when the terminal already owns focus. The picker switches
sessions; New, Restart and Close Session affect shells, while Close Panel only
hides them. The command palette exposes Toggle Terminal and New Terminal. Closing
the last session leaves a New Terminal button rather than automatically respawning.
Cmd+W within the Terminal panel closes the selected session and focuses another
session when available. Document tabs are unaffected. This binding belongs to
Gallery; library consumers choose their own application shortcuts.

For deterministic UI tests enable `terminal-test-support` on the facade as a dev
dependency (or `test-support` on the terminal crate). `TerminalSession::new_test`
starts no process; `feed_test_output` supplies recorded bytes, and `take_test_input`
drains bytes written by keyboard/IME/clipboard handlers. Restart preserves the test
backend. Production construction still uses real PTYs even when that feature is
unified into the dependency graph.

The terminal CI matrix builds on macOS 15, Ubuntu 24.04 and Windows Server 2022.
It runs parser/native-input tests, Gallery terminal interactions and real PTY
fixtures on each native platform. Windows requires ConPTY. Linux requires the
native graphics/font libraries used by GPUI. See [validation](validation.md) for
actual local checks and remaining native UI verification.
