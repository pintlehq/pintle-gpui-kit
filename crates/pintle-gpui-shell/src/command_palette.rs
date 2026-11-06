//! A searchable palette with application-defined identities and actions.
use gpui::component::{
    ActiveTheme, Icon, WindowExt,
    command::{Command, CommandGroup, CommandItem, CommandState},
    dialog::DialogEntrance,
};
use gpui::{prelude::*, *};
use gpui_kit as gpui;
use std::{cell::Cell, rc::Rc};

/// Labels and search presentation use Kit's native command item.
#[derive(Clone)]
pub struct CommandEntry {
    pub id: SharedString,
    pub group: SharedString,
    pub item: CommandItem,
}
impl CommandEntry {
    pub fn new(
        id: impl Into<SharedString>,
        group: impl Into<SharedString>,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            group: group.into(),
            item: CommandItem::new().label(label),
        }
    }
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.item = self.item.icon(icon);
        self
    }
    pub fn keywords(mut self, words: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.item = self.item.keywords(words);
        self
    }
    /// Action-backed entries dispatch through the modal's action ancestry.
    pub fn action(mut self, action: Box<dyn Action>) -> Self {
        self.item = self.item.action(action);
        self
    }
}

pub struct CommandPalette {
    pub state: Entity<CommandState>,
    groups: Rc<Vec<(SharedString, Vec<CommandEntry>)>>,
}
impl CommandPalette {
    pub fn new(entries: Vec<CommandEntry>, window: &mut Window, cx: &mut App) -> Self {
        let mut palette = Self {
            state: cx.new(|cx| CommandState::new(window, cx)),
            groups: Rc::new(vec![]),
        };
        palette.set_entries(entries);
        palette
    }
    pub fn set_entries(&mut self, entries: Vec<CommandEntry>) {
        let mut groups: Vec<(SharedString, Vec<CommandEntry>)> = vec![];
        for entry in entries {
            if let Some((_, items)) = groups.iter_mut().find(|(label, _)| *label == entry.group) {
                items.push(entry);
            } else {
                groups.push((entry.group.clone(), vec![entry]));
            }
        }
        self.groups = Rc::new(groups);
    }
    /// Selection resolves the original group/row after Kit filtering to a stable ID.
    pub fn render(
        &self,
        cx: &App,
        on_select: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> gpui::base::ObservedElement<Stateful<Div>> {
        let groups = self.groups.clone();
        let mut command = Command::new(&self.state)
            .bordered(false)
            .h_full()
            .rounded(cx.theme().radius_lg)
            .placeholder("Search resources and commands...")
            .max_h(px(300.))
            .empty(|_, _, cx| {
                div()
                    .id("command-empty")
                    .test_support()
                    .p_6()
                    .text_color(cx.theme().muted_foreground)
                    .child("No results found.")
            })
            .footer(|_, _, cx| {
                div()
                    .id("command-hints")
                    .test_support()
                    .px_3()
                    .py_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("↑ ↓ Navigate · Enter Select · Esc Clear / Close")
            })
            .on_confirm(move |index, window, cx| {
                if let Some(entry) = groups
                    .get(index.section)
                    .and_then(|(_, items)| items.get(index.row))
                {
                    window.close_dialog(cx);
                    on_select(entry.id.clone(), window, cx);
                }
            });
        for (index, (label, items)) in self.groups.iter().enumerate() {
            if index > 0 {
                command = command.separator();
            }
            command = command.group(
                CommandGroup::new()
                    .label(label.clone())
                    .items(items.iter().map(|entry| entry.item.clone())),
            );
        }
        div()
            .id("workspace-command-palette")
            .h(px(360.))
            .test_support()
            .child(command)
    }
    /// Present the palette with one-time focus after the modal mounts.
    /// The renderer may attach application action handlers to the returned element.
    pub fn open(
        &self,
        window: &mut Window,
        cx: &mut App,
        render: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) {
        if window.has_active_dialog(cx) {
            return;
        }
        self.state
            .update(cx, |state, cx| state.set_query("", window, cx));
        let state = self.state.clone();
        let focus_pending = Rc::new(Cell::new(true));
        let render = Rc::new(render);
        window.open_dialog(cx, move |dialog, _, _| {
            let state = state.clone();
            let focus_pending = focus_pending.clone();
            let render = render.clone();
            dialog
                .centered()
                .entrance(DialogEntrance::None)
                .width(px(560.))
                .close_button(false)
                .p_0()
                .content(move |content, window, cx| {
                    if focus_pending.replace(false) {
                        let state = state.clone();
                        window.defer(cx, move |window, cx| {
                            state.update(cx, |state, cx| state.focus(window, cx))
                        });
                    }
                    content.child(render(window, cx))
                })
        });
    }
}
