//! Spreadsheet interactions over Kit's virtualized DataTable.
mod selection;
mod state;
use gpui_kit::component::table::{Column, ColumnSort};
use gpui_kit::{App, SharedString, Window};
pub use selection::{CellAddress, CellSelection, parse_tsv, selection_tsv};
pub use state::{EditableTable, EditableTableAdapter, EditableTableState, TableContextMenuBuilder};

/// Editor chosen by the application for a cell. Dropdown values are closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CellEditor {
    ReadOnly,
    Text,
    Dropdown(Vec<SharedString>),
}

/// Application-owned data and validation. All keys must be unique and stable.
/// Commit must update the source synchronously, or return a user-facing error.
/// Notify other views through the application's shared data entity.
pub trait EditableTableDelegate: 'static {
    fn rows_count(&self, cx: &App) -> usize;
    fn columns_count(&self, cx: &App) -> usize;
    fn row_key(&self, row: usize, cx: &App) -> SharedString;
    fn column(&self, col: usize, cx: &App) -> Column;
    fn value(&self, cell: &CellAddress, cx: &App) -> SharedString;
    fn editor(&self, cell: &CellAddress, cx: &App) -> CellEditor;
    fn commit(
        &mut self,
        cell: &CellAddress,
        value: SharedString,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), SharedString>;
    /// Apply a selection edit atomically. Validate every change before mutating
    /// the source; returning an error must leave every cell unchanged.
    /// The default keeps existing single-cell delegates compatible and rejects
    /// batches rather than risking a partially applied edit.
    fn commit_cells(
        &mut self,
        changes: &[(CellAddress, SharedString)],
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), SharedString> {
        match changes {
            [] => Ok(()),
            [(cell, value)] => self.commit(cell, value.clone(), window, cx),
            _ => Err("This table delegate does not support batch edits".into()),
        }
    }
    fn sort(&mut self, _col: usize, _sort: ColumnSort, _window: &mut Window, _cx: &mut App) {}
}

/// Events emitted by retained editable-table state.
#[derive(Clone, Debug)]
pub enum EditableTableEvent {
    SelectionChanged(CellSelection),
    Committed {
        cell: CellAddress,
        old_value: SharedString,
        value: SharedString,
    },
    CopyFailed(SharedString),
    PasteFailed(SharedString),
}
