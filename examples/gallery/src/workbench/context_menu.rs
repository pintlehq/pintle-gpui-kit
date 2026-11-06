use super::*;
use gpui::component::{WindowExt, input::InputEvent, menu::PopupMenu};
use pintle_gpui_kit::shell::workspace::{ClosePolicy, SplitDirection};

pub(super) struct RenameState {
    id: SharedString,
    input: Entity<InputState>,
    error: Option<SharedString>,
    _subscription: gpui::Subscription,
}
pub(super) struct UnavailableResource {
    pub focus: FocusHandle,
}
impl Focusable for UnavailableResource {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for UnavailableResource {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .p_4()
            .track_focus(&self.focus)
            .child("Resource no longer available")
    }
}

impl Workbench {
    pub(super) fn connect_navigation_menu(&mut self, cx: &mut Context<Self>) {
        let owner = cx.weak_entity();
        self.navigation.update(cx, |tree, _| {
            tree.set_context_menu(Rc::new(move |id, menu, _, cx| {
                let Some(view) = owner.upgrade() else {
                    return menu;
                };
                view.read(cx)
                    .navigation_menu(id, menu, view.downgrade(), cx)
            }))
        });
    }
    fn navigation_menu(
        &self,
        id: SharedString,
        mut menu: PopupMenu,
        owner: gpui::WeakEntity<Self>,
        cx: &App,
    ) -> PopupMenu {
        let catalog = self.catalog.read(cx);
        let Some(node) = catalog.node(&id) else {
            return menu;
        };
        let item = |menu: PopupMenu, label: &'static str, command: &'static str| {
            let owner = owner.clone();
            let id = id.clone();
            menu.item(PopupMenuItem::new(label).on_click(move |_, window, cx| {
                let _ = owner.update(cx, |this, cx| {
                    this.navigation_command(&id, command, window, cx)
                });
            }))
        };
        if node.document.is_some() {
            menu = item(menu, "Open in New Tab", "open");
            menu = item(menu, "Open in Split Right", "right");
            menu = item(menu, "Open in Split Down", "down");
        } else {
            menu = item(
                menu,
                if self.navigation.read(cx).is_expanded(&id, cx) {
                    "Collapse"
                } else {
                    "Expand"
                },
                "toggle",
            );
        }
        menu = item(menu.separator(), "Copy Path", "copy");
        if catalog.editable(&id) {
            menu = item(menu.separator(), "Rename", "rename");
            menu = item(menu, "Delete", "delete");
        }
        menu
    }
    fn navigation_command(
        &mut self,
        id: &SharedString,
        command: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(node) = self.catalog.read(cx).node(id).cloned() else {
            return;
        };
        match command {
            "copy" => cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                self.catalog
                    .read(cx)
                    .path(id)
                    .iter()
                    .map(|part| part.as_ref())
                    .collect::<Vec<_>>()
                    .join("/"),
            )),
            "toggle" => self
                .navigation
                .update(cx, |tree, cx| tree.toggle_node(id, cx)),
            "rename" => self.open_rename(id.clone(), window, cx),
            "delete" => self.open_delete(id.clone(), window, cx),
            "open" | "right" | "down" => {
                if let Some(document) = node.document {
                    self.workspace.update(cx, |workspace, cx| {
                        if command == "open" {
                            workspace.open(document, OpenMode::Permanent, window, cx);
                        } else {
                            workspace.open_in_split(
                                document,
                                if command == "right" {
                                    SplitDirection::Right
                                } else {
                                    SplitDirection::Down
                                },
                                window,
                                cx,
                            );
                        }
                    });
                }
            }
            _ => {}
        }
    }
    pub(super) fn synchronize_catalog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let nodes = self.catalog.read(cx).nodes.clone();
        self.command_palette.refresh(&nodes);
        self.navigation
            .update(cx, |tree, cx| tree.set_nodes(nodes, cx));
        let documents: Vec<_> = ["database", "kubernetes"]
            .into_iter()
            .flat_map(|id| self.catalog.read(cx).documents(id))
            .collect();
        self.workspace.update(cx, |workspace, cx| {
            for document in &documents {
                workspace.update_document_metadata(document, cx);
            }
        });
        self.inspector.update(cx, |inspector, cx| {
            inspector.refresh_details(cx);
            cx.notify();
        });
        let _ = window;
        cx.notify();
    }
    fn open_rename(&mut self, id: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        if window.has_active_dialog(cx) || !self.catalog.read(cx).editable(&id) {
            return;
        }
        let name = self.catalog.read(cx).node(&id).unwrap().label.clone();
        let input = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let subscription =
            cx.subscribe_in(&input, window, |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.submit_rename(window, cx);
                }
            });
        self.rename = Some(RenameState {
            id,
            input: input.clone(),
            error: None,
            _subscription: subscription,
        });
        let owner = cx.weak_entity();
        let submit = owner.clone();
        let focus_pending = Rc::new(std::cell::Cell::new(true));
        window.open_dialog(cx, move |dialog, _, _| {
            let owner = owner.clone();
            let focus_pending = focus_pending.clone();
            let submit = submit.clone();
            dialog
                .title("Rename node")
                .on_ok(|_, _, _| false)
                .centered()
                .width(px(420.))
                .close_button(false)
                .content(move |content, window, cx| {
                    let Some(view) = owner.upgrade() else {
                        return content;
                    };
                    let Some(rename) = &view.read(cx).rename else {
                        return content;
                    };
                    let input = rename.input.clone();
                    let error = rename.error.clone();
                    if focus_pending.replace(false) {
                        let input = input.clone();
                        window.defer(cx, move |window, cx| {
                            window.focus(&input.focus_handle(cx), cx)
                        });
                    }
                    content
                        .child(
                            Input::new(&input)
                                .id("rename-node-input")
                                .aria_label("Node name"),
                        )
                        .children(error.map(|error| {
                            div()
                                .id("rename-node-error")
                                .test_support()
                                .text_color(cx.theme().danger)
                                .child(error)
                        }))
                })
                .footer(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            Button::new("rename-node-cancel")
                                .label("Cancel")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("rename-node-confirm")
                                .primary()
                                .label("Rename")
                                .on_click(move |_, window, cx| {
                                    let _ = submit
                                        .update(cx, |this, cx| this.submit_rename(window, cx));
                                }),
                        ),
                )
        });
    }
    fn submit_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !window.has_active_dialog(cx) {
            return;
        }
        let Some(rename) = &self.rename else {
            return;
        };
        let id = rename.id.clone();
        let name = rename.input.read(cx).value();
        if self
            .catalog
            .read(cx)
            .node(&id)
            .is_some_and(|node| node.label == name.trim())
        {
            self.rename = None;
            window.close_dialog(cx);
            return;
        }
        let result = self.catalog.update(cx, |catalog, cx| {
            let result = catalog.rename(&id, &name);
            if result.is_ok() {
                cx.notify();
            }
            result
        });
        match result {
            Ok(()) => {
                self.rename = None;
                window.close_dialog(cx);
            }
            Err(error) => {
                if let Some(rename) = &mut self.rename {
                    rename.error = Some(error);
                    window.focus(&rename.input.focus_handle(cx), cx);
                }
                window.refresh();
            }
        }
    }
    fn open_delete(&mut self, id: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        if window.has_active_dialog(cx) || !self.catalog.read(cx).editable(&id) {
            return;
        }
        let catalog = self.catalog.read(cx);
        let label = catalog.node(&id).unwrap().label.clone();
        let documents = catalog.documents(&id);
        let keys: Vec<_> = documents.iter().map(|doc| doc.key.clone()).collect();
        let tabs = self
            .workspace
            .read(cx)
            .groups(cx)
            .iter()
            .flat_map(|group| &group.views)
            .filter(|view| {
                self.workspace
                    .read(cx)
                    .document(**view, cx)
                    .is_some_and(|doc| keys.contains(&doc.key))
            })
            .count();
        let mut counts = (0, 0, 0, 0);
        for key in &keys {
            if let Some((_, store)) = catalog.sources.get(key) {
                let (i, u, d, e) = store.read(cx).draft_summary(cx);
                counts.0 += i;
                counts.1 += u;
                counts.2 += d;
                counts.3 += e;
            }
        }
        let message = format!(
            "Delete {label} and its subtree for this session? Resources: {} · Tabs: {tabs}\nDrafts: +{} · ~{} · −{} · {} open editors. Pending changes will be discarded.",
            documents.len(),
            counts.0,
            counts.1,
            counts.2,
            counts.3
        );
        let owner = cx.weak_entity();
        window.open_dialog(cx, move |dialog, _, _| {
            let owner = owner.clone();
            let id = id.clone();
            let confirm_owner = owner.clone();
            let confirm_id = id.clone();
            dialog
                .title("Delete node")
                .on_ok(move |_, window, cx| {
                    window.close_dialog(cx);
                    let _ = confirm_owner
                        .update(cx, |this, cx| this.delete_node(&confirm_id, window, cx));
                    false
                })
                .centered()
                .width(px(480.))
                .close_button(false)
                .child(
                    div()
                        .id("delete-node-summary")
                        .test_support()
                        .child(message.clone()),
                )
                .footer(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            Button::new("delete-node-cancel")
                                .label("Cancel")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("delete-node-confirm")
                                .danger()
                                .label("Delete")
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    let _ = owner
                                        .update(cx, |this, cx| this.delete_node(&id, window, cx));
                                }),
                        ),
                )
        });
    }
    fn delete_node(&mut self, id: &SharedString, window: &mut Window, cx: &mut Context<Self>) {
        if !self.catalog.read(cx).editable(id) {
            return;
        }
        let documents = self.catalog.read(cx).documents(id);
        let keys: Vec<_> = documents.iter().map(|doc| doc.key.clone()).collect();
        if !self.workspace.update(cx, |workspace, cx| {
            workspace.close_resources(&keys, ClosePolicy::Discard, window, cx)
        }) {
            if let Some(focus) = window.focused(cx) {
                window.on_next_frame(move |window, cx| window.focus(&focus, cx));
            }
            return;
        }
        for key in &keys {
            if let Some((_, store)) = self.catalog.read(cx).sources.get(key).cloned() {
                for grid in store.read(cx).open_editors() {
                    if grid.read(cx).is_editing() {
                        grid.update(cx, |grid, cx| grid.cancel_edit(window, cx));
                    }
                }
                store.update(cx, |store, cx| {
                    store.discard_draft();
                    cx.notify();
                });
            }
        }
        self.catalog.update(cx, |catalog, cx| {
            catalog.remove(id);
            cx.notify();
        });
        let tree = self.navigation.clone();
        window.on_next_frame(move |window, cx| tree.update(cx, |tree, cx| tree.focus(window, cx)));
    }
}
