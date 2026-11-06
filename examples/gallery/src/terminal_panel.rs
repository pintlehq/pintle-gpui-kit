//! Gallery policy for a collection of independent, retained terminal sessions.
use super::*;
use gpui::assets::IconName;
use gpui::component::menu::{DropdownMenu, PopupMenuItem};
use gpui::{Focusable, Subscription};
use pintle_gpui_kit::terminal::{TerminalOptions, TerminalSession, TerminalView};

struct Session {
    id: usize,
    session: Entity<TerminalSession>,
    view: Entity<TerminalView>,
    _observe: Subscription,
}
pub(crate) struct TerminalPanel {
    sessions: Vec<Session>,
    active: Option<usize>,
    next_id: usize,
    initialized: bool,
    focus: gpui::FocusHandle,
}
impl TerminalPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            sessions: vec![],
            active: None,
            next_id: 1,
            initialized: false,
            focus: cx.focus_handle(),
        }
    }
    #[cfg(test)]
    pub(crate) fn session_count(&self) -> usize {
        self.sessions.len()
    }
    #[cfg(test)]
    pub(crate) fn active_test_session(&self) -> Option<Entity<TerminalSession>> {
        self.active_session().map(|s| s.session.clone())
    }
    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.initialized {
            self.new_session(window, cx);
        } else {
            self.focus_active(window, cx);
        }
    }
    pub fn new_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.initialized = true;
        let id = self.next_id;
        self.next_id += 1;
        #[cfg(not(test))]
        let session = cx.new(|cx| TerminalSession::new(TerminalOptions::default(), cx));
        #[cfg(test)]
        let session = cx.new(|cx| TerminalSession::new_test(TerminalOptions::default(), cx));
        let view = cx.new(|cx| TerminalView::new(session.clone(), window, cx));
        view.update(cx, |view, cx| view.set_font("JetBrains Mono", px(13.), cx));
        let observe = cx.observe(&session, |_, _, cx| cx.notify());
        self.sessions.push(Session {
            id,
            session,
            view,
            _observe: observe,
        });
        self.select(id, window, cx);
    }
    fn select(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.active = Some(id);
        self.focus_active(window, cx);
        cx.notify();
    }
    pub fn focus_active(&self, window: &mut Window, cx: &mut App) {
        if let Some(session) = self.active_session() {
            window.focus(&session.view.focus_handle(cx), cx);
        } else {
            window.focus(&self.focus, cx);
        }
    }
    pub fn is_focused(&self, window: &Window, cx: &App) -> bool {
        self.focus.is_focused(window)
            || self
                .active_session()
                .is_some_and(|s| s.view.read(cx).is_focused(window))
    }
    fn active_session(&self) -> Option<&Session> {
        self.sessions.iter().find(|s| Some(s.id) == self.active)
    }
    fn close_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.sessions.iter().position(|s| Some(s.id) == self.active) {
            let session = self.sessions.remove(index);
            session.session.update(cx, |s, cx| s.close(cx));
            self.active = self
                .sessions
                .get(index.min(self.sessions.len().saturating_sub(1)))
                .map(|s| s.id);
            self.focus_active(window, cx);
            cx.notify();
        }
    }
}
impl Focusable for TerminalPanel {
    fn focus_handle(&self, _: &App) -> gpui::FocusHandle {
        self.focus.clone()
    }
}
impl Render for TerminalPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sessions: Vec<_> = self
            .sessions
            .iter()
            .map(|s| {
                let title = s.session.read(cx).title();
                (
                    s.id,
                    if title.is_empty() {
                        format!("Terminal {}", s.id)
                    } else {
                        format!("{} · {title}", s.id)
                    },
                )
            })
            .collect();
        let selected = sessions
            .iter()
            .find(|s| Some(s.0) == self.active)
            .map(|s| s.1.clone())
            .unwrap_or_else(|| "No sessions".into());
        let owner = cx.entity().downgrade();
        div()
            .id("terminal-panel")
            .test_support()
            .size_full()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .key_context("PintleTerminalPanel")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &CloseTerminal, window, cx| {
                this.close_active(window, cx);
            }))
            .child(
                div()
                    .h(px(30.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_2()
                    .child(
                        Button::new("terminal-session-picker")
                            .ghost()
                            .small()
                            .label(selected)
                            .disabled(self.sessions.is_empty())
                            .accessibility_label("Terminal sessions")
                            .dropdown_menu(move |mut menu, _, _| {
                                for (id, title) in &sessions {
                                    let id = *id;
                                    let owner = owner.clone();
                                    menu = menu.item(PopupMenuItem::new(title.clone()).on_click(
                                        move |_, window, cx| {
                                            let _ = owner.update(cx, |panel, cx| {
                                                panel.select(id, window, cx)
                                            });
                                        },
                                    ));
                                }
                                menu
                            }),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("terminal-new")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Plus)
                            .accessibility_label("New terminal")
                            .tooltip("New terminal")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.new_session(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("terminal-restart")
                            .ghost()
                            .xsmall()
                            .label("Restart")
                            .disabled(self.active.is_none())
                            .accessibility_label("Restart terminal")
                            .on_click(cx.listener(|this, _, window, cx| {
                                if let Some(session) = this.active_session() {
                                    session.session.update(cx, |s, cx| s.restart(cx));
                                }
                                this.focus_active(window, cx);
                            })),
                    )
                    .child(
                        Button::new("terminal-close-session")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Trash)
                            .disabled(self.active.is_none())
                            .accessibility_label("Close terminal session")
                            .tooltip("Close terminal session")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.close_active(window, cx)),
                            ),
                    ),
            )
            .child(if let Some(session) = self.active_session() {
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(session.view.clone())
                    .into_any_element()
            } else {
                div()
                    .id("terminal-empty")
                    .test_support()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Button::new("terminal-create-first")
                            .ghost()
                            .label("New Terminal")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.new_session(window, cx)),
                            ),
                    )
                    .into_any_element()
            })
    }
}
