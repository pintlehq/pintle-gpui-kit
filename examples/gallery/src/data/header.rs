//! Resource-specific controls; mutations remain owned by the Gallery store.
use super::*;
use gpui::component::{checkbox::Checkbox, popover::Popover, toolbar::Toolbar};

impl ResourceView {
    pub(super) fn editors(&self, cx: &App) -> Vec<Entity<EditableTableState<Rows>>> {
        self.store
            .read(cx)
            .editors
            .iter()
            .filter_map(|weak| weak.upgrade())
            .collect()
    }
    fn commit_editors(&self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.editors(cx)
            .into_iter()
            .all(|grid| grid.update(cx, |grid, cx| grid.commit_edit(window, cx)))
    }
    fn reveal_cell(&mut self, cell: CellAddress, window: &mut Window, cx: &mut Context<Self>) {
        self.filter
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.editable.update(cx, |grid, cx| {
            grid.update_delegate(cx, |rows, cx| {
                rows.query.clear();
                rows.rebuild(cx);
            });
            grid.select_cell(cell.clone(), false, false, cx);
            if let Some((row, col)) = grid.active_position(cx) {
                grid.table().update(cx, |table, cx| {
                    table.scroll_to_row(row, cx);
                    table.scroll_to_col(col, cx);
                });
            }
            grid.begin_edit(false, window, cx);
        });
    }
    pub(super) fn clear_query(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self
            .editable
            .update(cx, |grid, cx| grid.commit_edit(window, cx))
        {
            return;
        }
        self.filter
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.editable.update(cx, |grid, cx| {
            grid.update_delegate(cx, |rows, cx| {
                rows.query.clear();
                rows.rebuild(cx);
            })
        });
        window.focus(&self.filter.focus_handle(cx), cx);
        cx.notify();
    }
    pub(super) fn add_row(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.commit_editors(window, cx) {
            return;
        }
        let data = self.table.read(cx).delegate().data.clone();
        let key = self.store.update(cx, |store, cx| {
            let key = store.add(&data);
            cx.notify();
            key
        });
        self.reveal_cell(CellAddress::new(key, data.columns[1]), window, cx);
    }
    pub(super) fn delete_rows(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let keys = self.selected_rows.clone();
        if keys.is_empty() || !self.commit_editors(window, cx) {
            return;
        }
        let previous = self
            .editable
            .read(cx)
            .active_position(cx)
            .map(|p| p.0)
            .unwrap_or(0);
        self.store.update(cx, |store, cx| {
            store.delete(&keys);
            cx.notify();
        });
        self.editable.update(cx, |grid, cx| {
            grid.update_delegate(cx, |rows, cx| rows.rebuild(cx));
            let rows = grid.table().read(cx).delegate();
            let row = previous.min(rows.indices.len().saturating_sub(1));
            let key = rows
                .indices
                .get(row)
                .map(|i| rows.store.read(cx).row_keys[*i].clone());
            if let Some(key) = key {
                grid.select_cell(
                    CellAddress::new(key, rows.data.columns[1]),
                    false,
                    false,
                    cx,
                );
            }
        });
        window.focus(&self.editable.focus_handle(cx), cx);
    }
    pub(super) fn save_rows(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.commit_editors(window, cx) {
            return;
        }
        let data = self.table.read(cx).delegate().data.clone();
        let invalid = data
            .columns
            .iter()
            .position(|column| *column == "name")
            .and_then(|col| {
                let store = self.store.read(cx);
                store
                    .values
                    .iter()
                    .position(|row| row[col].trim().is_empty())
                    .map(|row| CellAddress::new(store.row_keys[row].clone(), "name"))
            });
        if let Some(cell) = invalid {
            self.reveal_cell(cell, window, cx);
            self.editable.update(cx, |grid, cx| {
                grid.set_error(Some("Name is required".into()), cx);
            });
            return;
        }
        self.store.update(cx, |store, cx| {
            store.save();
            cx.notify();
        });
        window.focus(&self.editable.focus_handle(cx), cx);
    }
    pub(super) fn revert_rows(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for grid in self.editors(cx) {
            if grid.read(cx).is_editing() {
                grid.update(cx, |grid, cx| grid.cancel_edit(window, cx));
            }
        }
        self.store.update(cx, |store, cx| {
            store.revert();
            cx.notify();
        });
        window.focus(&self.editable.focus_handle(cx), cx);
    }
    pub(super) fn apply_namespaces(
        &mut self,
        selected: BTreeSet<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let accepted = self.editable.update(cx, |grid, cx| {
            if !grid.commit_edit(window, cx) {
                return false;
            }
            grid.update_delegate(cx, |rows, cx| {
                rows.namespaces = selected;
                rows.rebuild(cx);
            });
            true
        });
        if !accepted {
            self.namespace_open = false;
            let grid = self.editable.clone();
            window.on_next_frame(move |window, cx| window.focus(&grid.focus_handle(cx), cx));
        }
        cx.notify();
    }
    fn namespace_dropdown(&self, cx: &mut Context<Self>) -> Popover {
        let selected = &self.table.read(cx).delegate().namespaces;
        let label = match selected.len() {
            0 => "Namespace: All".to_owned(),
            1 => format!("Namespace: {}", selected.first().unwrap()),
            n => format!("Namespaces: {n} selected"),
        };
        let owner = cx.weak_entity();
        let change = owner.clone();
        Popover::new(("namespace-popover", cx.entity_id()))
            .open(self.namespace_open)
            .w(px(280.))
            .p_0()
            .on_open_change(move |open, window, cx| {
                let _ = change.update(cx, |this, cx| {
                    this.namespace_open = *open;
                    if !open {
                        window.focus(&this.namespace_focus, cx);
                    }
                    cx.notify();
                });
            })
            .trigger(
                Button::new("namespace-trigger")
                    .outline()
                    .small()
                    .h(px(28.))
                    .track_focus(&self.namespace_focus)
                    .label(label.clone())
                    .icon(IconName::Funnel)
                    .accessibility_label(label),
            )
            .content(move |_, _, cx| {
                let Some(view) = owner.upgrade() else {
                    return div().into_any_element();
                };
                let this = view.read(cx);
                let selected = this.table.read(cx).delegate().namespaces.clone();
                let counts = &this.store.read(cx).namespaces;
                let options: BTreeSet<_> = counts
                    .keys()
                    .cloned()
                    .chain(selected.iter().cloned())
                    .collect();
                let clear = owner.clone();
                div()
                    .id("namespace-menu")
                    .test_support()
                    .p_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_2()
                            .py_1()
                            .child("Namespaces")
                            .child(
                                Button::new("namespace-clear")
                                    .ghost()
                                    .small()
                                    .label("Clear")
                                    .on_click(move |_, window, cx| {
                                        let _ = clear.update(cx, |this, cx| {
                                            this.apply_namespaces(BTreeSet::new(), window, cx)
                                        });
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .id("namespace-options")
                            .max_h(px(320.))
                            .overflow_y_scroll()
                            .children(options.into_iter().map(|name| {
                                let checked = selected.contains(&name);
                                let label =
                                    format!("{name} ({})", counts.get(&name).copied().unwrap_or(0));
                                let owner = owner.clone();
                                Checkbox::new(format!("namespace-option:{name}"))
                                    .checked(checked)
                                    .label(label)
                                    .small()
                                    .accessibility_label(name.clone())
                                    .w_full()
                                    .h(px(32.))
                                    .px_2()
                                    .on_click(move |checked, window, cx| {
                                        let _ = owner.update(cx, |this, cx| {
                                            let mut selected =
                                                this.table.read(cx).delegate().namespaces.clone();
                                            if *checked {
                                                selected.insert(name.clone());
                                            } else {
                                                selected.remove(&name);
                                            }
                                            this.apply_namespaces(selected, window, cx);
                                        });
                                    })
                            })),
                    )
                    .into_any_element()
            })
    }
    pub(super) fn resource_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let rows = self.table.read(cx).delegate().indices.len();
        let mut toolbar = Toolbar::new(("resource-controls", cx.entity_id()))
            .small()
            .h(px(32.))
            .p_0()
            .gap_1();
        if self.kind == ResourceKind::Database {
            let pending = self.store.read(cx).dirty()
                || self
                    .editors(cx)
                    .iter()
                    .any(|grid| grid.read(cx).is_editing());
            if !pending {
                self.cancel_hitbox.set(None);
            }
            let cancel_hitbox = self.cancel_hitbox.clone();
            toolbar = toolbar
                .child(
                    Button::new("db-add-row")
                        .ghost()
                        .icon(IconName::Plus)
                        .tooltip("Add row")
                        .accessibility_label("Add row")
                        .on_click(cx.listener(|this, _, window, cx| this.add_row(window, cx))),
                )
                .child(
                    Button::new("db-delete-rows")
                        .ghost()
                        .icon(IconName::Trash)
                        .tooltip("Delete selected rows")
                        .accessibility_label("Delete selected rows")
                        .disabled(self.selected_rows.is_empty())
                        .on_click(cx.listener(|this, _, window, cx| this.delete_rows(window, cx))),
                )
                .content(div().w(px(1.)).h(px(18.)).bg(cx.theme().border).mx_1())
                .child(
                    Button::new("db-save")
                        .ghost()
                        .icon(IconName::Save)
                        .tooltip("Save session changes")
                        .accessibility_label("Save session changes")
                        .disabled(!pending)
                        .on_click(cx.listener(|this, _, window, cx| this.save_rows(window, cx))),
                )
                .content(
                    div()
                        .on_children_prepainted(move |children, window, _| {
                            cancel_hitbox.set(children.first().map(|bounds| {
                                window
                                    .insert_hitbox(*bounds, gpui::HitboxBehavior::Normal)
                                    .id
                            }));
                        })
                        .id("db-cancel-control")
                        .flex()
                        .items_center()
                        .flex_shrink_0()
                        .child(
                            Button::new("db-cancel")
                                .small()
                                .ghost()
                                .icon(IconName::Undo2)
                                .tooltip("Discard pending table changes")
                                .accessibility_label("Discard pending table changes")
                                .disabled(!pending)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.revert_rows(window, cx)),
                                ),
                        ),
                );
            let (inserted, updated, deleted) = self.store.read(cx).changes();
            toolbar = toolbar.content(
                div()
                    .id("db-draft-status")
                    .test_support()
                    .text_xs()
                    .whitespace_nowrap()
                    .text_color(cx.theme().muted_foreground)
                    .child(if pending && (inserted, updated, deleted) == (0, 0, 0) {
                        "Editing".to_owned()
                    } else if pending {
                        format!("+{inserted} · ~{updated} · −{deleted}")
                    } else {
                        "Saved".to_owned()
                    }),
            );
        } else {
            toolbar = toolbar.content(self.namespace_dropdown(cx));
        }
        let label = if self.kind == ResourceKind::Database {
            format!("Filter {}", self.document.title)
        } else {
            format!("Search {}", self.document.title)
        };
        toolbar = toolbar
            .content(
                div().flex_1().min_w(px(180.)).max_w(px(448.)).child(
                    Input::new(&self.filter)
                        .id("row-filter")
                        .aria_label(label)
                        .small()
                        .map(|input| gpui::Styled::h(input, px(28.)))
                        .bg(cx.theme().background)
                        .text_size(px(12.5))
                        .prefix(Icon::new(IconName::Search).size(px(14.))),
                ),
            )
            .content(div().flex_1())
            .content(
                div()
                    .id("row-count")
                    .test_support()
                    .text_size(px(12.))
                    .whitespace_nowrap()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{rows} rows")),
            )
            .child(
                Button::new("clear-row-filter")
                    .ghost()
                    .icon(IconName::FunnelX)
                    .accessibility_label("Clear filter")
                    .tooltip("Clear search")
                    .disabled(self.filter.read(cx).value().is_empty())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.clear_query(window, cx);
                    })),
            );
        div()
            .id("resource-toolbar")
            .test_support()
            .overflow_x_scroll()
            .min_w_0()
            .h(px(32.))
            .px_3()
            .flex_shrink_0()
            .child(toolbar.w_full())
    }
}
