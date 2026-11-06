//! Pointer and keyboard placement for Kit's accessible popup menu.
use gpui::component::menu::PopupMenu;
use gpui_kit::{
    self as gpui, Context, Entity, Focusable, IntoElement, Pixels, Point, Render, Subscription,
    Window, div, prelude::*, px,
};

/// Retained menu host. Builders run when opening, never on ordinary renders.
pub struct ContextMenuState {
    menu: Option<Entity<PopupMenu>>,
    position: Point<Pixels>,
    subscription: Option<Subscription>,
    previous_focus: Option<gpui::FocusHandle>,
    hitbox: std::rc::Rc<std::cell::Cell<Option<gpui::HitboxId>>>,
}
impl ContextMenuState {
    pub fn new(_: &mut Context<Self>) -> Self {
        Self {
            menu: None,
            position: Point::default(),
            subscription: None,
            previous_focus: None,
            hitbox: Default::default(),
        }
    }
    /// True only over the currently rendered popup, respecting clipping/occlusion.
    pub fn contains_pointer(&self, window: &Window) -> bool {
        self.hitbox
            .get()
            .is_some_and(|hitbox| hitbox.is_hovered(window))
    }
    pub fn open(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
        build: impl FnOnce(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
    ) {
        let previous = if self
            .menu
            .as_ref()
            .is_some_and(|menu| menu.focus_handle(cx).contains_focused(window, cx))
        {
            self.previous_focus.clone()
        } else {
            window.focused(cx)
        };
        let owner = cx.weak_entity();
        // Run builders after pointer dispatch releases tree/grid entity leases.
        window.defer(cx, move |window, cx| {
            if owner.upgrade().is_none() {
                return;
            }
            let menu = PopupMenu::build(window, cx, |menu, window, cx| {
                let menu = if let Some(focus) = previous.clone() {
                    menu.action_context(focus)
                } else {
                    menu
                };
                build(menu, window, cx)
            });
            if menu.read(cx).is_empty() {
                return;
            }
            let _ = owner.update(cx, |this, cx| {
                this.position = position;
                this.previous_focus = previous;
                this.subscription =
                    Some(cx.subscribe(&menu, |this, _, _: &gpui::DismissEvent, cx| {
                        this.menu = None;
                        this.previous_focus = None;
                        cx.notify();
                    }));
                this.menu = Some(menu.clone());
                cx.notify();
            });
            window.defer(cx, move |window, cx| {
                window.focus(&menu.focus_handle(cx), cx)
            });
        });
    }
}
impl Render for ContextMenuState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let hitbox = self.hitbox.clone();
        div().children(self.menu.as_ref().map(|menu| {
            gpui::deferred(
                gpui::anchored()
                    .position(self.position)
                    .snap_to_window_with_margin(px(8.))
                    .child(
                        div()
                            .on_children_prepainted(move |children, window, _| {
                                hitbox.set(children.first().map(|bounds| {
                                    window
                                        .insert_hitbox(*bounds, gpui::HitboxBehavior::Normal)
                                        .id
                                }));
                            })
                            .child(menu.clone()),
                    ),
            )
            .with_priority(gpui::base::POPUP_PRIORITY)
        }))
    }
}

/// Menu / Shift+F10, shared by tree and grid without taking over editor keys.
pub fn is_context_menu_key(event: &gpui::KeyDownEvent) -> bool {
    event.keystroke.key == "menu"
        || (event.keystroke.key == "f10" && event.keystroke.modifiers.shift)
}
