//! Editable in-memory fixture data; no database or Kubernetes client is involved.
use super::*;
use gpui::assets::IconName;
use gpui::component::Icon;
use gpui::component::{
    input::InputEvent,
    table::{Column, ColumnSort, TableState},
};
use gpui::{FocusHandle, Focusable};
use pintle_gpui_kit::components::table::{
    CellAddress, CellEditor, EditableTable, EditableTableAdapter, EditableTableDelegate,
    EditableTableEvent, EditableTableState,
};
use pintle_gpui_kit::shell::workspace::{Document, NavigationNode};
use std::cell::Cell;
use std::rc::Rc;

mod header;
mod menu;
mod store;
use std::collections::BTreeSet;
pub use store::DatasetStore;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceKind {
    Database,
    Kubernetes,
}

pub struct Dataset {
    pub document: Document,
    pub columns: Vec<&'static str>,
    pub kind: ResourceKind,
    pub rows: Vec<Vec<SharedString>>,
}
impl Dataset {
    pub fn fixtures() -> Vec<Rc<Self>> {
        [
            (
                "db/local/shop/customers",
                "customers",
                "Local / shop / public",
                vec!["id", "name", "email", "country", "status"],
            ),
            (
                "db/local/shop/orders",
                "orders",
                "Local / shop / public",
                vec!["id", "customer", "total", "currency", "status"],
            ),
            (
                "db/local/shop/products",
                "products",
                "Local / shop / public",
                vec!["id", "name", "category", "price", "stock"],
            ),
            (
                "k8s/development/pods",
                "Pods",
                "Development",
                vec!["name", "namespace", "ready", "status", "node"],
            ),
            (
                "k8s/development/deployments",
                "Deployments",
                "Development",
                vec!["name", "namespace", "ready", "available", "age"],
            ),
            (
                "k8s/development/services",
                "Services",
                "Development",
                vec!["name", "namespace", "type", "cluster-ip", "port"],
            ),
        ]
        .into_iter()
        .enumerate()
        .map(|(kind, (key, title, context, columns))| {
            let rows = (1..=1000)
                .map(|i| {
                    let values = match kind {
                        0 => vec![
                            i.to_string(),
                            format!("{} {i}", ["Ada", "สมชาย", "José", "Yuki"][i % 4]),
                            format!("customer{i}@example.test"),
                            ["TH", "JP", "GB", "DE"][i % 4].into(),
                            if i % 7 == 0 { "Inactive" } else { "Active" }.into(),
                        ],
                        1 => vec![
                            i.to_string(),
                            format!("Customer {}", i % 250 + 1),
                            format!("{}", i * 17),
                            "USD".into(),
                            ["Paid", "Pending", "Shipped"][i % 3].into(),
                        ],
                        2 => vec![
                            i.to_string(),
                            format!("Product {i}"),
                            ["Hardware", "Software", "Accessories"][i % 3].into(),
                            (i * 3).to_string(),
                            (i % 125).to_string(),
                        ],
                        3 => vec![
                            format!("api-{i:04}"),
                            if i <= 500 {
                                "default"
                            } else if i <= 800 {
                                "staging"
                            } else {
                                "production"
                            }
                            .into(),
                            "1/1".into(),
                            if i % 19 == 0 { "Pending" } else { "Running" }.into(),
                            format!("worker-{}", i % 4 + 1),
                        ],
                        4 => vec![
                            format!("deployment-{i:04}"),
                            if i <= 500 {
                                "default"
                            } else if i <= 800 {
                                "staging"
                            } else {
                                "production"
                            }
                            .into(),
                            "3/3".into(),
                            "3".into(),
                            format!("{}d", i % 90 + 1),
                        ],
                        _ => vec![
                            format!("service-{i:04}"),
                            if i <= 500 {
                                "default"
                            } else if i <= 800 {
                                "staging"
                            } else {
                                "production"
                            }
                            .into(),
                            "ClusterIP".into(),
                            format!("10.96.{}.{}", i / 250, i % 250 + 1),
                            "8080/TCP".into(),
                        ],
                    };
                    values.into_iter().map(SharedString::from).collect()
                })
                .collect();
            Rc::new(Self {
                document: Document::new(key, title, context).icon_path(
                    if key.starts_with("db/") {
                        IconName::Table
                    } else {
                        IconName::Box
                    }
                    .path(),
                ),
                columns,
                kind: if kind < 3 {
                    ResourceKind::Database
                } else {
                    ResourceKind::Kubernetes
                },
                rows,
            })
        })
        .collect()
    }
}
pub fn navigation(datasets: &[Rc<Dataset>]) -> Vec<NavigationNode> {
    let resources = |range: std::ops::Range<usize>| {
        range
            .map(|i| {
                NavigationNode::resource(datasets[i].document.clone()).icon(if i < 3 {
                    IconName::Table
                } else {
                    IconName::Box
                })
            })
            .collect()
    };
    vec![
        NavigationNode::folder(
            "database",
            "Database",
            vec![NavigationNode::folder(
                "local",
                "Local connection",
                vec![NavigationNode::folder(
                    "shop",
                    "shop",
                    vec![NavigationNode::folder("public", "public", resources(0..3))],
                )],
            )],
        )
        .icon(IconName::Database),
        NavigationNode::folder(
            "kubernetes",
            "Kubernetes",
            vec![NavigationNode::folder(
                "development",
                "Development cluster",
                resources(3..6),
            )],
        )
        .icon(IconName::Network),
        NavigationNode::folder(
            "foundation",
            "Foundation",
            Page::ALL
                .into_iter()
                .map(|p| {
                    NavigationNode::resource(Document::new(
                        p.id(),
                        p.title(),
                        "Foundation examples",
                    ))
                })
                .collect(),
        ),
    ]
}
pub struct Rows {
    pub data: Rc<Dataset>,
    pub store: Entity<DatasetStore>,
    pub indices: Vec<usize>,
    pub query: String,
    pub sort: Option<(usize, ColumnSort)>,
    pub namespaces: BTreeSet<SharedString>,
    applied: (
        u64,
        String,
        Option<(usize, ColumnSort)>,
        BTreeSet<SharedString>,
    ),
    #[cfg(test)]
    pub(crate) projection_rebuilds: usize,
}
impl Rows {
    pub(crate) fn new(data: Rc<Dataset>, store: Entity<DatasetStore>, cx: &App) -> Self {
        let source = store.read(cx);
        let indices = (0..source.values.len()).collect();
        let revision = source.revision;
        Self {
            data,
            store,
            indices,
            query: String::new(),
            sort: None,
            namespaces: BTreeSet::new(),
            applied: (revision, String::new(), None, BTreeSet::new()),
            #[cfg(test)]
            projection_rebuilds: 0,
        }
    }
    pub fn rebuild(&mut self, cx: &App) {
        if self.is_current(cx) {
            return;
        }
        #[cfg(test)]
        {
            self.projection_rebuilds += 1;
        }
        let store = self.store.read(cx);
        let values = &store.values;
        self.indices = (0..values.len())
            .filter(|i| {
                let namespace_matches =
                    self.namespaces.is_empty() || self.namespaces.contains(&values[*i][1]);
                let search_matches = self.query.is_empty()
                    || match self.data.kind {
                        ResourceKind::Database => store.search_keys[*i]
                            .iter()
                            .any(|c| c.contains(&self.query)),
                        ResourceKind::Kubernetes => store.search_keys[*i][0].contains(&self.query),
                    };
                namespace_matches && search_matches
            })
            .collect();
        if let Some((col, sort)) = self.sort
            && sort != ColumnSort::Default
        {
            self.indices.sort_by(|a, b| {
                let order = match (store.numeric_keys[*a][col], store.numeric_keys[*b][col]) {
                    (Some(a), Some(b)) => a.cmp(&b),
                    _ => values[*a][col].cmp(&values[*b][col]),
                };
                if sort == ColumnSort::Descending {
                    order.reverse()
                } else {
                    order
                }
            });
        }
        self.applied = (
            store.revision,
            self.query.clone(),
            self.sort,
            self.namespaces.clone(),
        );
    }
    fn is_current(&self, cx: &App) -> bool {
        self.applied.0 == self.store.read(cx).revision
            && self.applied.1 == self.query
            && self.applied.2 == self.sort
            && self.applied.3 == self.namespaces
    }
}
impl EditableTableDelegate for Rows {
    fn columns_count(&self, _: &App) -> usize {
        self.data.columns.len()
    }
    fn rows_count(&self, _: &App) -> usize {
        self.indices.len()
    }
    fn row_key(&self, row: usize, cx: &App) -> SharedString {
        self.store.read(cx).row_keys[self.indices[row]].clone()
    }
    fn column(&self, col: usize, _: &App) -> Column {
        let column = Column::new(self.data.columns[col], self.data.columns[col])
            .width(px(if col == 0 { 180. } else { 200. }))
            .min_width(px(80.))
            .sortable()
            .resizable(true)
            .movable(false);
        if let Some((index, sort)) = self.sort
            && index == col
        {
            column.sort(sort)
        } else {
            column
        }
    }
    fn value(&self, cell: &CellAddress, cx: &App) -> SharedString {
        let Some(row) = self.store.read(cx).positions.get(&cell.row).copied() else {
            return SharedString::default();
        };
        let col = self
            .data
            .columns
            .iter()
            .position(|key| *key == cell.column.as_ref())
            .unwrap();
        self.store.read(cx).values[row][col].clone()
    }
    fn editor(&self, cell: &CellAddress, _: &App) -> CellEditor {
        let options: &[&str] = match (
            self.data.document.key.0.rsplit('/').next().unwrap_or(""),
            cell.column.as_ref(),
        ) {
            ("customers", "status") => &["Active", "Inactive"],
            ("orders", "status") => &["Paid", "Pending", "Shipped"],
            ("products", "category") => &["Hardware", "Software", "Accessories"],
            ("pods", "status") => &["Running", "Pending"],
            ("services", "type") => &["ClusterIP", "NodePort", "LoadBalancer", "ExternalName"],
            _ => &[],
        };
        if options.is_empty() {
            CellEditor::Text
        } else {
            CellEditor::Dropdown(options.iter().map(|s| (*s).into()).collect())
        }
    }
    fn commit(
        &mut self,
        cell: &CellAddress,
        value: SharedString,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), SharedString> {
        self.commit_cells(&[(cell.clone(), value)], window, cx)
    }
    fn commit_cells(
        &mut self,
        changes: &[(CellAddress, SharedString)],
        _: &mut Window,
        cx: &mut App,
    ) -> Result<(), SharedString> {
        // Resolve and validate the entire batch before notifying shared views.
        // A failed value must never leave a partially edited selection behind.
        let mut resolved = Vec::with_capacity(changes.len());
        for (cell, value) in changes {
            if let CellEditor::Dropdown(options) = self.editor(cell, cx)
                && !options.contains(value)
            {
                return Err("Choose a value from the list".into());
            }
            // Database cells commit into a draft; required fields are checked
            // across the whole store when Save advances its checkpoint.
            if self.data.kind == ResourceKind::Kubernetes
                && cell.column == "name"
                && value.trim().is_empty()
            {
                return Err("Name is required".into());
            }
            let row = self
                .store
                .read(cx)
                .positions
                .get(&cell.row)
                .copied()
                .ok_or_else(|| SharedString::from("Unknown row"))?;
            let col = self
                .data
                .columns
                .iter()
                .position(|key| *key == cell.column.as_ref())
                .ok_or_else(|| SharedString::from("Unknown column"))?;
            resolved.push((row, col, value.clone()));
        }
        if !resolved.is_empty() {
            self.store.update(cx, |store, cx| {
                let mut changed = false;
                for (row, col, value) in resolved {
                    changed |= store.set_value(row, col, value);
                }
                if changed {
                    store.revision = store.revision.wrapping_add(1);
                    cx.notify();
                }
            });
            self.rebuild(cx);
        }
        Ok(())
    }
    fn sort(&mut self, col: usize, sort: ColumnSort, _: &mut Window, cx: &mut App) {
        self.sort = Some((col, sort));
        self.rebuild(cx);
    }
}
pub struct ResourceView {
    pub table: Entity<TableState<EditableTableAdapter<Rows>>>,
    pub editable: Entity<EditableTableState<Rows>>,
    pub filter: Entity<InputState>,
    document: Document,
    kind: ResourceKind,
    store: Entity<DatasetStore>,
    selected_rows: BTreeSet<SharedString>,
    namespace_open: bool,
    namespace_focus: FocusHandle,
    cancel_hitbox: Rc<Cell<Option<gpui::HitboxId>>>,
    _subscription: gpui::Subscription,
    _activity: Vec<gpui::Subscription>,
    last_activity: (String, Option<(usize, ColumnSort)>),
}
impl ResourceView {
    pub fn new(
        data: Rc<Dataset>,
        store: Entity<DatasetStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let document = data.document.clone();
        let kind = data.kind;
        let editable =
            cx.new(|cx| EditableTableState::new(Rows::new(data, store.clone(), cx), window, cx));
        let table = editable.read(cx).table().clone();
        let weak = editable.downgrade();
        let shared = cx.observe(&store, move |_, _, cx| {
            let _ = weak.update(cx, |state, cx| {
                if !state.table().read(cx).delegate().is_current(cx) {
                    state.update_delegate(cx, |rows, cx| rows.rebuild(cx));
                }
            });
        });
        let cancel_hitbox = Rc::new(Cell::new(None));
        store.update(cx, |store, _| {
            store.editors.retain(|editor| editor.upgrade().is_some());
            store.editors.push(editable.downgrade());
            store
                .cancel_controls
                .retain(|control| control.upgrade().is_some());
            store.cancel_controls.push(Rc::downgrade(&cancel_hitbox));
        });
        let source = store.downgrade();
        let mut was_editing = false;
        let editor_status = cx.observe(&editable, move |_, grid, cx| {
            let editing = grid.read(cx).is_editing();
            if editing != was_editing {
                was_editing = editing;
                let _ = source.update(cx, |_, cx| cx.notify());
            }
        });
        let source_header = cx.observe(&store, |_, _, cx| cx.notify());
        let selection = cx.subscribe(&editable, |this, _, event: &EditableTableEvent, cx| {
            if let EditableTableEvent::SelectionChanged(selection) = event {
                let rows = selection
                    .cells
                    .iter()
                    .map(|cell| cell.row.clone())
                    .collect();
                if this.selected_rows != rows {
                    this.selected_rows = rows;
                    cx.notify();
                }
            }
        });
        let placeholder = match kind {
            ResourceKind::Database => SharedString::from("Filter rows…"),
            ResourceKind::Kubernetes => format!("Search {}…", document.title).into(),
        };
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let subscription = cx.subscribe_in(
            &filter,
            window,
            |this, input, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    let query = input.read(cx).value().to_lowercase();
                    if query == this.table.read(cx).delegate().query {
                        return;
                    }
                    let accepted = this.editable.update(cx, |state, cx| {
                        if !state.commit_edit(window, cx) {
                            return false;
                        }
                        state.update_delegate(cx, |rows, cx| {
                            rows.query = query;
                            rows.rebuild(cx);
                        });
                        true
                    });
                    if !accepted {
                        let previous = this.table.read(cx).delegate().query.clone();
                        input.update(cx, |input, cx| input.set_value(previous, window, cx));
                    }
                    cx.notify();
                }
            },
        );
        Self {
            table,
            editable,
            filter,
            document,
            kind,
            store,
            selected_rows: BTreeSet::new(),
            namespace_open: false,
            namespace_focus: cx.focus_handle(),
            cancel_hitbox,
            _subscription: subscription,
            _activity: vec![shared, editor_status, source_header, selection],
            last_activity: (String::new(), None),
        }
    }
    pub fn update_metadata(
        &mut self,
        document: Document,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.document.title == document.title && self.document.context == document.context {
            return;
        }
        if self.kind == ResourceKind::Kubernetes {
            self.filter.update(cx, |input, cx| {
                input.set_placeholder(format!("Search {}…", document.title), window, cx)
            });
        }
        self.document = document;
        cx.notify();
    }
    pub fn connect_catalog(
        &mut self,
        catalog: &Entity<crate::catalog::Catalog>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self._activity
            .push(cx.observe_in(catalog, window, |this, catalog, window, cx| {
                if let Some(document) = catalog
                    .read(cx)
                    .node(&this.document.key.0)
                    .and_then(|node| node.document.clone())
                {
                    this.update_metadata(document, window, cx);
                }
            }));
    }
    pub fn connect_inspector(
        &mut self,
        inspector: gpui::WeakEntity<crate::inspector::Inspector>,
        cx: &mut Context<Self>,
    ) {
        let selection = inspector.clone();
        self._activity.push(cx.subscribe(
            &self.editable,
            move |this, _, event: &EditableTableEvent, cx| {
                let _ = selection.update(cx, |inspector, cx| {
                    if let EditableTableEvent::Committed { cell, value, .. } = event {
                        inspector.record(
                            format!(
                                "Edit {} · {}[{}] = {}",
                                this.document.title, cell.row, cell.column, value
                            ),
                            cx,
                        );
                    }
                });
                // The ResourceView callback still owns its entity lease here.
                // Read the active page only after that lease has been released.
                let inspector = selection.clone();
                cx.defer(move |cx| {
                    let _ = inspector.update(cx, |inspector, cx| inspector.refresh_details(cx));
                });
            },
        ));
        self._activity
            .push(cx.observe(&self.table, move |this, table, cx| {
                let rows = table.read(cx).delegate();
                if rows.query == this.last_activity.0 && rows.sort == this.last_activity.1 {
                    return;
                }
                let next = (rows.query.clone(), rows.sort);
                if next != this.last_activity {
                    let message = if next.0 != this.last_activity.0 {
                        format!(
                            "Filter {}: {:?} · {} rows",
                            this.document.title,
                            next.0,
                            rows.indices.len()
                        )
                    } else {
                        format!("Sort {}: {:?}", this.document.title, next.1)
                    };
                    this.last_activity = next;
                    let _ = inspector.update(cx, |inspector, cx| inspector.record(message, cx));
                }
            }));
    }
}
impl Focusable for ResourceView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.editable.focus_handle(cx)
    }
}
impl Render for ResourceView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id(("resource-view", cx.entity_id()))
            .test_support()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .min_w_0()
            .child(
                div()
                    .id("resource-breadcrumb")
                    .test_support()
                    .h(px(22.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .px_3()
                    .gap_1()
                    .overflow_hidden()
                    .text_size(px(13.))
                    .text_color(cx.theme().muted_foreground)
                    .children(self.document.context.split(" / ").map(|part| {
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(part.to_owned())
                            .child(Icon::new(IconName::ChevronRight).size(px(12.)))
                    }))
                    .child(
                        Icon::new(if self.kind == ResourceKind::Database {
                            IconName::Table
                        } else {
                            IconName::Box
                        })
                        .size(px(14.)),
                    )
                    .child(self.document.title.clone()),
            )
            .child(self.resource_toolbar(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .child(EditableTable::new(&self.editable)),
            )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[gpui_kit::test]
    fn fixtures_filter_unicode_and_sort_numbers(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            let fixtures = Dataset::fixtures();
            assert_eq!(fixtures.len(), 6);
            let store = cx.new(|_| DatasetStore::new(&fixtures[0]));
            let mut rows = Rows::new(fixtures[0].clone(), store, cx);
            rows.query = "สมชาย".into();
            rows.rebuild(cx);
            assert_eq!(rows.indices.len(), 250);
            rows.query.clear();
            rows.sort = Some((0, ColumnSort::Descending));
            rows.rebuild(cx);
            assert_eq!(rows.data.rows[rows.indices[0]][0].as_ref(), "1000");
            rows.sort = Some((0, ColumnSort::Ascending));
            rows.rebuild(cx);
            assert_eq!(rows.data.rows[rows.indices[0]][0].as_ref(), "1");
        });
    }
}
