//! Resource commands share toolbar handlers and snapshot stable menu targets.
use super::*;
use gpui::component::{
    WindowExt,
    menu::{PopupMenu, PopupMenuItem},
};
use pintle_gpui_kit::components::table::{CellSelection, selection_tsv};

#[derive(Clone, Copy)]
enum Command {
    Copy,
    CopyRows,
    Paste,
    Edit,
    Add,
    Delete,
    Save,
    Cancel,
    Name,
    Namespace,
    FilterNamespace,
    Clear,
}

struct MenuTarget {
    cell: Option<CellAddress>,
    selection: CellSelection,
    rows: Vec<SharedString>,
    columns: Vec<SharedString>,
}

impl ResourceView {
    pub fn connect_context_menu(&mut self, cx: &mut Context<Self>) {
        let owner = cx.weak_entity();
        self.editable.update(cx, |grid, _| {
            grid.set_context_menu(Rc::new(move |cell, menu, _, cx| {
                let Some(view) = owner.upgrade() else {
                    return menu;
                };
                view.read(cx)
                    .build_context_menu(cell, menu, view.downgrade(), cx)
            }))
        });
    }
    fn build_context_menu(
        &self,
        cell: Option<CellAddress>,
        mut menu: PopupMenu,
        owner: gpui::WeakEntity<Self>,
        cx: &App,
    ) -> PopupMenu {
        let mut selection = self.editable.read(cx).selection();
        if let Some(cell) = &cell
            && !selection.contains(cell)
        {
            selection.select(cell.clone());
        }
        let pending = self.store.read(cx).dirty()
            || self
                .editors(cx)
                .iter()
                .any(|grid| grid.read(cx).is_editing());
        let rows = self.table.read(cx).delegate();
        let target = Rc::new(MenuTarget {
            cell: cell.clone(),
            selection: selection.clone(),
            rows: rows
                .indices
                .iter()
                .map(|index| rows.store.read(cx).row_keys[*index].clone())
                .collect(),
            columns: rows
                .data
                .columns
                .iter()
                .map(|column| (*column).into())
                .collect(),
        });
        let item = |menu: PopupMenu, label: &'static str, command, disabled| {
            let owner = owner.clone();
            let target = target.clone();
            menu.item(PopupMenuItem::new(label).disabled(disabled).on_click(
                move |_, window, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        this.run_context_command(command, target.clone(), window, cx)
                    });
                },
            ))
        };
        if cell.is_some() {
            menu = item(menu, "Copy", Command::Copy, selection.cells.is_empty());
            menu = item(
                menu,
                "Copy Rows",
                Command::CopyRows,
                selection.cells.is_empty(),
            );
            let paste_unavailable = cx
                .read_from_clipboard()
                .and_then(|item| item.text())
                .is_none_or(|text| text.is_empty());
            menu = item(menu, "Paste", Command::Paste, paste_unavailable);
            menu = item(menu, "Edit", Command::Edit, false).separator();
        }
        if self.kind == ResourceKind::Database {
            menu = item(menu, "Add Row", Command::Add, false);
            if cell.is_some() {
                menu = item(
                    menu,
                    "Delete Selected Rows",
                    Command::Delete,
                    selection.cells.is_empty(),
                );
            }
            menu = item(menu.separator(), "Save Changes", Command::Save, !pending);
            item(menu, "Cancel Changes", Command::Cancel, !pending)
        } else if cell.is_some() {
            menu = item(menu, "Copy Name", Command::Name, false);
            menu = item(menu, "Copy Namespace", Command::Namespace, false);
            item(
                menu.separator(),
                "Filter to This Namespace",
                Command::FilterNamespace,
                false,
            )
        } else {
            item(
                menu,
                "Clear Filters",
                Command::Clear,
                self.filter.read(cx).value().is_empty()
                    && self.table.read(cx).delegate().namespaces.is_empty(),
            )
        }
    }
    fn run_context_command(
        &mut self,
        command: Command,
        target: Rc<MenuTarget>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let cell = target.cell.clone();
        let selection = target.selection.clone();
        if !matches!(
            command,
            Command::Cancel | Command::Save | Command::Add | Command::Clear
        ) && (cell
            .as_ref()
            .is_some_and(|cell| !self.store.read(cx).positions.contains_key(&cell.row))
            || selection
                .cells
                .iter()
                .any(|cell| !self.store.read(cx).positions.contains_key(&cell.row)))
        {
            return;
        }
        match command {
            Command::Copy | Command::CopyRows => {
                let rows = self.table.read(cx).delegate();
                let row_keys = &target.rows;
                let columns = &target.columns;
                let mut selection = selection;
                if matches!(command, Command::CopyRows) {
                    let selected: BTreeSet<_> = selection
                        .cells
                        .iter()
                        .map(|cell| cell.row.clone())
                        .collect();
                    selection.cells = selected
                        .into_iter()
                        .flat_map(|row| {
                            columns
                                .iter()
                                .map(move |col| CellAddress::new(row.clone(), col.clone()))
                        })
                        .collect();
                }
                match selection_tsv(row_keys, columns, &selection, |cell| {
                    rows.value(cell, cx).to_string()
                }) {
                    Ok(text) => cx.write_to_clipboard(gpui::ClipboardItem::new_string(text)),
                    Err(error) => window.push_notification(error, cx),
                }
            }
            Command::Name | Command::Namespace => {
                if let Some(cell) = cell {
                    let value = self.table.read(cx).delegate().value(
                        &CellAddress::new(
                            cell.row,
                            if matches!(command, Command::Name) {
                                "name"
                            } else {
                                "namespace"
                            },
                        ),
                        cx,
                    );
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(value.to_string()));
                }
            }
            Command::Cancel => self.revert_rows(window, cx),
            Command::Save => self.save_rows(window, cx),
            Command::Add => self.add_row(window, cx),
            Command::FilterNamespace => {
                if let Some(cell) = cell {
                    let namespace = self
                        .table
                        .read(cx)
                        .delegate()
                        .value(&CellAddress::new(cell.row, "namespace"), cx);
                    self.apply_namespaces([namespace].into(), window, cx);
                }
            }
            Command::Clear => {
                if !self
                    .editable
                    .update(cx, |grid, cx| grid.commit_edit(window, cx))
                {
                    return;
                }
                self.apply_namespaces(BTreeSet::new(), window, cx);
                self.clear_query(window, cx);
            }
            Command::Paste | Command::Edit | Command::Delete => {
                let accepted = self.editable.update(cx, |grid, cx| {
                    if !grid.commit_edit(window, cx) {
                        return false;
                    }
                    grid.set_selection(selection.clone(), cx);
                    true
                });
                if !accepted {
                    return;
                }
                self.selected_rows = selection
                    .cells
                    .iter()
                    .map(|cell| cell.row.clone())
                    .collect();
                match command {
                    Command::Delete => self.delete_rows(window, cx),
                    Command::Edit => self
                        .editable
                        .update(cx, |grid, cx| grid.begin_edit(false, window, cx)),
                    Command::Paste => self
                        .editable
                        .update(cx, |grid, cx| grid.paste_clipboard(window, cx)),
                    _ => unreachable!(),
                }
            }
        }
    }
}
