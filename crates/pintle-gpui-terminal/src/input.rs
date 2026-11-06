//! xterm keyboard and mouse encoding, kept independent of the renderer.
use alacritty_terminal::term::TermMode;
use gpui_kit::{Keystroke, Modifiers};
gpui_kit::actions!(pintle_terminal, [Copy, Paste, Tab, BackTab]);

pub(crate) fn key_bytes(key: &Keystroke, mode: TermMode) -> Option<Vec<u8>> {
    let m = key.modifiers;
    if m.platform {
        return None;
    }
    let modifier = 1 + u8::from(m.shift) + 2 * u8::from(m.alt) + 4 * u8::from(m.control);
    let key_name = key.key.to_lowercase();
    let suffix = match key_name.as_str() {
        "up" => Some('A'),
        "down" => Some('B'),
        "right" => Some('C'),
        "left" => Some('D'),
        "home" => Some('H'),
        "end" => Some('F'),
        _ => None,
    };
    if let Some(suffix) = suffix {
        return Some(
            if modifier != 1 {
                format!("\x1b[1;{modifier}{suffix}")
            } else if mode.contains(TermMode::APP_CURSOR) {
                format!("\x1bO{suffix}")
            } else {
                format!("\x1b[{suffix}")
            }
            .into_bytes(),
        );
    }
    let function = match key_name.as_str() {
        "insert" => Some(2),
        "delete" => Some(3),
        "pageup" => Some(5),
        "pagedown" => Some(6),
        "f5" => Some(15),
        "f6" => Some(17),
        "f7" => Some(18),
        "f8" => Some(19),
        "f9" => Some(20),
        "f10" => Some(21),
        "f11" => Some(23),
        "f12" => Some(24),
        _ => None,
    };
    if let Some(n) = function {
        return Some(
            if modifier == 1 {
                format!("\x1b[{n}~")
            } else {
                format!("\x1b[{n};{modifier}~")
            }
            .into_bytes(),
        );
    }
    if let Some(n) = ["f1", "f2", "f3", "f4"].iter().position(|s| *s == key_name) {
        let suffix = (b'P' + n as u8) as char;
        return Some(
            if modifier == 1 {
                format!("\x1bO{suffix}")
            } else {
                format!("\x1b[1;{modifier}{suffix}")
            }
            .into_bytes(),
        );
    }
    let byte = match key_name.as_str() {
        "enter" => Some(b'\r'),
        "escape" => Some(0x1b),
        "backspace" => Some(if m.control { 8 } else { 127 }),
        "tab" if m.shift => return Some(b"\x1b[Z".to_vec()),
        "tab" => Some(b'\t'),
        "space" if m.control => Some(0),
        _ if m.control && key_name.len() == 1 => {
            let c = key_name.as_bytes()[0];
            match c {
                b'a'..=b'z' => Some(c - b'a' + 1),
                b'['..=b'_' => Some(c & 0x1f),
                b'2' | b'@' => Some(0),
                b'3' => Some(27),
                b'4' => Some(28),
                b'5' => Some(29),
                b'6' => Some(30),
                b'7' | b'/' => Some(31),
                b'8' | b'?' => Some(127),
                _ => None,
            }
        }
        _ => None,
    };
    if let Some(byte) = byte {
        let mut bytes = vec![];
        if m.alt {
            bytes.push(27);
        }
        bytes.push(byte);
        return Some(bytes);
    }
    // Plain text comes exclusively through native input/IME. Alt is the terminal Meta modifier.
    if m.alt && !m.control {
        let text = key.key_char.as_deref().unwrap_or(&key.key);
        if text.chars().count() == 1 {
            return Some(format!("\x1b{text}").into_bytes());
        }
    }
    None
}

pub(crate) fn paste_bytes(text: &str, mode: TermMode) -> Vec<u8> {
    if mode.contains(TermMode::BRACKETED_PASTE) {
        // Do not allow clipboard text to prematurely end bracketed paste.
        format!("\x1b[200~{}\x1b[201~", text.replace('\x1b', "")).into_bytes()
    } else {
        text.replace("\r\n", "\n").replace('\n', "\r").into_bytes()
    }
}

pub(crate) fn mouse_bytes(
    button: u8,
    col: usize,
    row: usize,
    release: bool,
    modifiers: Modifiers,
    mode: TermMode,
) -> Vec<u8> {
    let code = button
        + if modifiers.shift { 4 } else { 0 }
        + if modifiers.alt { 8 } else { 0 }
        + if modifiers.control { 16 } else { 0 };
    if mode.contains(TermMode::SGR_MOUSE) {
        format!(
            "\x1b[<{code};{};{}{}",
            col + 1,
            row + 1,
            if release { 'm' } else { 'M' }
        )
        .into_bytes()
    } else {
        let code = if release { 3 } else { code };
        if mode.contains(TermMode::UTF8_MOUSE) {
            let mut bytes = vec![27, b'[', b'M', code + 32];
            for value in [col + 33, row + 33] {
                let mut buf = [0; 4];
                bytes.extend_from_slice(
                    char::from_u32(value.min(2047) as u32)
                        .unwrap()
                        .encode_utf8(&mut buf)
                        .as_bytes(),
                );
            }
            bytes
        } else {
            vec![
                27,
                b'[',
                b'M',
                code + 32,
                (col + 33).min(255) as u8,
                (row + 33).min(255) as u8,
            ]
        }
    }
}
