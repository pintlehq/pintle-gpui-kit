//! Gallery catalog adapter for the shared command palette.
use super::*;
use gpui::WeakEntity;
use pintle_gpui_kit::shell::{
    command_palette::{CommandEntry, CommandPalette as Palette},
    workspace::NavigationNode,
};
pub(super) struct CommandPalette {
    palette: Palette,
}
impl CommandPalette {
    pub(super) fn new(
        nodes: &[NavigationNode],
        window: &mut Window,
        cx: &mut Context<Workbench>,
    ) -> Self {
        Self {
            palette: Palette::new(Self::entries(nodes), window, cx),
        }
    }
    fn entries(nodes: &[NavigationNode]) -> Vec<CommandEntry> {
        fn collect(
            nodes: &[NavigationNode],
            group: &SharedString,
            entries: &mut Vec<CommandEntry>,
        ) {
            for node in nodes {
                if let Some(doc) = &node.document {
                    entries.push(
                        CommandEntry::new(
                            doc.key.0.clone(),
                            group.clone(),
                            format!("{} · {}", doc.title, doc.context),
                        )
                        .icon(node.icon.clone())
                        .keywords([doc.key.0.clone(), group.clone()]),
                    );
                }
                collect(&node.children, group, entries);
            }
        }
        let mut entries = vec![];
        for node in nodes {
            collect(std::slice::from_ref(node), &node.label, &mut entries);
        }
        entries.extend([
            CommandEntry::new("toggle-sidebar", "Commands", "Toggle Sidebar")
                .icon(IconName::Files)
                .action(Box::new(ToggleSidebar)),
            CommandEntry::new("toggle-inspector", "Commands", "Toggle Inspector")
                .icon(IconName::PanelBottom)
                .action(Box::new(ToggleInspector)),
            CommandEntry::new("toggle-terminal", "Commands", "Toggle Terminal")
                .icon(IconName::Terminal)
                .action(Box::new(ToggleTerminal)),
            CommandEntry::new("new-terminal", "Commands", "New Terminal")
                .icon(IconName::Plus)
                .action(Box::new(NewTerminal)),
        ]);
        entries
    }
    pub(super) fn refresh(&mut self, nodes: &[NavigationNode]) {
        self.palette.set_entries(Self::entries(nodes));
    }
    fn render(&self, owner: WeakEntity<Workbench>, cx: &App) -> impl IntoElement + use<> {
        let confirm_owner = owner.clone();
        let sidebar_owner = owner.clone();
        let terminal_owner = owner.clone();
        let new_terminal_owner = owner.clone();
        self.palette
            .render(cx, move |id, window, cx| {
                let _ = confirm_owner.update(cx, |this, cx| {
                    let document = this
                        .catalog
                        .read(cx)
                        .node(&id)
                        .and_then(|node| node.document.clone());
                    if let Some(document) = document {
                        this.workspace.update(cx, |workspace, cx| {
                            workspace.open(document, OpenMode::Permanent, window, cx);
                        });
                    }
                });
            })
            .on_action(|_: &OpenCommandPalette, _, _| {})
            .on_action(move |_: &ToggleSidebar, window, cx| {
                window.close_dialog(cx);
                let _ = sidebar_owner.update(cx, |this, cx| this.toggle_sidebar(window, cx));
            })
            .on_action(move |_: &ToggleTerminal, window, cx| {
                window.close_dialog(cx);
                let _ = terminal_owner.update(cx, |this, cx| {
                    this.inspector
                        .update(cx, |i, cx| i.toggle_terminal(window, cx));
                });
            })
            .on_action(move |_: &NewTerminal, window, cx| {
                window.close_dialog(cx);
                let _ = new_terminal_owner.update(cx, |this, cx| {
                    this.inspector
                        .update(cx, |i, cx| i.new_terminal(window, cx));
                });
            })
            .on_action(move |_: &ToggleInspector, window, cx| {
                window.close_dialog(cx);
                let _ = owner.update(cx, |this, cx| {
                    this.inspector
                        .update(cx, |inspector, cx| inspector.toggle(window, cx));
                });
            })
    }
}
impl Workbench {
    pub(super) fn open_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let owner = cx.entity().downgrade();
        self.command_palette.palette.open(window, cx, move |_, cx| {
            if let Some(entity) = owner.upgrade() {
                entity
                    .read(cx)
                    .command_palette
                    .render(owner.clone(), cx)
                    .into_any_element()
            } else {
                div().into_any_element()
            }
        });
    }
}
