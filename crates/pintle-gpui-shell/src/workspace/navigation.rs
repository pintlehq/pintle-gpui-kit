use super::*;
use gpui::component::menu::PopupMenu;
use gpui::component::{
    ActiveTheme, Icon, IconName, Sizable,
    input::{Input, InputEvent, InputState},
    list::ListItem,
    tree::{Tree, TreeEvent, TreeItem, TreeState},
};
use gpui::px;
use pintle_gpui_components::context_menu::{ContextMenuState, is_context_menu_key};
use std::cell::RefCell;
/// Optional application-owned menu, receiving the stable node ID.
pub type NavigationContextMenuBuilder =
    Rc<dyn Fn(SharedString, PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu>;
use std::collections::{HashMap, HashSet};

/// Caller-owned trailing content, keyed by stable navigation ID.
pub type NavigationDecoration = Rc<dyn Fn(&SharedString, &mut App) -> gpui::AnyElement>;

/// Optional presentation settings. Defaults preserve the compact workspace tree.
#[derive(Clone)]
pub struct NavigationTreeOptions {
    pub row_height: gpui::Pixels,
    pub font_family: Option<SharedString>,
    pub initial_expanded: Option<HashSet<SharedString>>,
    pub decoration: Option<NavigationDecoration>,
}
impl Default for NavigationTreeOptions {
    fn default() -> Self {
        Self {
            row_height: px(22.),
            font_family: None,
            initial_expanded: None,
            decoration: None,
        }
    }
}

#[derive(Clone)]
pub struct NavigationNode {
    pub id: SharedString,
    pub label: SharedString,
    pub icon: Icon,
    pub document: Option<Document>,
    pub children: Vec<Self>,
}
impl NavigationNode {
    pub fn folder(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        children: Vec<Self>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: IconName::Folder.into(),
            document: None,
            children,
        }
    }
    pub fn resource(document: Document) -> Self {
        Self {
            id: document.key.0.clone(),
            label: document.title.clone(),
            icon: IconName::File.into(),
            document: Some(document),
            children: Vec::new(),
        }
    }
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = icon.into();
        self
    }
}
#[derive(Clone, Debug)]
pub struct NavigationEvent {
    pub document: Document,
    pub mode: OpenMode,
}
#[derive(Clone)]
struct NodeMetadata {
    icon: Icon,
    document: Option<Document>,
}

/// Search keeps matching ancestors and restores expansion when the query clears.
pub struct NavigationTree {
    options: NavigationTreeOptions,
    tree_focus: FocusHandle,
    context_menu: Option<NavigationContextMenuBuilder>,
    menu_host: Entity<ContextMenuState>,
    bounds: Rc<RefCell<HashMap<SharedString, gpui::Bounds<gpui::Pixels>>>>,
    nodes: Vec<NavigationNode>,
    lookup: Rc<HashMap<SharedString, NodeMetadata>>,
    expanded: HashSet<SharedString>,
    query: String,
    pub state: Entity<TreeState>,
    pub search: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}
fn project(
    nodes: &[NavigationNode],
    query: &str,
    expanded: &HashSet<SharedString>,
) -> Vec<TreeItem> {
    nodes
        .iter()
        .filter_map(|n| {
            let matches = query.is_empty()
                || n.label.to_lowercase().contains(query)
                || n.document
                    .as_ref()
                    .is_some_and(|d| d.context.to_lowercase().contains(query));
            let children = project(&n.children, if matches { "" } else { query }, expanded);
            if !matches && children.is_empty() {
                return None;
            }
            Some(
                TreeItem::new(n.id.clone(), n.label.clone())
                    .children(children)
                    .expanded(if query.is_empty() {
                        expanded.contains(&n.id)
                    } else {
                        true
                    }),
            )
        })
        .collect()
}
impl NavigationTree {
    pub fn new(nodes: Vec<NavigationNode>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_options(nodes, NavigationTreeOptions::default(), window, cx)
    }
    pub fn with_options(
        nodes: Vec<NavigationNode>,
        options: NavigationTreeOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        fn index(
            nodes: &[NavigationNode],
            map: &mut HashMap<SharedString, NodeMetadata>,
            expanded: &mut HashSet<SharedString>,
        ) {
            for n in nodes {
                map.insert(
                    n.id.clone(),
                    NodeMetadata {
                        icon: n.icon.clone(),
                        document: n.document.clone(),
                    },
                );
                if n.document.is_none() {
                    expanded.insert(n.id.clone());
                }
                index(&n.children, map, expanded);
            }
        }
        let mut lookup = HashMap::new();
        let mut expanded = HashSet::new();
        index(&nodes, &mut lookup, &mut expanded);
        if let Some(initial) = &options.initial_expanded {
            expanded = initial.clone();
        }
        let state = cx.new(|cx| TreeState::new(cx).items(project(&nodes, "", &expanded)));
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search resources"));
        let input = cx.subscribe(&search, |this, input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                let query = input.read(cx).value().to_string();
                this.set_query(&query, cx);
            }
        });
        let expand = cx.subscribe(&state, |this, _, event: &TreeEvent, cx| {
            if this.query.is_empty() {
                match event {
                    TreeEvent::Expanded(id) => {
                        this.expanded.insert(id.clone());
                    }
                    TreeEvent::Collapsed(id) => {
                        this.expanded.remove(id);
                    }
                }
            }
            cx.notify();
        });
        Self {
            options,
            tree_focus: cx.focus_handle(),
            context_menu: None,
            menu_host: cx.new(ContextMenuState::new),
            bounds: Rc::default(),
            nodes,
            lookup: Rc::new(lookup),
            expanded,
            query: String::new(),
            state,
            search,
            _subscriptions: vec![input, expand],
        }
    }
    /// Replace catalog metadata while retaining expansion and surviving selection.
    pub fn set_nodes(&mut self, nodes: Vec<NavigationNode>, cx: &mut Context<Self>) {
        fn path(nodes: &[NavigationNode], id: &SharedString) -> Option<Vec<SharedString>> {
            for node in nodes {
                if &node.id == id {
                    return Some(vec![id.clone()]);
                }
                if let Some(mut child) = path(&node.children, id) {
                    child.insert(0, node.id.clone());
                    return Some(child);
                }
            }
            None
        }
        fn index(nodes: &[NavigationNode], map: &mut HashMap<SharedString, NodeMetadata>) {
            for node in nodes {
                map.insert(
                    node.id.clone(),
                    NodeMetadata {
                        icon: node.icon.clone(),
                        document: node.document.clone(),
                    },
                );
                index(&node.children, map);
            }
        }
        let previous = self
            .state
            .read(cx)
            .selected_item()
            .and_then(|item| path(&self.nodes, &item.id));
        let mut lookup = HashMap::new();
        index(&nodes, &mut lookup);
        self.expanded.retain(|id| lookup.contains_key(id));
        let selected =
            previous.and_then(|path| path.into_iter().rev().find(|id| lookup.contains_key(id)));
        self.nodes = nodes;
        self.lookup = Rc::new(lookup);
        self.bounds.borrow_mut().clear();
        let items = project(&self.nodes, &self.query, &self.expanded);
        self.state.update(cx, |state, cx| {
            state.set_items(items, cx);
            state.set_selected_item(selected.map(|id| TreeItem::new(id, "")).as_ref(), cx);
        });
        cx.notify();
    }
    pub fn set_context_menu(&mut self, builder: NavigationContextMenuBuilder) {
        self.context_menu = Some(builder);
    }
    pub fn is_expanded(&self, id: &SharedString, cx: &App) -> bool {
        self.state
            .read(cx)
            .index_of(id)
            .and_then(|index| self.state.read(cx).entry(index))
            .is_some_and(|entry| entry.is_expanded())
    }
    pub fn toggle_node(&mut self, id: &SharedString, cx: &mut Context<Self>) {
        fn change(items: &mut [TreeItem], id: &SharedString, expanded: bool) {
            for item in items {
                if &item.id == id {
                    *item = item.clone().expanded(expanded);
                }
                change(&mut item.children, id, expanded);
            }
        }
        let expanded = !self.is_expanded(id, cx);
        if self.query.is_empty() {
            if expanded {
                self.expanded.insert(id.clone());
            } else {
                self.expanded.remove(id);
            }
        }
        let mut items = project(&self.nodes, &self.query, &self.expanded);
        change(&mut items, id, expanded);
        self.state
            .update(cx, |state, cx| state.set_items(items, cx));
        cx.notify();
    }
    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| state.focus(window, cx));
    }
    fn show_context_menu(
        &mut self,
        id: SharedString,
        position: gpui::Point<gpui::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.context_menu.is_none() {
            return;
        }
        self.state.update(cx, |state, cx| {
            state.set_selected_item(Some(&TreeItem::new(id.clone(), "")), cx)
        });
        if let Some(builder) = self.context_menu.clone() {
            self.menu_host.update(cx, |host, cx| {
                host.open(position, window, cx, move |menu, window, cx| {
                    builder(id, menu, window, cx)
                })
            });
        }
    }
    pub fn set_query(&mut self, query: &str, cx: &mut Context<Self>) {
        self.query = query.trim().to_lowercase();
        let items = project(&self.nodes, &self.query, &self.expanded);
        self.state.update(cx, |s, cx| s.set_items(items, cx));
        cx.notify();
    }
    pub fn open_selected(&mut self, mode: OpenMode, cx: &mut Context<Self>) {
        if let Some(doc) = self
            .state
            .read(cx)
            .selected_item()
            .and_then(|i| self.lookup.get(&i.id))
            .and_then(|n| n.document.clone())
        {
            cx.emit(NavigationEvent {
                document: doc,
                mode,
            });
        }
    }
}
impl EventEmitter<NavigationEvent> for NavigationTree {}

impl Render for NavigationTree {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.weak_entity();
        let lookup = self.lookup.clone();
        let bounds = self.bounds.clone();
        let focused = self.tree_focus.contains_focused(window, cx);
        let row_height = self.options.row_height;
        let decoration = self.options.decoration.clone();
        div()
            .id("navigation-tree")
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .text_size(px(13.))
            .when_some(self.options.font_family.clone(), |view, font| {
                view.font_family(font)
            })
            .child(
                div()
                    .p_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Input::new(&self.search)
                            .id("resource-search")
                            .aria_label("Search resources")
                            .small()
                            .map(|input| gpui::Styled::h(input, px(28.)))
                            .bg(cx.theme().background)
                            .text_size(px(12.))
                            .prefix(Icon::new(IconName::Search).size(px(14.))),
                    ),
            )
            .child(self.menu_host.clone())
            .child(
                div()
                    .track_focus(&self.tree_focus)
                    .flex_1()
                    .min_h_0()
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        if is_context_menu_key(event) {
                            if let Some(id) = this
                                .state
                                .read(cx)
                                .selected_item()
                                .map(|item| item.id.clone())
                            {
                                let position = this
                                    .bounds
                                    .borrow()
                                    .get(&id)
                                    .map(|bounds| bounds.bottom_left());
                                if let Some(position) = position {
                                    this.show_context_menu(id, position, window, cx);
                                    cx.stop_propagation();
                                }
                            }
                            return;
                        }
                        if event.keystroke.key == "enter" {
                            this.open_selected(OpenMode::Permanent, cx);
                        }
                    }))
                    .child(Tree::new(&self.state, move |_, entry, selected, _, cx| {
                        let node = lookup.get(&entry.item().id).cloned();
                        let owner = owner.clone();
                        let label = entry.item().label.clone();
                        let id = entry.item().id.clone();
                        let measure_id = id.clone();
                        let bounds = bounds.clone();
                        let right_owner = owner.clone();
                        let row =
                            ListItem::new(SharedString::from(format!("nav:{}", entry.item().id)))
                                .accessibility_label(label.clone())
                                .h(row_height)
                                .py_0()
                                .border_1()
                                .border_color(cx.theme().transparent)
                                .relative()
                                .rounded_none()
                                .text_color(if selected {
                                    cx.theme().foreground
                                } else {
                                    cx.theme().muted_foreground
                                })
                                .when(selected && focused, |row| {
                                    row.border_1().border_color(cx.theme().ring)
                                })
                                .pl(px(entry.depth() as f32 * 12. + 8.))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(6.))
                                        .relative()
                                        .group("tree-row")
                                        .w_full()
                                        .children((0..entry.depth()).map(|depth| {
                                            div()
                                                .absolute()
                                                .top(px(-2.))
                                                .bottom(px(-2.))
                                                .left(px((depth as f32 - entry.depth() as f32)
                                                    * 12.
                                                    + 7.))
                                                .w(px(1.))
                                                .bg(cx.theme().border)
                                                .opacity(if focused { 1. } else { 0. })
                                                .group_hover("tree-row", |line| line.opacity(1.))
                                        }))
                                        .when(!entry.is_folder(), |row| {
                                            row.child(div().w(px(16.)).flex_shrink_0())
                                        })
                                        .child(
                                            (if entry.is_folder() {
                                                Icon::new(if entry.is_expanded() {
                                                    IconName::ChevronDown
                                                } else {
                                                    IconName::ChevronRight
                                                })
                                            } else {
                                                node.as_ref()
                                                    .map(|n| n.icon.clone())
                                                    .unwrap_or_else(|| IconName::File.into())
                                            })
                                            .size(px(16.)),
                                        )
                                        .when(entry.is_folder(), |row| {
                                            row.child(
                                                node.as_ref()
                                                    .map(|n| n.icon.clone())
                                                    .unwrap_or_else(|| IconName::Folder.into())
                                                    .size(px(16.)),
                                            )
                                        })
                                        .child(
                                            div()
                                                .min_w_0()
                                                .flex_1()
                                                .whitespace_nowrap()
                                                .text_ellipsis()
                                                .child(label),
                                        )
                                        .when_some(decoration.as_ref(), |row, render| {
                                            row.child(render(&id, cx))
                                        }),
                                )
                                .on_click(move |event, _, cx| {
                                    if let Some(document) =
                                        node.as_ref().and_then(|n| n.document.clone())
                                    {
                                        let _ = owner.update(cx, |_, cx| {
                                            cx.emit(NavigationEvent {
                                                document,
                                                mode: if event.click_count() == 2 {
                                                    OpenMode::Permanent
                                                } else {
                                                    OpenMode::Preview
                                                },
                                            })
                                        });
                                    }
                                });
                        row.child(
                            gpui::canvas(
                                move |rect, _, _| {
                                    bounds.borrow_mut().insert(measure_id.clone(), rect);
                                },
                                |_, _, _, _| {},
                            )
                            .absolute()
                            .inset_0(),
                        )
                        .capture_any_mouse_down(
                            move |event: &gpui::MouseDownEvent, window, cx| {
                                if event.button == MouseButton::Right {
                                    cx.stop_propagation();
                                    let _ = right_owner.update(cx, |this, cx| {
                                        this.show_context_menu(
                                            id.clone(),
                                            event.position,
                                            window,
                                            cx,
                                        )
                                    });
                                }
                            },
                        )
                    })),
            )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_preserves_ancestors_without_mutating_expansion() {
        let nodes = vec![NavigationNode::folder(
            "db",
            "Database",
            vec![NavigationNode::folder(
                "schema",
                "public",
                vec![
                    NavigationNode::resource(Document::new("customers", "Customers", "demo")),
                    NavigationNode::resource(Document::new("orders", "Orders", "demo")),
                ],
            )],
        )];
        let expanded = HashSet::from(["db".into()]);
        let filtered = project(&nodes, "customers", &expanded);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].children[0].children.len(), 1);
        assert!(filtered[0].children[0].is_expanded());
        let restored = project(&nodes, "", &expanded);
        assert!(!restored[0].children[0].is_expanded());
        assert_eq!(restored[0].children[0].children.len(), 2);
    }
}
