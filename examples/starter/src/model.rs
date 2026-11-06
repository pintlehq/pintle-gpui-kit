//! Replace this in-memory model with your application's data/service layer.
use gpui::assets::IconName;
use gpui::component::table::{Column, ColumnSort};
use gpui::{App, AppContext, Entity, SharedString, Window, px};
use pintle_gpui_kit::{
    components::table::{CellAddress, CellEditor, EditableTableDelegate},
    kit as gpui,
    shell::workspace::Document,
};
use std::collections::BTreeMap;

pub const COLUMNS: [(&str, &str); 4] = [
    ("id", "ID"),
    ("name", "Name"),
    ("email", "Email"),
    ("status", "Status"),
];
const STATUSES: [&str; 2] = ["Active", "Inactive"];

pub struct Store {
    // Keys belong to records, never to their current sort/filter positions.
    pub records: BTreeMap<SharedString, [SharedString; 4]>,
}
#[derive(Clone)]
pub struct Dataset {
    pub document: Document,
    pub store: Entity<Store>,
}
pub fn datasets(cx: &mut App) -> Vec<Dataset> {
    [("contacts", "Contacts"), ("team", "Team")]
        .into_iter()
        .map(|(key, title)| {
            let records = (1..=1000)
                .map(|n| {
                    (
                        format!("{key}-{n:04}").into(),
                        [
                            format!("{n:04}").into(),
                            format!("{} {n}", ["Ada", "สมชาย", "José", "Yuki"][(n - 1) % 4]).into(),
                            format!("{key}{n}@example.test").into(),
                            if n % 5 == 0 { "Inactive" } else { "Active" }.into(),
                        ],
                    )
                })
                .collect();
            Dataset {
                document: Document::new(key, title, "Demo / People")
                    .icon_path(IconName::Table.path()),
                store: cx.new(|_| Store { records }),
            }
        })
        .collect()
}

/// A fresh delegate per view: only the underlying store is shared across splits.
pub struct Rows {
    pub store: Entity<Store>,
    pub visible: Vec<SharedString>,
    pub query: String,
    pub sort: Option<(usize, ColumnSort)>,
}
impl Rows {
    pub fn new(store: Entity<Store>, cx: &App) -> Self {
        let mut rows = Self {
            store,
            visible: Vec::new(),
            query: String::new(),
            sort: None,
        };
        rows.rebuild(cx);
        rows
    }
    pub fn rebuild(&mut self, cx: &App) {
        let records = &self.store.read(cx).records;
        self.visible = records
            .iter()
            .filter(|(_, values)| {
                values
                    .iter()
                    .any(|value| value.to_lowercase().contains(&self.query))
            })
            .map(|(key, _)| key.clone())
            .collect();
        if let Some((col, direction)) = self.sort
            && direction != ColumnSort::Default
        {
            self.visible.sort_by(|a, b| {
                let order = records[a][col].cmp(&records[b][col]);
                let order = if direction == ColumnSort::Descending {
                    order.reverse()
                } else {
                    order
                };
                order.then_with(|| a.cmp(b))
            });
        }
    }
    fn column_index(cell: &CellAddress) -> Result<usize, SharedString> {
        COLUMNS
            .iter()
            .position(|(key, _)| *key == cell.column.as_ref())
            .ok_or_else(|| "Unknown column".into())
    }
}
impl EditableTableDelegate for Rows {
    fn rows_count(&self, _: &App) -> usize {
        self.visible.len()
    }
    fn columns_count(&self, _: &App) -> usize {
        COLUMNS.len()
    }
    fn row_key(&self, row: usize, _: &App) -> SharedString {
        self.visible[row].clone()
    }
    fn column(&self, col: usize, _: &App) -> Column {
        let (key, label) = COLUMNS[col];
        let column = Column::new(key, label)
            .width(px([120., 220., 280., 160.][col]))
            .min_width(px(80.))
            .sortable()
            .resizable(true)
            .movable(false);
        if let Some((index, direction)) = self.sort
            && index == col
        {
            column.sort(direction)
        } else {
            column
        }
    }
    fn value(&self, cell: &CellAddress, cx: &App) -> SharedString {
        self.store.read(cx).records[&cell.row]
            [Self::column_index(cell).expect("known table column")]
        .clone()
    }
    fn editor(&self, cell: &CellAddress, _: &App) -> CellEditor {
        match cell.column.as_ref() {
            "id" => CellEditor::ReadOnly,
            "status" => CellEditor::Dropdown(STATUSES.map(SharedString::from).to_vec()),
            _ => CellEditor::Text,
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
        let mut checked = Vec::with_capacity(changes.len());
        for (cell, value) in changes {
            let col = Self::column_index(cell)?;
            if !self.store.read(cx).records.contains_key(&cell.row) {
                return Err("Unknown record".into());
            }
            match col {
                0 => return Err("ID is read-only".into()),
                1 if value.trim().is_empty() => return Err("Name is required".into()),
                2 if !value.is_empty() && !value.contains('@') => {
                    return Err("Email must contain @".into());
                }
                3 if !STATUSES.contains(&value.as_ref()) => {
                    return Err("Choose Active or Inactive".into());
                }
                _ => {}
            }
            checked.push((cell.row.clone(), col, value.clone()));
        }
        // Validate everything first; one invalid cell must roll back the whole
        // fill/paste. Notify once so other views never see a partial update.
        if !checked.is_empty() {
            self.store.update(cx, |store, cx| {
                for (key, col, value) in checked {
                    store.records.get_mut(&key).expect("validated record")[col] = value;
                }
                cx.notify();
            });
            self.rebuild(cx);
        }
        Ok(())
    }
    fn sort(&mut self, col: usize, direction: ColumnSort, _: &mut Window, cx: &mut App) {
        self.sort = Some((col, direction));
        self.rebuild(cx);
    }
}
