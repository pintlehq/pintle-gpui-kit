use crate::{
    TerminalPalette, TerminalSession, TerminalStatus,
    backend::{self, Size},
    input,
};
use alacritty_terminal::{
    grid::Scroll,
    index::{Column, Line, Point as GridPoint, Side},
    selection::{Selection, SelectionType},
    term::{TermMode, cell::Flags},
    vte::ansi::{Color, CursorShape},
};
use gpui_kit::{self as gpui, component::ActiveTheme, prelude::*, *};
use std::ops::Range;

/// A focusable native terminal surface. Retain its entity to retain selection,
/// IME and view configuration; retain the session to keep its shell alive.
pub struct TerminalView {
    session: Entity<TerminalSession>,
    focus: FocusHandle,
    palette: Option<TerminalPalette>,
    font_family: Option<SharedString>,
    font_size: Pixels,
    bounds: Bounds<Pixels>,
    cell: Size,
    requested_size: Option<Size>,
    selecting: bool,
    pressed: Option<MouseButton>,
    last_mouse: Option<(usize, usize)>,
    wheel_remainder: f32,
    composition: String,
    composition_selection: Range<usize>,
    cursor_bounds: Bounds<Pixels>,
    input_error: Option<String>,
    _subscriptions: Vec<Subscription>,
}
impl TerminalView {
    pub fn new(
        session: Entity<TerminalSession>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        crate::init(cx);
        let focus = cx.focus_handle();
        let observe = cx.observe(&session, |this, _, cx| {
            this.requested_size = None;
            cx.notify();
        });
        let focus_in = cx.on_focus(&focus, window, |this, _, cx| this.focus_changed(true, cx));
        let focus_out = cx.on_blur(&focus, window, |this, _, cx| this.focus_changed(false, cx));
        Self {
            session,
            focus,
            palette: None,
            font_family: None,
            font_size: px(13.),
            bounds: Bounds::default(),
            cell: Size::default(),
            requested_size: None,
            selecting: false,
            pressed: None,
            last_mouse: None,
            wheel_remainder: 0.,
            composition: String::new(),
            composition_selection: 0..0,
            cursor_bounds: Bounds::default(),
            input_error: None,
            _subscriptions: vec![observe, focus_in, focus_out],
        }
    }
    pub fn session(&self) -> &Entity<TerminalSession> {
        &self.session
    }
    pub fn set_palette(&mut self, palette: Option<TerminalPalette>, cx: &mut Context<Self>) {
        self.palette = palette;
        cx.notify();
    }
    pub fn set_font(
        &mut self,
        family: impl Into<SharedString>,
        size: Pixels,
        cx: &mut Context<Self>,
    ) {
        self.font_family = Some(family.into());
        self.font_size = size.max(px(6.));
        cx.notify();
    }
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus, cx);
    }
    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus.is_focused(window)
    }
    fn mode(&self, cx: &App) -> TermMode {
        *self
            .session
            .read(cx)
            .backend
            .model
            .lock()
            .unwrap()
            .term
            .mode()
    }
    fn send(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        if bytes.is_empty() {
            return;
        }
        self.input_error = self.session.read(cx).write(bytes).err();
        if self.input_error.is_none() {
            let mut model = self.session.read(cx).backend.model.lock().unwrap();
            model.term.selection = None;
            model.term.scroll_display(Scroll::Bottom);
        }
        cx.notify();
    }
    fn focus_changed(&mut self, focused: bool, cx: &mut Context<Self>) {
        let report = {
            let mut model = self.session.read(cx).backend.model.lock().unwrap();
            model.term.is_focused = focused;
            model.term.mode().contains(TermMode::FOCUS_IN_OUT)
        };
        if report {
            self.input_error = self
                .session
                .read(cx)
                .write(if focused { b"\x1b[I" } else { b"\x1b[O" }.to_vec())
                .err();
        }
        if !focused {
            self.selecting = false;
            self.pressed = None;
        }
        cx.notify();
    }
    pub fn copy(&self, cx: &mut App) {
        let text = self
            .session
            .read(cx)
            .backend
            .model
            .lock()
            .unwrap()
            .term
            .selection_to_string();
        if let Some(text) = text {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }
    pub fn paste(&mut self, text: &str, cx: &mut Context<Self>) {
        self.send(input::paste_bytes(text, self.mode(cx)), cx);
    }
    fn position(&self, position: Point<Pixels>) -> (usize, usize, Side) {
        let x = f32::from(position.x - self.bounds.left()) / self.cell.cell_width.max(1) as f32;
        let y = f32::from(position.y - self.bounds.top()) / self.cell.cell_height.max(1) as f32;
        (
            x.floor().max(0.) as usize,
            y.floor().max(0.) as usize,
            if x.fract() < 0.5 {
                Side::Left
            } else {
                Side::Right
            },
        )
    }
    fn mouse(
        &mut self,
        position: Point<Pixels>,
        button: u8,
        release: bool,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        let (col, row, _) = self.position(position);
        let mode = self.mode(cx);
        let (col, row) = (
            col.min(self.cell.cols as usize - 1),
            row.min(self.cell.rows as usize - 1),
        );
        // Mouse/protocol traffic must preserve scrollback and the user's selection.
        self.input_error = self
            .session
            .read(cx)
            .write(input::mouse_bytes(
                button, col, row, release, modifiers, mode,
            ))
            .err();
    }
    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        self.pressed = Some(event.button);
        self.last_mouse = None;
        if !event.modifiers.shift && self.mode(cx).intersects(TermMode::MOUSE_MODE) {
            self.mouse(
                event.position,
                button_code(event.button),
                false,
                event.modifiers,
                cx,
            );
        } else if event.button == MouseButton::Left {
            self.selecting = true;
            let (col, row, side) = self.position(event.position);
            let mut model = self.session.read(cx).backend.model.lock().unwrap();
            let point = GridPoint::new(
                Line(
                    row.min(model.size.rows as usize - 1) as i32
                        - model.term.grid().display_offset() as i32,
                ),
                Column(col.min(model.size.cols as usize - 1)),
            );
            let kind = match event.click_count {
                2 => SelectionType::Semantic,
                3.. => SelectionType::Lines,
                _ => SelectionType::Simple,
            };
            model.term.selection = Some(Selection::new(kind, point, side));
        }
        window.prevent_default();
        cx.stop_propagation();
        cx.notify();
    }
    fn mouse_up(&mut self, event: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.pressed.is_none() && !self.selecting {
            return;
        }
        if self.pressed.is_some()
            && !self.selecting
            && !event.modifiers.shift
            && self.mode(cx).intersects(TermMode::MOUSE_MODE)
        {
            self.mouse(
                event.position,
                button_code(event.button),
                true,
                event.modifiers,
                cx,
            );
        }
        self.selecting = false;
        self.pressed = None;
        cx.stop_propagation();
    }
    fn mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let (col, row, side) = self.position(event.position);
        if self.selecting {
            let mut model = self.session.read(cx).backend.model.lock().unwrap();
            let outside = if event.position.y < self.bounds.top() {
                1
            } else if event.position.y > self.bounds.bottom() {
                -1
            } else {
                0
            };
            if outside != 0 {
                backend::scroll(&mut model, outside);
            }
            let point = GridPoint::new(
                Line(
                    row.min(model.size.rows as usize - 1) as i32
                        - model.term.grid().display_offset() as i32,
                ),
                Column(col.min(model.size.cols as usize - 1)),
            );
            if let Some(selection) = &mut model.term.selection {
                selection.update(point, side);
            }
            drop(model);
            cx.notify();
        } else if !event.modifiers.shift {
            let mode = self.mode(cx);
            let motion = mode.contains(TermMode::MOUSE_MOTION)
                || (mode.contains(TermMode::MOUSE_DRAG) && self.pressed.is_some());
            if motion && self.last_mouse != Some((col, row)) {
                let code = self.pressed.map(button_code).unwrap_or(3) + 32;
                self.mouse(event.position, code, false, event.modifiers, cx);
                self.last_mouse = Some((col, row));
            }
        }
    }
    fn scroll(&mut self, event: &ScrollEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.wheel_remainder +=
            f32::from(event.delta.pixel_delta(px(self.cell.cell_height as f32)).y)
                / self.cell.cell_height.max(1) as f32;
        let lines = self.wheel_remainder.trunc() as i32;
        self.wheel_remainder -= lines as f32;
        if lines == 0 {
            cx.stop_propagation();
            return;
        }
        let mode = self.mode(cx);
        if !event.modifiers.shift && mode.intersects(TermMode::MOUSE_MODE) {
            for _ in 0..lines.unsigned_abs().min(100) {
                self.mouse(
                    event.position,
                    if lines > 0 { 64 } else { 65 },
                    false,
                    event.modifiers,
                    cx,
                );
            }
        } else if !event.modifiers.shift
            && mode.contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL)
        {
            let sequence = if lines > 0 { b"\x1bOA" } else { b"\x1bOB" };
            self.send(sequence.repeat(lines.unsigned_abs().min(100) as usize), cx);
        } else {
            backend::scroll(
                &mut self.session.read(cx).backend.model.lock().unwrap(),
                lines,
            );
        }
        cx.stop_propagation();
        cx.notify();
    }
    fn frame(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Frame {
        self.bounds = bounds;
        let palette = self.palette.clone().unwrap_or_else(|| {
            if cx.theme().is_dark() {
                TerminalPalette::dark()
            } else {
                TerminalPalette::light()
            }
        });
        let font = gpui::font(
            self.font_family
                .clone()
                .unwrap_or_else(|| cx.theme().mono_font_family.clone()),
        );
        let font_id = window.text_system().resolve_font(&font);
        let width = window
            .text_system()
            .ch_advance(font_id, self.font_size)
            .unwrap_or(px(8.))
            .ceil()
            .max(px(1.));
        let height = (self.font_size * 1.5).ceil().max(px(1.));
        let new_size = Size {
            rows: ((bounds.size.height / height).floor() as u16).max(1),
            cols: ((bounds.size.width / width).floor() as u16).max(2),
            cell_width: f32::from(width) as u16,
            cell_height: f32::from(height) as u16,
        };
        self.cell = new_size;
        let session = self.session.read(cx);
        let mut model = session.backend.model.lock().unwrap();
        model.palette = palette.clone();
        if model.size != new_size
            && self.requested_size != Some(new_size)
            && matches!(
                model.status,
                TerminalStatus::Starting | TerminalStatus::Running
            )
        {
            drop(model);
            if let Err(error) = session.resize(new_size) {
                self.input_error = Some(error);
            } else {
                self.requested_size = Some(new_size);
            }
            model = session.backend.model.lock().unwrap();
        }
        if model.size != new_size
            && !matches!(
                model.status,
                TerminalStatus::Starting | TerminalStatus::Running
            )
        {
            model.resize(new_size);
        }
        let mut frame = Frame {
            line_height: height,
            backgrounds: vec![fill(bounds, rgb(palette.background))],
            lines: vec![],
            cursor: None,
        };
        let content = model.term.renderable_content();
        let cursor = content.cursor;
        self.cursor_bounds = Bounds::new(
            point(
                bounds.left() + width * cursor.point.column.0 as f32,
                bounds.top()
                    + height * (cursor.point.line.0 + content.display_offset as i32) as f32,
            ),
            size(width, height),
        );
        for indexed in content.display_iter {
            let row = indexed.point.line.0 + content.display_offset as i32;
            if row < 0
                || row >= new_size.rows as i32
                || indexed.point.column.0 >= new_size.cols as usize
            {
                continue;
            }
            let cell = indexed.cell;
            let origin = point(
                bounds.left() + width * indexed.point.column.0 as f32,
                bounds.top() + height * row as f32,
            );
            let cell_bounds = Bounds::new(
                origin,
                size(
                    width
                        * if cell.flags.contains(Flags::WIDE_CHAR) {
                            2.
                        } else {
                            1.
                        },
                    height,
                ),
            );
            let mut fg = color(cell.fg, content.colors, &palette);
            let mut bg = color(cell.bg, content.colors, &palette);
            if cell.flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            if content
                .selection
                .is_some_and(|selection| selection.contains(indexed.point))
            {
                bg = palette.selection;
            }
            if bg != palette.background {
                frame.backgrounds.push(fill(cell_bounds, rgb(bg)));
            }
            if cell.flags.intersects(
                Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER | Flags::HIDDEN,
            ) {
                continue;
            }
            if cell.c == ' '
                && !cell
                    .flags
                    .intersects(Flags::ALL_UNDERLINES | Flags::STRIKEOUT)
            {
                continue;
            }
            let mut text = cell.c.to_string();
            if let Some(marks) = cell.zerowidth() {
                text.extend(marks);
            }
            let mut cell_font = font.clone();
            if cell.flags.contains(Flags::BOLD) {
                cell_font.weight = FontWeight::BOLD;
            }
            if cell.flags.contains(Flags::ITALIC) {
                cell_font.style = FontStyle::Italic;
            }
            if cell.flags.contains(Flags::DIM) {
                fg = blend(fg, bg);
            }
            let run = TextRun {
                len: text.len(),
                font: cell_font,
                color: rgb(fg).into(),
                background_color: None,
                underline: cell
                    .flags
                    .intersects(Flags::ALL_UNDERLINES)
                    .then_some(UnderlineStyle {
                        thickness: px(1.),
                        color: None,
                        wavy: cell.flags.contains(Flags::UNDERCURL),
                    }),
                strikethrough: cell.flags.contains(Flags::STRIKEOUT).then_some(
                    StrikethroughStyle {
                        thickness: px(1.),
                        color: None,
                    },
                ),
            };
            frame.lines.push((
                origin,
                window
                    .text_system()
                    .shape_line(text.into(), self.font_size, &[run], None),
            ));
        }
        if self.focus.is_focused(window) && cursor.shape != CursorShape::Hidden {
            let mut bounds = self.cursor_bounds;
            if bounds.top() >= self.bounds.top() && bounds.bottom() <= self.bounds.bottom() {
                match cursor.shape {
                    CursorShape::Beam => bounds.size.width = px(2.),
                    CursorShape::Underline => {
                        bounds.origin.y = bounds.bottom() - px(2.);
                        bounds.size.height = px(2.);
                    }
                    _ => {}
                }
                frame.cursor = Some(fill(bounds, rgba((palette.cursor << 8) | 0x80)));
            }
        }
        if !self.composition.is_empty() {
            let run = TextRun {
                len: self.composition.len(),
                font,
                color: rgb(palette.foreground).into(),
                background_color: Some(rgb(palette.background).into()),
                underline: Some(UnderlineStyle {
                    thickness: px(1.),
                    color: None,
                    wavy: false,
                }),
                strikethrough: None,
            };
            frame.lines.push((
                self.cursor_bounds.origin,
                window.text_system().shape_line(
                    self.composition.clone().into(),
                    self.font_size,
                    &[run],
                    None,
                ),
            ));
        }
        frame
    }
}
fn button_code(button: MouseButton) -> u8 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
        _ => 3,
    }
}
fn blend(a: u32, b: u32) -> u32 {
    ((a & 0xfefefe) >> 1) + ((b & 0xfefefe) >> 1)
}
fn color(
    color: Color,
    overrides: &alacritty_terminal::term::color::Colors,
    palette: &TerminalPalette,
) -> u32 {
    let color = match color {
        Color::Spec(rgb) => rgb,
        Color::Indexed(index) => {
            overrides[index as usize].unwrap_or_else(|| palette.indexed(index as usize))
        }
        Color::Named(name) => overrides[name].unwrap_or_else(|| palette.indexed(name as usize)),
    };
    ((color.r as u32) << 16) | ((color.g as u32) << 8) | color.b as u32
}
struct Frame {
    line_height: Pixels,
    backgrounds: Vec<PaintQuad>,
    lines: Vec<(Point<Pixels>, ShapedLine)>,
    cursor: Option<PaintQuad>,
}
impl Focusable for TerminalView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for TerminalView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity();
        let input = owner.clone();
        let focus = self.focus.clone();
        let status = self.session.read(cx).status();
        let message = self
            .input_error
            .clone()
            .or_else(|| (status != TerminalStatus::Running).then(|| status.to_string()));
        div()
            .id("terminal-view")
            .test_support()
            .size_full()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .key_context("PintleTerminal")
            .track_focus(&self.focus)
            .role(Role::Terminal)
            .aria_label("Terminal")
            .cursor(CursorStyle::IBeam)
            // Keymap actions run before on_key_down; consume these here so Root's
            // focus traversal never takes Tab away from the shell.
            .on_action(cx.listener(|this, _: &input::Tab, _, cx| {
                this.send(b"\t".to_vec(), cx);
            }))
            .on_action(cx.listener(|this, _: &input::BackTab, _, cx| {
                this.send(b"\x1b[Z".to_vec(), cx);
            }))
            .on_action(cx.listener(|this, _: &input::Copy, _, cx| this.copy(cx)))
            .on_action(cx.listener(|this, _: &input::Paste, _, cx| {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    this.paste(&text, cx);
                }
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if let Some(bytes) = input::key_bytes(&event.keystroke, this.mode(cx)) {
                    this.send(bytes, cx);
                    window.prevent_default();
                    cx.stop_propagation();
                }
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_scroll(cx.listener(Self::scroll))
            .child(
                canvas(
                    move |bounds, window, cx| {
                        owner.update(cx, |view, cx| view.frame(bounds, window, cx))
                    },
                    move |bounds, frame: Frame, window, cx| {
                        window.handle_input(&focus, ElementInputHandler::new(bounds, input), cx);
                        let line_height = frame.line_height;
                        window.with_content_mask(Some(ContentMask { bounds }), |window| {
                            for quad in frame.backgrounds {
                                window.paint_quad(quad);
                            }
                            for (origin, line) in frame.lines {
                                let _ = line.paint(
                                    origin,
                                    line_height,
                                    TextAlign::Left,
                                    None,
                                    window,
                                    cx,
                                );
                            }
                            if let Some(cursor) = frame.cursor {
                                window.paint_quad(cursor);
                            }
                        });
                    },
                )
                .size_full(),
            )
            .when_some(message, |body, message| {
                body.child(
                    div()
                        .id("terminal-status")
                        .test_support()
                        .flex_shrink_0()
                        .px_2()
                        .text_xs()
                        .child(message),
                )
            })
    }
}

fn utf8_offset(text: &str, utf16: usize) -> usize {
    let mut units = 0;
    for (offset, ch) in text.char_indices() {
        if units >= utf16 {
            return offset;
        }
        units += ch.len_utf16();
    }
    text.len()
}
impl EntityInputHandler for TerminalView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let start = utf8_offset(&self.composition, range.start);
        let end = utf8_offset(&self.composition, range.end);
        *adjusted = Some(
            self.composition[..start].encode_utf16().count()
                ..self.composition[..end].encode_utf16().count(),
        );
        Some(self.composition[start..end].to_owned())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.composition_selection.clone(),
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        (!self.composition.is_empty()).then(|| 0..self.composition.encode_utf16().count())
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let text = std::mem::take(&mut self.composition);
        self.composition_selection = 0..0;
        self.send(text.into_bytes(), cx);
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut committed = std::mem::take(&mut self.composition);
        if let Some(range) = range {
            let start = utf8_offset(&committed, range.start);
            let end = utf8_offset(&committed, range.end);
            committed.replace_range(start..end, text);
        } else {
            committed = text.to_owned();
        }
        self.composition_selection = 0..0;
        self.send(committed.into_bytes(), cx);
        window.notify_text_input_changed(&self.focus);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range.unwrap_or(0..self.composition.encode_utf16().count());
        let start = utf8_offset(&self.composition, range.start);
        let end = utf8_offset(&self.composition, range.end);
        self.composition.replace_range(start..end, text);
        self.composition_selection = selected
            .map(|r| (range.start + r.start)..(range.start + r.end))
            .unwrap_or_else(|| {
                let end = range.start + text.encode_utf16().count();
                end..end
            });
        window.notify_text_input_changed(&self.focus);
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(self.cursor_bounds)
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.composition_selection.end)
    }
    fn paste(&mut self, item: ClipboardItem, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = item.text() {
            TerminalView::paste(self, &text, cx);
        }
    }
}
