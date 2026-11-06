use super::*;
use gpui::base::ElementExt;
use gpui::component::button::{Button, ButtonVariants};
use gpui::component::{
    ActiveTheme, Sizable, Size,
    input::{Input, InputEvent, InputState},
    popover::Popover,
    table::{DataTable, TableDelegate, TableState},
};
use gpui::{
    AppContext, Bounds, ClipboardItem, Context, Edges, ElementInputHandler, Entity,
    EntityInputHandler, EventEmitter, FocusHandle, Focusable, IntoElement, KeyDownEvent,
    MouseButton, Pixels, Point, Render, RenderOnce, SharedString, Subscription, TestSupportExt,
    UTF16Selection, WeakEntity, canvas, div, point, prelude::*, px,
};
use gpui_kit as gpui;
use std::{
    cell::RefCell,
    collections::{BTreeSet, HashMap},
    ops::{Deref, DerefMut, Range},
    rc::Rc,
    time::{Duration, Instant},
};

gpui::actions!(
    pintle_editable_table,
    [
        GridUp,
        GridDown,
        GridLeft,
        GridRight,
        ExtendUp,
        ExtendDown,
        ExtendLeft,
        ExtendRight,
        GridEnter,
        GridBackEnter,
        GridTab,
        GridBackTab,
        GridEdit,
        GridCopy,
        GridPaste,
        GridCancel
    ]
);
/// Application builder receives a stable cell identity, or None for table background.
pub type TableContextMenuBuilder = Rc<
    dyn Fn(
        Option<CellAddress>,
        gpui::component::menu::PopupMenu,
        &mut Window,
        &mut Context<gpui::component::menu::PopupMenu>,
    ) -> gpui::component::menu::PopupMenu,
>;
struct Bindings;
impl gpui::Global for Bindings {}

#[derive(Default)]
struct Interaction {
    selection: CellSelection,
    bounds: HashMap<CellAddress, Bounds<Pixels>>,
    widths: HashMap<SharedString, Pixels>,
    range_base: BTreeSet<CellAddress>,
    rows: Rc<Vec<SharedString>>,
    columns: Rc<Vec<SharedString>>,
    row_positions: HashMap<SharedString, usize>,
    column_positions: HashMap<SharedString, usize>,
    viewport: Bounds<Pixels>,
    drag: Option<DragSelection>,
    editor: Option<Editor>,
    error: Option<SharedString>,
}
impl Interaction {
    fn set_rows(&mut self, rows: Vec<SharedString>) {
        self.row_positions = rows
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, key)| (key, i))
            .collect();
        self.rows = Rc::new(rows);
    }
    fn set_columns(&mut self, columns: Vec<SharedString>) {
        self.column_positions = columns
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, key)| (key, i))
            .collect();
        self.columns = Rc::new(columns);
    }
}
struct DragSelection {
    anchor: CellAddress,
    base: BTreeSet<CellAddress>,
    pointer: Point<Pixels>,
}
#[derive(Clone)]
struct Editor {
    cell: CellAddress,
    targets: Vec<CellAddress>,
    input: Entity<InputState>,
    options: Option<Vec<SharedString>>,
    cursor: usize,
    scroll: gpui::ScrollHandle,
}

/// Kit adapter. Dereferences to the application delegate for inspection.
/// Mutate data through `EditableTableState::update_delegate` so keys reconcile.
pub struct EditableTableAdapter<D: EditableTableDelegate> {
    delegate: D,
    owner: WeakEntity<EditableTableState<D>>,
    ui: Rc<RefCell<Interaction>>,
}
impl<D: EditableTableDelegate> Deref for EditableTableAdapter<D> {
    type Target = D;
    fn deref(&self) -> &D {
        &self.delegate
    }
}
impl<D: EditableTableDelegate> DerefMut for EditableTableAdapter<D> {
    fn deref_mut(&mut self) -> &mut D {
        &mut self.delegate
    }
}
impl<D: EditableTableDelegate> TableDelegate for EditableTableAdapter<D> {
    fn rows_count(&self, cx: &App) -> usize {
        self.delegate.rows_count(cx)
    }
    fn columns_count(&self, cx: &App) -> usize {
        self.delegate.columns_count(cx)
    }
    fn column(&self, col: usize, cx: &App) -> Column {
        let mut column = self.delegate.column(col, cx);
        column.paddings = Some(Edges::all(px(0.)));
        if let Some(width) = self.ui.borrow().widths.get(&column.key) {
            column.width = *width;
        }
        column
    }
    fn render_tr(
        &mut self,
        row: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(("editable-row", row))
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(px(13.))
    }
    fn render_header(
        &mut self,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> gpui::Stateful<gpui::Div> {
        self.ui.borrow_mut().bounds.clear();
        div().id("editable-table-header")
    }
    fn render_th(
        &mut self,
        col: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let owner = self.owner.clone();
        let column = self.delegate.column(col, cx);
        let sortable = column.sort.is_some();
        let next = match column.sort {
            Some(ColumnSort::Descending) => ColumnSort::Ascending,
            _ => ColumnSort::Descending,
        };
        div()
            .id(("editable-header", col))
            .size_full()
            .px_2()
            .cursor_pointer()
            .child(column.name)
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                if !sortable {
                    return;
                }
                let _ = owner.update(cx, |state, cx| {
                    if state.commit_edit(window, cx) {
                        state.update_delegate(cx, |d, cx| d.sort(col, next, window, cx));
                    }
                });
            })
    }
    fn perform_sort(
        &mut self,
        col: usize,
        sort: ColumnSort,
        window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) {
        // Kit invokes this while its table is borrowed. Validate against the
        // delegate here; never re-enter that entity through the owner.
        let editor = self.ui.borrow().editor.clone();
        let mut committed = Vec::new();
        if let Some(editor) = editor {
            // Leaving a dropdown without choosing an option only dismisses its
            // search query; it must not broadcast the active cell's old value.
            if editor.options.is_none() {
                let value = editor.input.read(cx).value();
                match apply_edit(&mut self.delegate, &editor, value, window, cx) {
                    Ok(events) => committed = events,
                    Err(error) => {
                        self.ui.borrow_mut().error = Some(error);
                        window.focus(&editor.input.focus_handle(cx), cx);
                        let owner = self.owner.clone();
                        window.defer(cx, move |_, cx| {
                            let _ = owner.update(cx, |s, cx| s.refresh(cx));
                        });
                        return;
                    }
                }
            }
            self.ui.borrow_mut().editor = None;
            self.ui.borrow_mut().error = None;
        }
        self.delegate.sort(col, sort, window, cx);
        {
            let rows = (0..self.delegate.rows_count(cx))
                .map(|r| self.delegate.row_key(r, cx))
                .collect();
            self.ui.borrow_mut().set_rows(rows);
        }
        cx.notify();
        let owner = self.owner.clone();
        window.defer(cx, move |_, cx| {
            let _ = owner.update(cx, |state, cx| {
                for event in committed {
                    cx.emit(event);
                }
                state.refresh(cx);
            });
        });
    }
    fn cell_label(&self, row: usize, col: usize, cx: &App) -> Option<SharedString> {
        Some(self.delegate.value(&self.address(row, col, cx), cx))
    }
    fn cell_text(&self, row: usize, col: usize, cx: &App) -> String {
        self.delegate
            .value(&self.address(row, col, cx), cx)
            .to_string()
    }
    fn render_td(
        &mut self,
        row: usize,
        col: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let cell = self.address(row, col, cx);
        let value = self.delegate.value(&cell, cx);
        let (selected, active, editor, error) = {
            let ui = self.ui.borrow();
            (
                ui.selection.contains(&cell),
                ui.selection.active.as_ref() == Some(&cell),
                ui.editor.as_ref().filter(|e| e.cell == cell).cloned(),
                ui.error.clone(),
            )
        };
        let edge = |dr: isize, dc: isize| {
            let Some(r) = row.checked_add_signed(dr) else {
                return true;
            };
            let Some(c) = col.checked_add_signed(dc) else {
                return true;
            };
            r >= self.delegate.rows_count(cx)
                || c >= self.delegate.columns_count(cx)
                || !self.ui.borrow().selection.contains(&self.address(r, c, cx))
        };
        let (top, bottom, left, right) = if selected && !active {
            (edge(-1, 0), edge(1, 0), edge(0, -1), edge(0, 1))
        } else {
            (false, false, false, false)
        };
        let weak = self.owner.clone();
        let click_owner = weak.clone();
        let double_owner = weak.clone();
        let measure_ui = self.ui.clone();
        let measured_cell = cell.clone();
        let down_cell = cell.clone();
        let double_cell = cell.clone();
        let mut body = div()
            .id(format!("editable-cell:{row}:{col}"))
            .test_support()
            .role(gpui::Role::GridCell)
            .aria_selected(selected)
            .aria_label(value.clone())
            .size_full()
            .relative()
            .flex()
            .items_center()
            .min_w_0()
            .border_1()
            .border_color(if active {
                cx.theme().ring
            } else {
                gpui::transparent_black()
            })
            .when(selected, |d| d.bg(cx.theme().selection.opacity(0.35)))
            .when(selected && !active, |d| {
                d.child(
                    div()
                        .absolute()
                        .inset_0()
                        .border_color(cx.theme().ring.opacity(0.65))
                        .when(top, |d| d.border_t_1())
                        .when(bottom, |d| d.border_b_1())
                        .when(left, |d| d.border_l_1())
                        .when(right, |d| d.border_r_1()),
                )
            })
            .on_prepaint(move |bounds, _, _| {
                measure_ui
                    .borrow_mut()
                    .bounds
                    .insert(measured_cell.clone(), bounds);
            })
            .on_mouse_down(
                MouseButton::Left,
                move |event: &gpui::MouseDownEvent, window: &mut Window, cx: &mut App| {
                    if editor_hit(&click_owner, &down_cell, cx) {
                        return;
                    }
                    cx.stop_propagation();
                    let _ = click_owner.update(cx, |state, cx| {
                        state.pointer_down(
                            down_cell.clone(),
                            event.modifiers.shift,
                            additive(&event.modifiers),
                            event.position,
                            window,
                            cx,
                        )
                    });
                },
            )
            .on_click(move |event, window, cx| {
                cx.stop_propagation();
                if event.click_count() == 2 {
                    let _ = double_owner.update(cx, |state, cx| {
                        if state.is_editing() {
                            return;
                        }
                        state.select_cell(double_cell.clone(), false, false, cx);
                        state.begin_edit(false, window, cx);
                    });
                }
            });
        if let Some(editor) = editor {
            if let Some(options) = editor.options.clone() {
                let owner = weak.clone();
                let dismiss = weak.clone();
                let input = editor.input.clone();
                let focused = input.focus_handle(cx);
                let scroll = editor.scroll.clone();
                body = body.child(
                    Popover::new(("cell-options", input.entity_id()))
                        .open(true)
                        .track_focus(&focused)
                        .overlay_closable(true)
                        .on_open_change(move |open, window, cx| {
                            if !open {
                                let _ =
                                    dismiss.update(cx, |state, cx| state.cancel_edit(window, cx));
                            }
                        })
                        .trigger_style(gpui::StyleRefinement::default().size_full())
                        .trigger(
                            Button::new("cell-dropdown-trigger")
                                .ghost()
                                .label(value)
                                .h(px(26.))
                                .w_full(),
                        )
                        .content(move |_, _, cx| {
                            let query = input.read(cx).value().to_lowercase();
                            let filtered: Vec<_> = options
                                .iter()
                                .filter(|v| v.to_lowercase().contains(&query))
                                .cloned()
                                .collect();
                            let cursor = owner
                                .upgrade()
                                .map(|s| {
                                    s.read(cx)
                                        .ui
                                        .borrow()
                                        .editor
                                        .as_ref()
                                        .map_or(0, |e| e.cursor)
                                })
                                .unwrap_or(0);
                            div()
                                .id("cell-dropdown-popup")
                                .key_context("PintleCellDropdown")
                                .test_support()
                                .w(px(240.))
                                .max_h(px(280.))
                                .flex()
                                .flex_col()
                                .child(Input::new(&input).id("cell-dropdown-search").small())
                                .child(
                                    div()
                                        .id("cell-dropdown-options")
                                        .track_scroll(&scroll)
                                        .max_h(px(220.))
                                        .overflow_y_scroll()
                                        .when(filtered.is_empty(), |d| {
                                            d.child(div().p_2().child("No matching options"))
                                        })
                                        .children(filtered.into_iter().enumerate().map(
                                            |(i, option)| {
                                                let owner = owner.clone();
                                                div()
                                                    .id(("cell-option", i))
                                                    .test_support()
                                                    .role(gpui::Role::ListItem)
                                                    .aria_selected(i == cursor)
                                                    .h(px(28.))
                                                    .px_2()
                                                    .flex()
                                                    .items_center()
                                                    .cursor_pointer()
                                                    .when(i == cursor, |d| {
                                                        d.bg(cx.theme().selection)
                                                    })
                                                    .hover(|d| d.bg(cx.theme().list_hover))
                                                    .child(option.clone())
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        move |_, window, cx| {
                                                            cx.stop_propagation();
                                                            let _ =
                                                                owner.update(cx, |state, cx| {
                                                                    state.choose_option(
                                                                        option.clone(),
                                                                        window,
                                                                        cx,
                                                                    );
                                                                });
                                                        },
                                                    )
                                            },
                                        )),
                                )
                        }),
                );
            } else {
                body = body.child(
                    Input::new(&editor.input)
                        .id("cell-editor")
                        .small()
                        .h(px(26.))
                        .w_full()
                        .rounded(px(0.)),
                );
            }
        } else {
            body = body.child(div().px_2().truncate().child(value));
        }
        if active && let Some(error) = error {
            body = body.child(
                div()
                    .absolute()
                    .bottom_0()
                    .right_0()
                    .text_size(px(10.))
                    .text_color(cx.theme().foreground)
                    .bg(cx.theme().background)
                    .child(error),
            );
        }
        body
    }
}
fn editor_hit<D: EditableTableDelegate>(
    owner: &WeakEntity<EditableTableState<D>>,
    cell: &CellAddress,
    cx: &App,
) -> bool {
    owner.upgrade().is_some_and(|s| {
        s.read(cx)
            .ui
            .borrow()
            .editor
            .as_ref()
            .is_some_and(|e| &e.cell == cell)
    })
}
fn additive(m: &gpui::Modifiers) -> bool {
    if cfg!(target_os = "macos") {
        m.platform
    } else {
        m.control
    }
}
impl<D: EditableTableDelegate> EditableTableAdapter<D> {
    fn address(&self, row: usize, col: usize, _: &App) -> CellAddress {
        let ui = self.ui.borrow();
        CellAddress::new(ui.rows[row].clone(), ui.columns[col].clone())
    }
}

/// Retained spreadsheet state. Create once per view, not inside render.
pub struct EditableTableState<D: EditableTableDelegate> {
    table: Entity<TableState<EditableTableAdapter<D>>>,
    focus: FocusHandle,
    ui: Rc<RefCell<Interaction>>,
    _input_subscription: Option<Subscription>,
    scroll_generation: u64,
    scroll_pending: bool,
    scroll_last: Option<Instant>,
    _focus_subscription: Subscription,
    context_menu: Option<TableContextMenuBuilder>,
    menu_host: Entity<crate::context_menu::ContextMenuState>,
}
impl<D: EditableTableDelegate> EventEmitter<EditableTableEvent> for EditableTableState<D> {}
impl<D: EditableTableDelegate> Focusable for EditableTableState<D> {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl<D: EditableTableDelegate> EditableTableState<D> {
    pub fn new(delegate: D, window: &mut Window, cx: &mut Context<Self>) -> Self {
        if !cx.has_global::<Bindings>() {
            cx.set_global(Bindings);
            for context in ["PintleEditableTable", "PintleEditableTable > DataTable"] {
                cx.bind_keys([
                    gpui::KeyBinding::new("up", GridUp, Some(context)),
                    gpui::KeyBinding::new("down", GridDown, Some(context)),
                    gpui::KeyBinding::new("left", GridLeft, Some(context)),
                    gpui::KeyBinding::new("right", GridRight, Some(context)),
                    gpui::KeyBinding::new("shift-up", ExtendUp, Some(context)),
                    gpui::KeyBinding::new("shift-down", ExtendDown, Some(context)),
                    gpui::KeyBinding::new("shift-left", ExtendLeft, Some(context)),
                    gpui::KeyBinding::new("shift-right", ExtendRight, Some(context)),
                    gpui::KeyBinding::new("enter", GridEnter, Some(context)),
                    gpui::KeyBinding::new("shift-enter", GridBackEnter, Some(context)),
                    gpui::KeyBinding::new("tab", GridTab, Some(context)),
                    gpui::KeyBinding::new("shift-tab", GridBackTab, Some(context)),
                    gpui::KeyBinding::new("f2", GridEdit, Some(context)),
                    gpui::KeyBinding::new("alt-down", GridEdit, Some(context)),
                    gpui::KeyBinding::new(
                        if cfg!(target_os = "macos") {
                            "cmd-c"
                        } else {
                            "ctrl-c"
                        },
                        GridCopy,
                        Some(context),
                    ),
                    gpui::KeyBinding::new(
                        if cfg!(target_os = "macos") {
                            "cmd-v"
                        } else {
                            "ctrl-v"
                        },
                        GridPaste,
                        Some(context),
                    ),
                    gpui::KeyBinding::new("escape", GridCancel, Some(context)),
                ]);
            }
            for context in [
                "PintleEditableTable > DataTable > Input",
                "PintleEditableTable > Input",
            ] {
                cx.bind_keys([
                    gpui::KeyBinding::new("enter", GridEnter, Some(context)),
                    gpui::KeyBinding::new("shift-enter", GridBackEnter, Some(context)),
                    gpui::KeyBinding::new("tab", GridTab, Some(context)),
                    gpui::KeyBinding::new("shift-tab", GridBackTab, Some(context)),
                    gpui::KeyBinding::new("escape", GridCancel, Some(context)),
                ]);
            }
            cx.bind_keys([
                gpui::KeyBinding::new("up", GridUp, Some("PintleCellDropdown > Input")),
                gpui::KeyBinding::new("down", GridDown, Some("PintleCellDropdown > Input")),
            ]);
        }
        let mut interaction = Interaction::default();
        interaction.set_rows(
            (0..delegate.rows_count(cx))
                .map(|i| delegate.row_key(i, cx))
                .collect(),
        );
        interaction.set_columns(
            (0..delegate.columns_count(cx))
                .map(|i| delegate.column(i, cx).key)
                .collect(),
        );
        let ui = Rc::new(RefCell::new(interaction));
        let adapter = EditableTableAdapter {
            delegate,
            owner: cx.weak_entity(),
            ui: ui.clone(),
        };
        let table = cx.new(|cx| {
            TableState::new(adapter, window, cx)
                .row_selectable(false)
                .cell_selectable(false)
                .col_selectable(false)
                .loop_selection(false)
                .row_header(false)
        });
        let focus = cx.focus_handle();
        let focus_subscription = cx.on_focus(&focus, window, |state, window, cx| {
            let editor = state.ui.borrow().editor.clone();
            if let Some(editor) = editor {
                window.focus(&editor.input.focus_handle(cx), cx);
            }
        });
        Self {
            table,
            ui,
            focus,
            _focus_subscription: focus_subscription,
            context_menu: None,
            menu_host: cx.new(crate::context_menu::ContextMenuState::new),
            _input_subscription: None,
            scroll_generation: 0,
            scroll_pending: false,
            scroll_last: None,
        }
    }
    /// Whether the current pointer press belongs to this grid's application menu.
    pub fn context_menu_contains_pointer(&self, window: &Window, cx: &App) -> bool {
        self.menu_host.read(cx).contains_pointer(window)
    }
    /// Configure an application-owned menu; existing grids have none.
    pub fn set_context_menu(&mut self, builder: TableContextMenuBuilder) {
        self.context_menu = Some(builder);
    }
    fn open_context_menu(
        &mut self,
        cell: Option<CellAddress>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(builder) = self.context_menu.clone() else {
            return;
        };
        if !self.is_editing()
            && let Some(cell) = &cell
            && !self.ui.borrow().selection.cells.contains(cell)
        {
            self.select_cell(cell.clone(), false, false, cx);
        }
        self.menu_host.update(cx, |host, cx| {
            host.open(position, window, cx, move |menu, window, cx| {
                builder(cell, menu, window, cx)
            })
        });
    }
    /// Kit escape hatch for scrolling and column geometry. Data changes should
    /// use `update_delegate`, not mutate the adapter directly.
    pub fn table(&self) -> &Entity<TableState<EditableTableAdapter<D>>> {
        &self.table
    }
    pub fn selection(&self) -> CellSelection {
        self.ui.borrow().selection.clone()
    }
    /// Restore a stable-key selection, dropping targets no longer in this projection.
    pub fn set_selection(&mut self, mut selection: CellSelection, cx: &mut Context<Self>) {
        let mut ui = self.ui.borrow_mut();
        selection.cells.retain(|cell| {
            ui.row_positions.contains_key(&cell.row)
                && ui.column_positions.contains_key(&cell.column)
        });
        if !selection
            .active
            .as_ref()
            .is_some_and(|cell| selection.cells.contains(cell))
        {
            selection.active = selection.cells.first().cloned();
        }
        if !selection
            .anchor
            .as_ref()
            .is_some_and(|cell| selection.cells.contains(cell))
        {
            selection.anchor = selection.active.clone();
        }
        if ui.selection == selection {
            return;
        }
        ui.selection = selection;
        ui.range_base.clear();
        ui.error = None;
        drop(ui);
        self.changed(cx);
    }
    pub fn is_editing(&self) -> bool {
        self.ui.borrow().editor.is_some()
    }
    pub fn error(&self) -> Option<SharedString> {
        self.ui.borrow().error.clone()
    }
    /// Display or clear an application-level validation error, without changing
    /// the editor, selection or delegate. Applications can use this after Save
    /// validation; selecting or successfully committing a cell clears it normally.
    pub fn set_error(&mut self, error: Option<SharedString>, cx: &mut Context<Self>) {
        self.ui.borrow_mut().error = error;
        self.table.update(cx, |_, cx| cx.notify());
        cx.notify();
    }
    pub fn active_cell(&self) -> Option<CellAddress> {
        self.ui.borrow().selection.active.clone()
    }
    pub fn active_position(&self, cx: &App) -> Option<(usize, usize)> {
        self.active_cell().and_then(|c| self.position(&c, cx))
    }
    fn keys(&self, _: &App) -> (Rc<Vec<SharedString>>, Rc<Vec<SharedString>>) {
        let ui = self.ui.borrow();
        (ui.rows.clone(), ui.columns.clone())
    }
    fn position(&self, cell: &CellAddress, _: &App) -> Option<(usize, usize)> {
        let ui = self.ui.borrow();
        Some((
            *ui.row_positions.get(&cell.row)?,
            *ui.column_positions.get(&cell.column)?,
        ))
    }
    /// Reconcile stable selection after a shared source update or filter/sort.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let d = &self.table.read(cx).delegate().delegate;
        let rows: Vec<_> = (0..d.rows_count(cx)).map(|i| d.row_key(i, cx)).collect();
        let cols: Vec<_> = (0..d.columns_count(cx))
            .map(|i| d.column(i, cx).key)
            .collect();
        {
            let mut ui = self.ui.borrow_mut();
            ui.set_rows(rows);
            ui.set_columns(cols);
        }
        let mut ui = self.ui.borrow_mut();
        let before = (
            ui.selection.cells.len(),
            ui.selection.active.clone(),
            ui.selection.anchor.clone(),
        );
        let Interaction {
            selection,
            row_positions,
            column_positions,
            ..
        } = &mut *ui;
        selection.cells.retain(|c| {
            row_positions.contains_key(&c.row) && column_positions.contains_key(&c.column)
        });
        if ui.selection.active.as_ref().is_some_and(|c| {
            !ui.row_positions.contains_key(&c.row) || !ui.column_positions.contains_key(&c.column)
        }) {
            ui.selection.active = ui.selection.cells.first().cloned();
        }
        if ui.selection.anchor.as_ref().is_some_and(|c| {
            !ui.row_positions.contains_key(&c.row) || !ui.column_positions.contains_key(&c.column)
        }) {
            ui.selection.anchor = ui.selection.active.clone();
        }
        ui.drag = None;
        let selection_changed = before
            != (
                ui.selection.cells.len(),
                ui.selection.active.clone(),
                ui.selection.anchor.clone(),
            );
        drop(ui);
        self.stop_autoscroll();
        self.table.update(cx, |t, cx| {
            let widths: Vec<_> = (0..t.delegate().delegate.columns_count(cx))
                .filter_map(|i| {
                    t.col_width(i)
                        .map(|w| (t.delegate().delegate.column(i, cx).key, w))
                })
                .collect();
            t.delegate().ui.borrow_mut().widths.extend(widths);
            t.refresh(cx);
            cx.notify();
        });
        if selection_changed {
            cx.emit(EditableTableEvent::SelectionChanged(self.selection()));
        }
        cx.notify();
    }
    pub fn update_delegate<R>(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut D, &mut App) -> R,
    ) -> R {
        let result = self
            .table
            .update(cx, |table, cx| f(&mut table.delegate_mut().delegate, cx));
        self.refresh(cx);
        result
    }
    pub fn select_cell(
        &mut self,
        cell: CellAddress,
        extend: bool,
        add: bool,
        cx: &mut Context<Self>,
    ) {
        let mut ui = self.ui.borrow_mut();
        if !ui.row_positions.contains_key(&cell.row)
            || !ui.column_positions.contains_key(&cell.column)
        {
            return;
        }
        if !extend
            && !add
            && ui.selection.active.as_ref() == Some(&cell)
            && ui.selection.anchor.as_ref() == Some(&cell)
            && ui.selection.cells.len() == 1
            && ui.range_base.is_empty()
            && ui.error.is_none()
        {
            return;
        }
        if extend {
            let anchor = ui.selection.anchor.clone().unwrap_or_else(|| cell.clone());
            if !add {
                ui.selection.cells = ui.range_base.clone();
            }
            let rows = ui.rows.clone();
            let cols = ui.columns.clone();
            add_rectangle(&mut ui.selection.cells, &anchor, &cell, &rows, &cols);
            ui.selection.anchor = Some(anchor);
            ui.selection.active = Some(cell);
        } else if add {
            ui.range_base = ui.selection.cells.clone();
            ui.range_base.remove(&cell);
            if !ui.selection.cells.remove(&cell) {
                ui.selection.cells.insert(cell.clone());
            }
            ui.selection.anchor = Some(cell.clone());
            ui.selection.active = Some(cell);
        } else {
            ui.range_base.clear();
            ui.selection.select(cell);
        }
        ui.error = None;
        drop(ui);
        self.changed(cx);
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.table.update(cx, |_, cx| cx.notify());
        cx.emit(EditableTableEvent::SelectionChanged(self.selection()));
        cx.notify();
    }
    fn pointer_down(
        &mut self,
        cell: CellAddress,
        extend: bool,
        add: bool,
        point: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.commit_edit(window, cx) {
            return;
        }
        window.focus(&self.focus, cx);
        let (previous_anchor, previous_cells) = {
            let ui = self.ui.borrow();
            (
                ui.selection.anchor.clone(),
                add.then(|| ui.selection.cells.clone()),
            )
        };
        self.select_cell(cell.clone(), extend, add, cx);
        let anchor = if extend {
            previous_anchor.unwrap_or_else(|| cell.clone())
        } else {
            cell
        };
        let base = if add {
            previous_cells.unwrap_or_default()
        } else if extend {
            self.ui.borrow().range_base.clone()
        } else {
            BTreeSet::new()
        };
        self.ui.borrow_mut().drag = Some(DragSelection {
            anchor,
            base,
            pointer: point,
        });
        self.stop_autoscroll();
        self.schedule_autoscroll(window, cx);
    }
    fn drag_to(&mut self, point: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let cell = self
            .ui
            .borrow()
            .bounds
            .iter()
            .find(|(_, b)| b.contains(&point))
            .map(|(c, _)| c.clone());
        if let Some(drag) = self.ui.borrow_mut().drag.as_mut() {
            drag.pointer = point;
        }
        if let Some(cell) = cell {
            self.extend_drag(cell, cx);
        }
        self.schedule_autoscroll(window, cx);
    }
    fn extend_drag(&mut self, cell: CellAddress, cx: &mut Context<Self>) {
        if self.ui.borrow().selection.active.as_ref() == Some(&cell) {
            return;
        }
        let (rows, cols) = self.keys(cx);
        let mut ui = self.ui.borrow_mut();
        let Some(drag) = &ui.drag else {
            return;
        };
        let mut cells = drag.base.clone();
        add_rectangle(&mut cells, &drag.anchor, &cell, &rows, &cols);
        if cells == ui.selection.cells && ui.selection.active.as_ref() == Some(&cell) {
            return;
        }
        ui.selection.cells = cells;
        ui.selection.active = Some(cell);
        drop(ui);
        self.changed(cx);
    }
    fn stop_autoscroll(&mut self) {
        self.scroll_generation = self.scroll_generation.wrapping_add(1);
        self.scroll_pending = false;
        self.scroll_last = None;
    }
    fn edge_direction(&self) -> Point<f32> {
        let ui = self.ui.borrow();
        let Some(drag) = &ui.drag else {
            return point(0., 0.);
        };
        let p = drag.pointer;
        let b = ui.viewport;
        let dy = if p.y < b.top() + px(48.) {
            1.
        } else if p.y > b.bottom() - px(20.) {
            -1.
        } else {
            0.
        };
        let dx = if p.x < b.left() + px(20.) {
            1.
        } else if p.x > b.right() - px(20.) {
            -1.
        } else {
            0.
        };
        point(dx, dy)
    }
    fn schedule_autoscroll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let direction = self.edge_direction();
        if direction == point(0., 0.) {
            self.stop_autoscroll();
            return;
        }
        if self.scroll_pending {
            return;
        }
        let table = self.table.read(cx);
        let vertical = table.vertical_scroll_handle.0.borrow().base_handle.clone();
        let horizontal = &table.horizontal_scroll_handle;
        let can_move = |offset: Pixels, max: Pixels, direction: f32| {
            direction > 0. && offset < px(0.) || direction < 0. && offset > -max
        };
        if !can_move(vertical.offset().y, vertical.max_offset().y, direction.y)
            && !can_move(
                horizontal.offset().x,
                horizontal.max_offset().x,
                direction.x,
            )
        {
            self.stop_autoscroll();
            return;
        }
        self.scroll_pending = true;
        let generation = self.scroll_generation;
        self.scroll_last
            .get_or_insert_with(|| cx.background_executor().now());
        let weak = cx.weak_entity();
        window.on_next_frame(move |window, cx| {
            let _ = weak.update(cx, |state, cx| {
                if generation != state.scroll_generation {
                    return;
                }
                state.scroll_pending = false;
                let now = cx.background_executor().now();
                let elapsed = now
                    .duration_since(state.scroll_last.replace(now).unwrap_or(now))
                    .min(Duration::from_millis(50))
                    .as_secs_f32();
                let direction = state.edge_direction();
                state.table.update(cx, |table, cx| {
                    let vertical = table.vertical_scroll_handle.0.borrow().base_handle.clone();
                    let horizontal = table.horizontal_scroll_handle.base_handle().clone();
                    for (handle, delta) in [
                        (vertical, point(px(0.), px(direction.y * 562.5 * elapsed))),
                        (horizontal, point(px(direction.x * 562.5 * elapsed), px(0.))),
                    ] {
                        let offset = handle.offset();
                        let max = handle.max_offset();
                        handle.set_offset(point(
                            (offset.x + delta.x).clamp(-max.x, px(0.)),
                            (offset.y + delta.y).clamp(-max.y, px(0.)),
                        ));
                    }
                    cx.notify();
                });
                state.extend_to_pointer(cx);
                state.schedule_autoscroll(window, cx);
                if !state.scroll_pending && direction != point(0., 0.) {
                    // Paint the final offset before resolving the last edge
                    // cell, even when no further scrolling is possible.
                    let generation = state.scroll_generation;
                    let weak = cx.weak_entity();
                    window.on_next_frame(move |_, cx| {
                        let _ = weak.update(cx, |state, cx| {
                            if generation == state.scroll_generation {
                                state.extend_to_pointer(cx);
                            }
                        });
                    });
                }
            });
        });
    }
    fn extend_to_pointer(&mut self, cx: &mut Context<Self>) {
        let ui = self.ui.borrow();
        let Some(pointer) = ui.drag.as_ref().map(|drag| drag.pointer) else {
            return;
        };
        let nearest = ui
            .bounds
            .iter()
            .min_by(|(_, a), (_, b)| {
                let distance = |r: &Bounds<Pixels>| {
                    (r.center().x - pointer.x).abs() + (r.center().y - pointer.y).abs()
                };
                distance(a)
                    .partial_cmp(&distance(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(cell, _)| cell.clone());
        drop(ui);
        if let Some(cell) = nearest {
            self.extend_drag(cell, cx);
        }
    }
    pub fn begin_edit(&mut self, replace: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_editing() {
            return;
        }
        let Some(cell) = self.active_cell() else {
            return;
        };
        let d = &self.table.read(cx).delegate().delegate;
        let kind = d.editor(&cell, cx);
        let original = d.value(&cell, cx);
        if kind == CellEditor::ReadOnly {
            return;
        }
        let options = if let CellEditor::Dropdown(options) = kind {
            Some(options)
        } else {
            None
        };
        let value = if replace || options.is_some() {
            SharedString::default()
        } else {
            original.clone()
        };
        let input = cx.new(|cx| InputState::new(window, cx).default_value(value));
        self._input_subscription = Some(cx.subscribe(&input, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                if let Some(e) = this.ui.borrow_mut().editor.as_mut() {
                    e.cursor = 0;
                }
                cx.notify();
            }
        }));
        let targets = self.selection().cells.into_iter().collect();
        self.ui.borrow_mut().editor = Some(Editor {
            cell,
            targets,
            input: input.clone(),
            options,
            cursor: 0,
            scroll: gpui::ScrollHandle::new(),
        });
        self.ui.borrow_mut().error = None;
        window.focus(&input.focus_handle(cx), cx);
        self.changed(cx);
    }
    pub fn cancel_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ui.borrow_mut().editor = None;
        self.ui.borrow_mut().error = None;
        self._input_subscription = None;
        window.focus(&self.focus, cx);
        self.changed(cx);
    }
    fn choose_option(
        &mut self,
        value: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let editor = self.ui.borrow().editor.clone();
        let Some(editor) = editor else {
            return true;
        };
        if !editor
            .options
            .as_ref()
            .is_some_and(|items| items.contains(&value))
        {
            return false;
        }
        self.commit_value(editor, value, window, cx)
    }
    pub fn commit_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let editor = self.ui.borrow().editor.clone();
        let Some(editor) = editor else {
            return true;
        };
        // A dropdown query is not a value. Only choosing an option applies it
        // to the selected cells; leaving the editor keeps their values intact.
        if editor.options.is_some() {
            self.cancel_edit(window, cx);
            return true;
        }
        let value = editor.input.read(cx).value();
        self.commit_value(editor, value, window, cx)
    }
    fn commit_value(
        &mut self,
        editor: Editor,
        value: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let result = self.table.update(cx, |t, cx| {
            apply_edit(&mut t.delegate_mut().delegate, &editor, value, window, cx)
        });
        let events = match result {
            Ok(events) => events,
            Err(message) => {
                self.ui.borrow_mut().error = Some(message);
                window.focus(&editor.input.focus_handle(cx), cx);
                self.changed(cx);
                return false;
            }
        };
        self.ui.borrow_mut().editor = None;
        self.ui.borrow_mut().error = None;
        self._input_subscription = None;
        window.focus(&self.focus, cx);
        for event in events {
            cx.emit(event);
        }
        self.refresh(cx);
        true
    }
    pub fn copy(&mut self, cx: &mut Context<Self>) -> Result<(), SharedString> {
        let (rows, cols) = self.keys(cx);
        let result = selection_tsv(&rows, &cols, &self.selection(), |cell| {
            self.table
                .read(cx)
                .delegate()
                .delegate
                .value(cell, cx)
                .to_string()
        });
        match result {
            Ok(text) => {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                self.ui.borrow_mut().error = None;
                cx.notify();
                Ok(())
            }
            Err(error) => {
                let error: SharedString = error.into();
                self.ui.borrow_mut().error = Some(error.clone());
                cx.emit(EditableTableEvent::CopyFailed(error.clone()));
                cx.notify();
                Err(error)
            }
        }
    }
    /// Paste a rectangular TSV block at the active cell in display order.
    /// Rejects the entire block on overflow, read-only cells or validation errors.
    /// While an editor is open, paste belongs to Kit Input rather than the grid.
    pub fn paste_text(
        &mut self,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), SharedString> {
        let result = (|| {
            if self.is_editing() {
                return Err(SharedString::from(
                    "Finish editing before pasting a cell block",
                ));
            }
            let matrix = parse_tsv(text).map_err(SharedString::from)?;
            let (row, col) = self
                .active_position(cx)
                .ok_or_else(|| SharedString::from("Select a cell to paste"))?;
            let (rows, columns) = self.keys(cx);
            if matrix.len() > rows.len() - row || matrix[0].len() > columns.len() - col {
                return Err("Pasted cells extend beyond the visible table".into());
            }
            let changes: Vec<_> = matrix
                .into_iter()
                .enumerate()
                .flat_map(|(r, values)| {
                    let key = rows[row + r].clone();
                    let columns = &columns;
                    values.into_iter().enumerate().map(move |(c, value)| {
                        (
                            CellAddress::new(key.clone(), columns[col + c].clone()),
                            SharedString::from(value),
                        )
                    })
                })
                .collect();
            let events = self.table.update(cx, |table, cx| {
                apply_changes(&mut table.delegate_mut().delegate, &changes, window, cx)
            })?;
            {
                let mut ui = self.ui.borrow_mut();
                ui.selection.cells = changes.iter().map(|(cell, _)| cell.clone()).collect();
                ui.selection.anchor = ui.selection.active.clone();
                ui.range_base.clear();
                ui.drag = None;
                ui.error = None;
            }
            self.stop_autoscroll();
            for event in events {
                cx.emit(event);
            }
            self.refresh(cx);
            self.changed(cx);
            window.focus(&self.focus, cx);
            Ok(())
        })();
        if let Err(error) = &result {
            self.paste_error(error.clone(), cx);
        }
        result
    }
    fn paste_error(&mut self, error: SharedString, cx: &mut Context<Self>) {
        self.ui.borrow_mut().error = Some(error.clone());
        cx.emit(EditableTableEvent::PasteFailed(error));
        cx.notify();
    }
    /// Paste clipboard text using the grid's atomic validation.
    pub fn paste_clipboard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let _ = self.paste_text(&text, window, cx);
        } else {
            self.paste_error("Clipboard contains no text cells".into(), cx);
        }
    }
    fn navigate(&mut self, dr: isize, dc: isize, extend: bool, cx: &mut Context<Self>) {
        let (rows, cols) = self.keys(cx);
        if rows.is_empty() || cols.is_empty() {
            return;
        }
        let (r, c) = self
            .active_position(cx)
            .map(|(r, c)| {
                (
                    r.saturating_add_signed(dr).min(rows.len() - 1),
                    c.saturating_add_signed(dc).min(cols.len() - 1),
                )
            })
            .unwrap_or((0, 0));
        self.select_cell(
            CellAddress::new(rows[r].clone(), cols[c].clone()),
            extend,
            false,
            cx,
        );
        self.table.update(cx, |table, cx| {
            table.scroll_to_row(r, cx);
            table.scroll_to_col(c, cx);
        });
    }
    fn command(&mut self, key: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        self.key(
            &KeyDownEvent {
                keystroke: gpui::Keystroke::parse(key).expect("static grid key binding"),
                is_held: false,
                prefer_character_input: false,
            },
            window,
            cx,
        );
    }
    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let modifiers = &event.keystroke.modifiers;
        let editor = self.ui.borrow().editor.clone();
        if let Some(editor) = editor {
            if editor
                .input
                .update(cx, |input, cx| input.marked_text_range(window, cx))
                .is_some()
            {
                return;
            }
            if key == "escape" {
                self.cancel_edit(window, cx);
                cx.stop_propagation();
                return;
            }
            if let Some(options) = editor.options.clone()
                && matches!(key, "up" | "down" | "enter")
            {
                let query = editor.input.read(cx).value().to_lowercase();
                let filtered: Vec<_> = options
                    .into_iter()
                    .filter(|v| v.to_lowercase().contains(&query))
                    .collect();
                if key == "enter" {
                    if let Some(value) =
                        filtered.get(editor.cursor.min(filtered.len().saturating_sub(1)))
                    {
                        self.choose_option(value.clone(), window, cx);
                    }
                } else if let Some(e) = self.ui.borrow_mut().editor.as_mut() {
                    e.cursor = if key == "up" {
                        e.cursor.saturating_sub(1)
                    } else {
                        (e.cursor + 1).min(filtered.len().saturating_sub(1))
                    };
                }
                if let Some(e) = self.ui.borrow().editor.as_ref() {
                    e.scroll.scroll_to_item(e.cursor);
                }
                cx.stop_propagation();
                self.table.update(cx, |_, cx| cx.notify());
                cx.notify();
                return;
            }
            if matches!(key, "enter" | "tab") {
                if self.commit_edit(window, cx) {
                    let step = if modifiers.shift { -1 } else { 1 };
                    self.navigate(
                        if key == "enter" { step } else { 0 },
                        if key == "tab" { step } else { 0 },
                        false,
                        cx,
                    );
                }
                cx.stop_propagation();
            }
            return;
        }
        if additive(modifiers) && key == "c" {
            let _ = self.copy(cx);
            cx.stop_propagation();
            return;
        }
        if additive(modifiers) && key == "v" {
            self.paste_clipboard(window, cx);
            cx.stop_propagation();
            return;
        }
        if modifiers.platform || modifiers.control {
            return;
        }
        if key == "f2" || (key == "down" && modifiers.alt) {
            self.begin_edit(false, window, cx);
            cx.stop_propagation();
            return;
        }
        let direction = match key {
            "up" => Some((-1, 0)),
            "down" => Some((1, 0)),
            "left" => Some((0, -1)),
            "right" => Some((0, 1)),
            "enter" => Some((if modifiers.shift { -1 } else { 1 }, 0)),
            "tab" => Some((0, if modifiers.shift { -1 } else { 1 })),
            _ => None,
        };
        if let Some((dr, dc)) = direction {
            self.navigate(
                dr,
                dc,
                modifiers.shift && !matches!(key, "tab" | "enter"),
                cx,
            );
            cx.stop_propagation();
        } else if key == "f2" || (key == "down" && modifiers.alt) {
            self.begin_edit(false, window, cx);
            cx.stop_propagation();
        } else if key == "escape" {
            self.ui.borrow_mut().drag = None;
            self.stop_autoscroll();
            cx.stop_propagation();
        }
    }
}
// Snapshot stable keys when editing starts: source filtering/sorting caused by
// a commit cannot change the batch targets. Delegates own atomic validation and
// mutation, while the adapter enforces closed dropdowns and read-only cells.
fn apply_edit<D: EditableTableDelegate>(
    delegate: &mut D,
    editor: &Editor,
    value: SharedString,
    window: &mut Window,
    cx: &mut App,
) -> Result<Vec<EditableTableEvent>, SharedString> {
    let changes: Vec<_> = editor
        .targets
        .iter()
        .filter(|cell| delegate.editor(cell, cx) != CellEditor::ReadOnly)
        .map(|cell| (cell.clone(), value.clone()))
        .collect();
    apply_changes(delegate, &changes, window, cx)
}
// Both fill and paste use the same validation/atomic delegate contract. Paste
// rejects read-only destinations instead of silently creating a partial block.
fn apply_changes<D: EditableTableDelegate>(
    delegate: &mut D,
    requested: &[(CellAddress, SharedString)],
    window: &mut Window,
    cx: &mut App,
) -> Result<Vec<EditableTableEvent>, SharedString> {
    let mut events = Vec::new();
    for (cell, value) in requested {
        match delegate.editor(cell, cx) {
            CellEditor::ReadOnly => {
                return Err(format!("{}: cell is read-only", cell.column).into());
            }
            CellEditor::Dropdown(options) if !options.contains(value) => {
                return Err(format!("{}: choose a value from the list", cell.column).into());
            }
            _ => {}
        }
        let old_value = delegate.value(cell, cx);
        if old_value != *value {
            events.push(EditableTableEvent::Committed {
                cell: cell.clone(),
                old_value,
                value: value.clone(),
            });
        }
    }
    // An unchanged value can still be invalid in a newly inserted draft row.
    // Validate the entire request; emit activity only for actual changes.
    delegate.commit_cells(requested, window, cx)?;
    Ok(events)
}

fn add_rectangle(
    cells: &mut BTreeSet<CellAddress>,
    a: &CellAddress,
    b: &CellAddress,
    rows: &[SharedString],
    cols: &[SharedString],
) {
    let Some(r1) = rows.iter().position(|r| r == &a.row) else {
        return;
    };
    let Some(r2) = rows.iter().position(|r| r == &b.row) else {
        return;
    };
    let Some(c1) = cols.iter().position(|c| c == &a.column) else {
        return;
    };
    let Some(c2) = cols.iter().position(|c| c == &b.column) else {
        return;
    };
    for row in &rows[r1.min(r2)..=r1.max(r2)] {
        for col in &cols[c1.min(c2)..=c1.max(c2)] {
            cells.insert(CellAddress::new(row.clone(), col.clone()));
        }
    }
}

impl<D: EditableTableDelegate> Render for EditableTableState<D> {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ui.borrow_mut().bounds.clear();
        let ui = self.ui.clone();
        let focus = self.focus.clone();
        let owner = cx.entity();
        div()
            .id("editable-table")
            .test_support()
            .role(gpui::Role::Grid)
            .size_full()
            .min_h_0()
            .min_w_0()
            .relative()
            .track_focus(&self.focus)
            .key_context("PintleEditableTable")
            .capture_key_down(cx.listener(Self::key))
            .on_action(cx.listener(|state, _: &GridUp, window, cx| state.command("up", window, cx)))
            .on_action(
                cx.listener(|state, _: &GridDown, window, cx| state.command("down", window, cx)),
            )
            .on_action(
                cx.listener(|state, _: &GridLeft, window, cx| state.command("left", window, cx)),
            )
            .on_action(
                cx.listener(|state, _: &GridRight, window, cx| state.command("right", window, cx)),
            )
            .on_action(
                cx.listener(|state, _: &ExtendUp, window, cx| {
                    state.command("shift-up", window, cx)
                }),
            )
            .on_action(cx.listener(|state, _: &ExtendDown, window, cx| {
                state.command("shift-down", window, cx)
            }))
            .on_action(cx.listener(|state, _: &ExtendLeft, window, cx| {
                state.command("shift-left", window, cx)
            }))
            .on_action(cx.listener(|state, _: &ExtendRight, window, cx| {
                state.command("shift-right", window, cx)
            }))
            .on_action(
                cx.listener(|state, _: &GridEnter, window, cx| state.command("enter", window, cx)),
            )
            .on_action(cx.listener(|state, _: &GridBackEnter, window, cx| {
                state.command("shift-enter", window, cx)
            }))
            .on_action(
                cx.listener(|state, _: &GridTab, window, cx| state.command("tab", window, cx)),
            )
            .on_action(cx.listener(|state, _: &GridBackTab, window, cx| {
                state.command("shift-tab", window, cx)
            }))
            .on_action(
                cx.listener(|state, _: &GridEdit, window, cx| state.command("f2", window, cx)),
            )
            .on_action(
                cx.listener(|state, _: &GridCancel, window, cx| {
                    state.command("escape", window, cx)
                }),
            )
            .on_action(cx.listener(|state, _: &GridCopy, _, cx| {
                if state.is_editing() {
                    cx.propagate();
                } else {
                    let _ = state.copy(cx);
                }
            }))
            .on_action(cx.listener(|state, _: &GridPaste, window, cx| {
                if state.is_editing() {
                    cx.propagate();
                } else {
                    state.paste_clipboard(window, cx);
                }
            }))
            .on_mouse_move(
                cx.listener(|state, event: &gpui::MouseMoveEvent, window, cx| {
                    if event.pressed_button == Some(MouseButton::Left)
                        && state.ui.borrow().drag.is_some()
                    {
                        state.drag_to(event.position, window, cx);
                    }
                }),
            )
            .capture_any_mouse_up(cx.listener(|state, event: &gpui::MouseUpEvent, _, _| {
                if event.button == MouseButton::Left {
                    state.ui.borrow_mut().drag = None;
                    state.stop_autoscroll();
                }
            }))
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|state, _, _, _| {
                    state.ui.borrow_mut().drag = None;
                    state.stop_autoscroll();
                }),
            )
            .capture_any_mouse_down(cx.listener(
                |state, event: &gpui::MouseDownEvent, window, cx| {
                    if event.button != MouseButton::Right {
                        return;
                    }
                    let ui = state.ui.borrow();
                    let cell = ui
                        .bounds
                        .iter()
                        .find(|(_, bounds)| bounds.contains(&event.position))
                        .map(|(cell, _)| cell.clone());
                    if ui
                        .editor
                        .as_ref()
                        .is_some_and(|editor| Some(&editor.cell) == cell.as_ref())
                    {
                        return;
                    }
                    if state.context_menu.is_none()
                        || event.position.y < ui.viewport.top() + px(28.)
                    {
                        cx.stop_propagation();
                        return;
                    }
                    drop(ui);
                    cx.stop_propagation();
                    state.open_context_menu(cell, event.position, window, cx);
                },
            ))
            .on_key_down(cx.listener(|state, event: &KeyDownEvent, window, cx| {
                if crate::context_menu::is_context_menu_key(event)
                    && !state.is_editing()
                    && state.context_menu.is_some()
                {
                    let cell = state.active_cell();
                    let position = cell
                        .as_ref()
                        .and_then(|cell| {
                            state
                                .ui
                                .borrow()
                                .bounds
                                .get(cell)
                                .map(|bounds| bounds.bottom_left())
                        })
                        .unwrap_or_else(|| state.ui.borrow().viewport.center());
                    cx.stop_propagation();
                    state.open_context_menu(cell, position, window, cx);
                }
            }))
            .child(self.menu_host.clone())
            .child(
                DataTable::new(&self.table)
                    .bordered(false)
                    .with_size(Size::Size(px(28.)))
                    .header_row_height(px(28.))
                    .stripe(false),
            )
            .when_some(self.ui.borrow().error.clone(), |d, error| {
                d.child(
                    div()
                        .id("editable-table-error")
                        .test_support()
                        .absolute()
                        .bottom_1()
                        .left_1()
                        .max_w_full()
                        .px_2()
                        .py_1()
                        .bg(cx.theme().popover)
                        .text_color(cx.theme().popover_foreground)
                        .border_1()
                        .border_color(cx.theme().ring)
                        .text_size(px(12.))
                        .child(error),
                )
            })
            .child(
                canvas(
                    move |bounds, _, _| {
                        ui.borrow_mut().viewport = bounds;
                    },
                    move |bounds, _, window, cx| {
                        if focus.is_focused(window) {
                            window.handle_input(
                                &focus,
                                ElementInputHandler::new(bounds, owner),
                                cx,
                            );
                        }
                    },
                )
                .absolute()
                .inset_0(),
            )
    }
}
/// Render a retained editable table. The state owns editors and selections.
#[derive(IntoElement)]
pub struct EditableTable<D: EditableTableDelegate> {
    state: Entity<EditableTableState<D>>,
}
impl<D: EditableTableDelegate> EditableTable<D> {
    pub fn new(state: &Entity<EditableTableState<D>>) -> Self {
        Self {
            state: state.clone(),
        }
    }
}
impl<D: EditableTableDelegate> RenderOnce for EditableTable<D> {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.state
    }
}

// A grid accepts native text input before an editor exists. Forward the first
// text/composition event to Kit's InputState; subsequent events go straight to
// that focused input, preserving UTF-16 ranges and IME composition.
impl<D: EditableTableDelegate> EntityInputHandler for EditableTableState<D> {
    fn text_for_range(
        &mut self,
        _: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        *adjusted = Some(0..0);
        Some(String::new())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        None
    }
    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {}
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.begin_edit(true, window, cx);
        let editor = self.ui.borrow().editor.clone();
        if let Some(editor) = editor {
            editor.input.update(cx, |input, cx| {
                input.replace_text_in_range(range, text, window, cx)
            });
        }
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.begin_edit(true, window, cx);
        let editor = self.ui.borrow().editor.clone();
        if let Some(editor) = editor {
            editor.input.update(cx, |input, cx| {
                input.replace_and_mark_text_in_range(range, text, selected, window, cx)
            });
        }
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        self.active_cell()
            .and_then(|cell| self.ui.borrow().bounds.get(&cell).copied())
            .or(Some(bounds))
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(0)
    }
    fn paste(&mut self, item: ClipboardItem, window: &mut Window, cx: &mut Context<Self>) {
        let editor = self.ui.borrow().editor.clone();
        if let Some(editor) = editor {
            editor.input.update(cx, |input, cx| {
                EntityInputHandler::paste(input, item, window, cx)
            });
        } else if let Some(text) = item.text() {
            let _ = self.paste_text(&text, window, cx);
        } else {
            self.paste_error("Clipboard contains no text cells".into(), cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rectangles_reverse_union_and_anchor_keys() {
        let rows = ["r0", "r1", "r2"].map(SharedString::from);
        let cols = ["a", "b", "c"].map(SharedString::from);
        let mut cells = BTreeSet::new();
        add_rectangle(
            &mut cells,
            &CellAddress::new("r2", "b"),
            &CellAddress::new("r0", "a"),
            &rows,
            &cols,
        );
        assert_eq!(cells.len(), 6);
        add_rectangle(
            &mut cells,
            &CellAddress::new("r0", "a"),
            &CellAddress::new("r1", "c"),
            &rows,
            &cols,
        );
        assert_eq!(cells.len(), 8);
        cells.remove(&CellAddress::new("r1", "b"));
        assert_eq!(cells.len(), 7);
        let before = cells.clone();
        add_rectangle(
            &mut cells,
            &CellAddress::new("hidden", "a"),
            &CellAddress::new("r0", "a"),
            &rows,
            &cols,
        );
        assert_eq!(cells, before);
    }
}
