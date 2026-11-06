//! Retained document workspace. Kit owns the layout; Pintle owns document policy.
mod navigation;
mod skin;
mod tab_drop;
use gpui::TestSupportExt as _;
use gpui::component::dock::{
    BasePanel, DockArea, DockAreaRenderer, DockEvent, DockPlacement, DockSkin, InsertTarget,
    NodeId, PaneRef, Panel, PanelEvent, PanelId, panel_handle,
};
use gpui::component::{ActiveTheme, Placement};
use gpui::{
    AnyElement, AnyView, App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    IntoElement, KeyDownEvent, MouseButton, Render, SharedString, Subscription, WeakEntity, Window,
    div, prelude::*,
};
use gpui_kit as gpui;
pub use navigation::{
    NavigationContextMenuBuilder, NavigationDecoration, NavigationEvent, NavigationNode,
    NavigationTree, NavigationTreeOptions,
};
use std::{collections::HashMap, rc::Rc};

gpui::actions!(
    pintle_workspace,
    [
        CloseActive,
        NextTab,
        PreviousTab,
        KeepActive,
        SplitActiveRight,
        SplitActiveDown
    ]
);
struct WorkspaceBindings;
impl gpui::Global for WorkspaceBindings {}

/// Stable application identity, independent of the number of open views.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ResourceKey(pub SharedString);
impl From<&str> for ResourceKey {
    fn from(value: &str) -> Self {
        Self(value.to_owned().into())
    }
}
/// A live view identity, not a resource identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ViewId(pub PanelId);
#[derive(Clone, Debug)]
pub struct Document {
    pub key: ResourceKey,
    pub title: SharedString,
    pub context: SharedString,
    /// Optional consumer-supplied asset path for the tab icon.
    pub icon_path: Option<SharedString>,
}
impl Document {
    pub fn new(
        key: impl Into<ResourceKey>,
        title: impl Into<SharedString>,
        context: impl Into<SharedString>,
    ) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            context: context.into(),
            icon_path: None,
        }
    }
    pub fn icon_path(mut self, path: impl Into<SharedString>) -> Self {
        self.icon_path = Some(path.into());
        self
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenMode {
    Preview,
    Permanent,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitDirection {
    Right,
    Down,
}
/// Validation policy for an application-confirmed batch removal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosePolicy {
    Validate,
    Discard,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseScope {
    One,
    Others,
    ToRight,
    All,
}
type DeactivateGuard = Rc<dyn Fn(&mut Window, &mut App) -> bool>;

/// A retained application view and its focus handle, with optional edit validation.
pub struct WorkspacePage {
    pub view: AnyView,
    pub focus: FocusHandle,
    before_deactivate: Option<DeactivateGuard>,
}
impl WorkspacePage {
    pub fn new<V: Render + Focusable>(view: Entity<V>, cx: &App) -> Self {
        Self {
            focus: view.focus_handle(cx),
            view: view.into(),
            before_deactivate: None,
        }
    }
}
impl WorkspacePage {
    /// Validate or commit pending edits before changing/closing a page. Returning
    /// false keeps the workspace unchanged; the callback should focus its error.
    pub fn before_deactivate(
        mut self,
        guard: impl Fn(&mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.before_deactivate = Some(Rc::new(guard));
        self
    }
}
pub type ViewFactory = Rc<dyn Fn(&Document, &mut Window, &mut App) -> WorkspacePage>;
#[derive(Clone, Debug)]
pub struct GroupSnapshot {
    pub id: NodeId,
    pub views: Vec<ViewId>,
    pub active: Option<ViewId>,
}

pub struct Workspace {
    dock: Entity<DockArea>,
    factory: ViewFactory,
    panels: HashMap<ViewId, Entity<DocumentPanel>>,
    active_group: Option<NodeId>,
    focus: FocusHandle,
    _subscription: Subscription,
}
impl Workspace {
    pub fn new(factory: ViewFactory, window: &mut Window, cx: &mut Context<Self>) -> Self {
        if !cx.has_global::<WorkspaceBindings>() {
            cx.set_global(WorkspaceBindings);
            cx.bind_keys([
                gpui::KeyBinding::new("cmd-w", CloseActive, Some("PintleWorkspace")),
                gpui::KeyBinding::new("ctrl-w", CloseActive, Some("PintleWorkspace")),
                gpui::KeyBinding::new("ctrl-pagedown", NextTab, Some("PintleWorkspace")),
                gpui::KeyBinding::new("ctrl-pageup", PreviousTab, Some("PintleWorkspace")),
                gpui::KeyBinding::new("cmd-alt-right", NextTab, Some("PintleWorkspace")),
                gpui::KeyBinding::new("cmd-alt-left", PreviousTab, Some("PintleWorkspace")),
                gpui::KeyBinding::new("cmd-shift-enter", KeepActive, Some("PintleWorkspace")),
                gpui::KeyBinding::new("ctrl-shift-enter", KeepActive, Some("PintleWorkspace")),
            ]);
        }
        let owner = cx.weak_entity();
        let dock = cx.new(|cx| {
            let base = DockSkin::new(cx);
            let renderer = Rc::new(skin::WorkspaceSkin { base, owner });
            DockArea::new("pintle-workspace", None, window, cx).with_renderer(renderer)
        });
        let subscription = cx.subscribe_in(&dock, window, |this, _, _: &DockEvent, window, cx| {
            // The layout is the source of truth, including mutations made by Kit's drop targets.
            let groups = this.groups(cx);
            this.panels
                .retain(|id, _| groups.iter().any(|g| g.views.contains(id)));
            // Dropping into a group that already displays the same resource keeps
            // the activated (moved) entity and releases the destination's old view.
            for group in &groups {
                let mut by_resource = HashMap::new();
                let ordered = group
                    .active
                    .iter()
                    .chain(group.views.iter().filter(|id| Some(**id) != group.active));
                for id in ordered {
                    if let Some(document) = this.document(*id, cx)
                        && by_resource.insert(document.key, *id).is_some()
                        && let Some(panel) = this.panels.remove(id)
                    {
                        this.dock
                            .update(cx, |dock, cx| dock.remove_panel(panel, window, cx));
                    }
                }
            }
            if !groups.iter().any(|g| Some(g.id) == this.active_group) {
                this.active_group = groups.first().map(|g| g.id);
            }
            cx.notify();
        });
        Self {
            dock,
            factory,
            panels: HashMap::new(),
            active_group: None,
            focus: cx.focus_handle(),
            _subscription: subscription,
        }
    }
    /// Upstream escape hatch. Layout mutation remains owned by DockArea.
    pub fn dock(&self) -> &Entity<DockArea> {
        &self.dock
    }
    pub fn groups(&self, cx: &App) -> Vec<GroupSnapshot> {
        let mut groups = Vec::new();
        if let Some(tree) = self.dock.read(cx).layout(DockPlacement::Center) {
            tree.root().walk(&mut |node| {
                if let PaneRef::Tabs { panels, active_ix } = node.kind()
                    && !panels.is_empty()
                {
                    groups.push(GroupSnapshot {
                        id: node.id(),
                        views: panels.iter().copied().map(ViewId).collect(),
                        active: panels.get(active_ix).copied().map(ViewId),
                    });
                }
            });
        }
        groups
    }
    pub fn active_group(&self) -> Option<NodeId> {
        self.active_group
    }
    pub fn activate_group(&mut self, node: NodeId, cx: &mut Context<Self>) {
        if self.active_group == Some(node) {
            return;
        }
        self.active_group = Some(node);
        cx.notify();
    }
    pub fn document(&self, id: ViewId, cx: &App) -> Option<Document> {
        self.panels.get(&id).map(|p| p.read(cx).document.clone())
    }
    pub fn page(&self, id: ViewId, cx: &App) -> Option<AnyView> {
        self.panels.get(&id).map(|p| p.read(cx).page.view.clone())
    }
    pub fn is_preview(&self, id: ViewId, cx: &App) -> bool {
        self.panels.get(&id).is_some_and(|p| p.read(cx).preview)
    }
    /// Focus destination when another workbench region closes.
    pub fn active_focus_handle(&self, cx: &App) -> FocusHandle {
        self.active_view(cx)
            .and_then(|id| self.panels.get(&id))
            .map(|panel| panel.read(cx).page.focus.clone())
            .unwrap_or_else(|| self.focus.clone())
    }
    pub fn active_view(&self, cx: &App) -> Option<ViewId> {
        let groups = self.groups(cx);
        groups
            .iter()
            .find(|g| Some(g.id) == self.active_group)
            .or(groups.first())
            .and_then(|g| g.active)
    }
    pub fn keep_open(&mut self, id: ViewId, cx: &mut Context<Self>) {
        if let Some(panel) = self.panels.get(&id) {
            panel.update(cx, |p, cx| {
                p.preview = false;
                cx.notify();
            });
        }
        cx.notify();
    }
    fn prepare_pages(&self, window: &mut Window, cx: &mut App) -> bool {
        self.prepare_other_pages(None, window, cx)
    }
    fn prepare_other_pages(
        &self,
        except: Option<ViewId>,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let guards: Vec<_> = self
            .panels
            .iter()
            .filter(|(id, _)| Some(**id) != except)
            .map(|(_, panel)| panel)
            .filter_map(|panel| panel.read(cx).page.before_deactivate.clone())
            .collect();
        guards.into_iter().all(|guard| guard(window, cx))
    }
    pub fn select(&mut self, id: ViewId, window: &mut Window, cx: &mut Context<Self>) {
        if !self.prepare_pages(window, cx) {
            return;
        }
        self.select_unchecked(id, window, cx);
    }
    fn select_unchecked(&mut self, id: ViewId, window: &mut Window, cx: &mut Context<Self>) {
        let mut changed = true;
        if let Some(group) = self.groups(cx).iter().find(|g| g.views.contains(&id)) {
            changed = self.active_group != Some(group.id) || group.active != Some(id);
            self.active_group = Some(group.id);
        }
        if changed {
            self.dock
                .update(cx, |dock, cx| dock.select_panel(id.0, window, cx));
        }
        if let Some(panel) = self.panels.get(&id) {
            let focus = panel.read(cx).page.focus.clone();
            window.focus(&focus, cx);
        }
        if changed {
            cx.notify();
        }
    }
    pub fn open(
        &mut self,
        document: Document,
        mode: OpenMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> ViewId {
        if let Some(active) = self.active_view(cx)
            && !self.prepare_pages(window, cx)
        {
            return active;
        }
        let groups = self.groups(cx);
        let group = groups
            .iter()
            .find(|g| Some(g.id) == self.active_group)
            .or(groups.first());
        if let Some(existing) = group
            .and_then(|g| {
                g.views.iter().find(|id| {
                    self.document(**id, cx)
                        .is_some_and(|d| d.key == document.key)
                })
            })
            .copied()
        {
            if mode == OpenMode::Permanent {
                self.keep_open(existing, cx);
            }
            self.select(existing, window, cx);
            return existing;
        }
        let replacing = group
            .and_then(|g| g.views.iter().find(|id| self.is_preview(**id, cx)))
            .copied()
            .filter(|_| mode == OpenMode::Preview);
        let target = group.map(|g| {
            (
                g.id,
                replacing.and_then(|id| g.views.iter().position(|v| *v == id)),
            )
        });
        // Insert first so replacing the last preview doesn't collapse its group.
        let id = self.create(document, mode, window, cx);
        if let Some((node, index)) = target {
            self.dock.update(cx, |dock, cx| {
                dock.move_panel(
                    id.0,
                    InsertTarget::Tabs {
                        node,
                        ix: index,
                        activate: true,
                    },
                    window,
                    cx,
                )
            });
        }
        if let Some(old) = replacing {
            self.close(old, CloseScope::One, window, cx);
        }
        self.select(id, window, cx);
        id
    }
    fn create(
        &mut self,
        document: Document,
        mode: OpenMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> ViewId {
        let page = (self.factory)(&document, window, cx);
        let owner = cx.weak_entity();
        let panel = cx.new(|cx| {
            let focus_owner = owner.clone();
            let id = ViewId(cx.entity_id().into());
            let subscription = cx.on_focus_in(&page.focus, window, move |_, _, cx| {
                let _ = focus_owner.update(cx, |workspace, cx| {
                    if let Some(group) = workspace.groups(cx).iter().find(|g| g.views.contains(&id))
                    {
                        workspace.activate_group(group.id, cx);
                    }
                });
            });
            DocumentPanel {
                document,
                page,
                preview: mode == OpenMode::Preview,
                owner,
                _focus_subscription: subscription,
            }
        });
        let id = ViewId(panel.entity_id().into());
        self.panels.insert(id, panel.clone());
        self.dock.update(cx, |dock, cx| {
            dock.add_panel_view(panel_handle(panel), DockPlacement::Center, None, window, cx)
        });
        id
    }
    /// Update all retained tab metadata without replacing page entities.
    pub fn update_document_metadata(&mut self, document: &Document, cx: &mut Context<Self>) {
        let mut changed = false;
        for panel in self.panels.values() {
            if panel.read(cx).document.key == document.key
                && (panel.read(cx).document.title != document.title
                    || panel.read(cx).document.context != document.context
                    || panel.read(cx).document.icon_path != document.icon_path)
            {
                panel.update(cx, |panel, cx| {
                    panel.document = document.clone();
                    cx.notify();
                });
                changed = true;
            }
        }
        if changed {
            cx.notify();
        }
    }
    /// Open a document beside the active group, without an extra source tab.
    pub fn open_in_split(
        &mut self,
        document: Document,
        direction: SplitDirection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<ViewId> {
        if !self.prepare_pages(window, cx) {
            return None;
        }
        let node = self
            .active_group
            .or_else(|| self.groups(cx).first().map(|group| group.id));
        let Some(node) = node else {
            return Some(self.open(document, OpenMode::Permanent, window, cx));
        };
        let new = self.create(document, OpenMode::Permanent, window, cx);
        self.dock.update(cx, |dock, cx| {
            dock.move_panel(
                new.0,
                InsertTarget::Split {
                    node,
                    placement: match direction {
                        SplitDirection::Right => Placement::Right,
                        SplitDirection::Down => Placement::Bottom,
                    },
                    size: None,
                },
                window,
                cx,
            )
        });
        self.select_unchecked(new, window, cx);
        Some(new)
    }
    /// Close all views of these keys atomically. Discard skips only their guards;
    /// every unrelated page must still pass validation before any view is removed.
    pub fn close_resources(
        &mut self,
        keys: &[ResourceKey],
        policy: ClosePolicy,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let guards: Vec<_> = self
            .panels
            .values()
            .filter(|panel| {
                policy == ClosePolicy::Validate || !keys.contains(&panel.read(cx).document.key)
            })
            .filter_map(|panel| panel.read(cx).page.before_deactivate.clone())
            .collect();
        if !guards.into_iter().all(|guard| guard(window, cx)) {
            return false;
        }
        let ids: Vec<_> = self
            .panels
            .iter()
            .filter(|(_, panel)| keys.contains(&panel.read(cx).document.key))
            .map(|(id, _)| *id)
            .collect();
        if ids.is_empty() {
            return true;
        }
        for id in ids {
            if let Some(panel) = self.panels.remove(&id) {
                self.dock
                    .update(cx, |dock, cx| dock.remove_panel(panel, window, cx));
            }
        }
        let groups = self.groups(cx);
        if !groups
            .iter()
            .any(|group| Some(group.id) == self.active_group)
        {
            self.active_group = groups.first().map(|group| group.id);
        }
        if let Some(id) = self.active_view(cx) {
            self.select_unchecked(id, window, cx);
        } else {
            window.focus(&self.focus, cx);
        }
        cx.notify();
        true
    }
    /// Split duplicates the resource, never its view state.
    pub fn split(
        &mut self,
        id: ViewId,
        direction: SplitDirection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<ViewId> {
        if !self.prepare_pages(window, cx) {
            return None;
        }
        let document = self.document(id, cx)?;
        let node = self.groups(cx).iter().find(|g| g.views.contains(&id))?.id;
        self.keep_open(id, cx);
        let new = self.create(document, OpenMode::Permanent, window, cx);
        self.dock.update(cx, |dock, cx| {
            dock.move_panel(
                new.0,
                InsertTarget::Split {
                    node,
                    placement: match direction {
                        SplitDirection::Right => Placement::Right,
                        SplitDirection::Down => Placement::Bottom,
                    },
                    size: None,
                },
                window,
                cx,
            )
        });
        self.select(new, window, cx);
        Some(new)
    }
    /// Move the existing entity; filter, selection, scroll and application state survive.
    pub fn move_view(
        &mut self,
        id: ViewId,
        target: NodeId,
        index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.keep_open(id, cx);
        self.dock.update(cx, |dock, cx| {
            dock.move_panel(
                id.0,
                InsertTarget::Tabs {
                    node: target,
                    ix: index,
                    activate: true,
                },
                window,
                cx,
            )
        });
        self.select_unchecked(id, window, cx);
    }
    fn cycle_tab(&mut self, offset: isize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(group) = self
            .groups(cx)
            .into_iter()
            .find(|g| Some(g.id) == self.active_group)
            && let Some(ix) = group.views.iter().position(|v| Some(*v) == group.active)
        {
            let next = (ix as isize + offset).rem_euclid(group.views.len() as isize) as usize;
            self.select(group.views[next], window, cx);
        }
    }
    pub fn close(
        &mut self,
        id: ViewId,
        scope: CloseScope,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.prepare_pages(window, cx) {
            return;
        }
        let Some(group) = self.groups(cx).into_iter().find(|g| g.views.contains(&id)) else {
            return;
        };
        let index = group.views.iter().position(|v| *v == id).unwrap();
        let closing: Vec<_> = group
            .views
            .iter()
            .enumerate()
            .filter(|(i, v)| match scope {
                CloseScope::One => **v == id,
                CloseScope::Others => **v != id,
                CloseScope::ToRight => *i > index,
                CloseScope::All => true,
            })
            .map(|(_, v)| *v)
            .collect();
        let next = group.active.filter(|v| !closing.contains(v)).or_else(|| {
            group
                .views
                .iter()
                .skip(index + 1)
                .chain(group.views[..index].iter().rev())
                .find(|v| !closing.contains(v))
                .copied()
        });
        for id in closing {
            if let Some(panel) = self.panels.remove(&id) {
                self.dock
                    .update(cx, |dock, cx| dock.remove_panel(panel, window, cx));
            }
        }
        if let Some(next) = next {
            self.select(next, window, cx);
        } else {
            let groups = self.groups(cx);
            self.active_group = groups.first().map(|g| g.id);
            if let Some(next) = self.active_view(cx) {
                self.select(next, window, cx);
            } else {
                window.focus(&self.focus, cx);
            }
        }
        cx.notify();
    }
}
impl Focusable for Workspace {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for Workspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("workspace")
            .size_full()
            .min_w_0()
            .min_h_0()
            .track_focus(&self.focus)
            .key_context("PintleWorkspace")
            .on_key_down(|event, window, cx| {
                if event.keystroke.key == "escape" && cx.stop_active_drag(window) {
                    cx.stop_propagation();
                    window.refresh();
                }
            })
            .on_action(cx.listener(|this, _: &CloseActive, window, cx| {
                if let Some(id) = this.active_view(cx) {
                    this.close(id, CloseScope::One, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &NextTab, window, cx| this.cycle_tab(1, window, cx)))
            .on_action(
                cx.listener(|this, _: &PreviousTab, window, cx| this.cycle_tab(-1, window, cx)),
            )
            .on_action(cx.listener(|this, _: &KeepActive, _, cx| {
                if let Some(id) = this.active_view(cx) {
                    this.keep_open(id, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &SplitActiveRight, window, cx| {
                if let Some(id) = this.active_view(cx) {
                    this.split(id, SplitDirection::Right, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &SplitActiveDown, window, cx| {
                if let Some(id) = this.active_view(cx) {
                    this.split(id, SplitDirection::Down, window, cx);
                }
            }))
            .when(self.panels.is_empty(), |d| {
                d.child(
                    div()
                        .id("empty-workspace")
                        .test_support()
                        .border_1()
                        .border_color(gpui::transparent_black())
                        .rounded(gpui::px(8.))
                        .relative()
                        .overflow_hidden()
                        .child(pintle_gpui_components::rounded_frame_overlay(
                            gpui::px(8.),
                            cx.theme().sidebar,
                            cx.theme().border,
                        ))
                        .bg(cx.theme().background)
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child("Open a resource from the explorer"),
                )
            })
            .when(!self.panels.is_empty(), |d| d.child(self.dock.clone()))
    }
}
struct DocumentPanel {
    _focus_subscription: Subscription,
    document: Document,
    page: WorkspacePage,
    preview: bool,
    owner: WeakEntity<Workspace>,
}
impl EventEmitter<PanelEvent> for DocumentPanel {}
impl BasePanel for DocumentPanel {
    fn panel_name(&self) -> &'static str {
        "PintleDocument"
    }
}
impl Panel for DocumentPanel {
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
impl Focusable for DocumentPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.page.focus.clone()
    }
}
impl Render for DocumentPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let id = ViewId(cx.entity_id().into());
        let owner = self.owner.clone();
        let guard = owner.clone();
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .capture_any_mouse_down(move |event: &gpui::MouseDownEvent, window, cx| {
                if event.button != MouseButton::Left {
                    return;
                }
                let accepted = guard
                    .update(cx, |w, cx| w.prepare_other_pages(Some(id), window, cx))
                    .unwrap_or(false);
                if !accepted {
                    cx.stop_propagation();
                }
            })
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                let _ = owner.update(cx, |w, cx| {
                    if let Some(group) = w.groups(cx).iter().find(|g| g.views.contains(&id)) {
                        w.activate_group(group.id, cx);
                    }
                });
            })
            .child(self.page.view.clone())
    }
}
