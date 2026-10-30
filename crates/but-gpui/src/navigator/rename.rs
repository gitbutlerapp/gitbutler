//! A branch's or a terminal's name edited in its row, as Lite does, rather than in a prompt.

use gpui::{
    AnyElement, App, Context, Entity, Focusable as _, MouseButton, Subscription, Window, div,
    prelude::*,
};

use super::Navigator;
use crate::backend::{FullName, short_ref};
use crate::overlays::branch_name;
use crate::state::{ProjectId, Selection, snack};
use crate::terminal::TerminalId;
use crate::theme::Theme;
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{dp, field};

/// The row whose name is being edited, a branch's or a terminal's, and the field it's edited
/// in.
pub(super) struct Renaming {
    item: Selection,
    input: Entity<TextInput>,
    _subscriptions: [Subscription; 3],
}

impl Navigator {
    /// Edit the name of branch `ref_name` in its row. Enter runs `submit` with the name typed,
    /// unless it's the name it has, or one it can't have: then the snackbar says why and the
    /// field stays. Escape, or a click anywhere else, leaves it as it was.
    pub(crate) fn rename(
        &mut self,
        project: ProjectId,
        ref_name: Option<FullName>,
        window: &mut Window,
        cx: &mut Context<Self>,
        submit: impl FnOnce(String, &mut App) + 'static,
    ) {
        let current = ref_name
            .as_ref()
            .map_or(String::new(), |r| short_ref(r).into_owned());
        let input = cx.new(|cx| {
            let mut input = TextInput::new("Branch name", false, cx).spaces_as_dashes();
            input.load(current.clone(), cx);
            input.select_all_text(cx);
            input
        });
        let (checked, mut submit) = (project.clone(), Some(submit));
        let typed = cx.subscribe_in(&input, window, move |this, input, event, window, cx| {
            match event {
                InputEvent::Submit => {
                    let name = input.read(cx).text().to_owned();
                    match branch_name(this.store.read(cx), &checked, &name, &current) {
                        Err(why) => snack(&this.store, why, cx),
                        Ok(short) => {
                            this.renaming = None;
                            this.focus_list(window, cx);
                            // Renaming to the name it has does nothing.
                            if let Some(submit) = submit.take().filter(|_| short != current) {
                                submit(name, cx)
                            }
                        }
                    }
                }
                InputEvent::Cancel => {
                    this.renaming = None;
                    this.focus_list(window, cx)
                }
                InputEvent::Changed => {}
            }
        });
        let item = Selection::Branch(project, ref_name);
        self.edit_in_row(item, input, typed, window, cx)
    }

    /// Edit the name of terminal `id` in its row. Enter names it what's typed, even the title it
    /// shows, which its shell's titles then don't change; with nothing typed, the shell's title
    /// shows again. Escape, or a click anywhere else, leaves it as it was.
    pub(crate) fn rename_terminal(
        &mut self,
        project: ProjectId,
        id: TerminalId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(current) = self.store.read(cx).terminals.title(id).cloned() else {
            return;
        };
        let input = cx.new(|cx| {
            let mut input = TextInput::new("Terminal name", false, cx);
            input.load(current.to_string(), cx);
            input.select_all_text(cx);
            input
        });
        let typed = cx.subscribe_in(&input, window, move |this, input, event, window, cx| {
            let name = match event {
                InputEvent::Submit => Some(input.read(cx).text().to_owned()),
                InputEvent::Cancel => None,
                InputEvent::Changed => return,
            };
            this.renaming = None;
            this.focus_list(window, cx);
            if let Some(name) = name {
                (this.store).update(cx, |store, cx| store.name_terminal(id, &name, cx))
            }
        });
        self.edit_in_row(Selection::Terminal(project, id), input, typed, window, cx)
    }

    /// Edit the name of `item`, the selection, in `input` in its row, `typed` taking its keys.
    fn edit_in_row(
        &mut self,
        item: Selection,
        input: Entity<TextInput>,
        typed: Subscription,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // The keys go where the click went. GPUI blurs it as another app comes in front too,
        // which leaves it as it is for when the window's back.
        let left = cx.on_blur(&input.focus_handle(cx), window, |this, window, cx| {
            if window.is_window_active() {
                this.renaming = None;
                cx.notify()
            }
        });
        let moved = cx.observe(&self.store, |this, _, cx| this.end_rename_if_moved(cx));
        window.focus(&input.focus_handle(cx), cx);
        // The row is the selection: scrolled to, its field is drawn to take the keys. A filter
        // that hides it goes first, as the palette can rename it from there.
        if self.filtering(cx) && !self.items().contains(&&item) {
            self.filter.update(cx, |input, cx| input.set_text("", cx));
            self.filter_changed(cx);
        }
        self.reveal_selection = true;
        self.renaming = Some(Renaming {
            item,
            input,
            _subscriptions: [typed, left, moved],
        });
        cx.notify();
    }

    /// Stop editing a name once something else is selected, as a click on another row or the
    /// window coming back from another app with a click elsewhere does, or once a branch is
    /// gone, as when an agent renamed it, saying so. A terminal that closes takes the selection
    /// with it.
    fn end_rename_if_moved(&mut self, cx: &mut Context<Self>) {
        let Some(item) = self.renaming.as_ref().map(|r| &r.item) else {
            return;
        };
        let store = self.store.read(cx);
        let why = match item {
            Selection::Branch(project, ref_name) => (store.workspace(project))
                .is_some_and(|ws| ws.find_branch(ref_name.as_ref()).is_none())
                .then(|| {
                    let name = ref_name.as_ref().map_or("The branch".into(), short_ref);
                    format!("{name} was renamed or left the workspace meanwhile")
                }),
            Selection::Terminal(..) => None,
            // Not renamed in their row.
            Selection::Project(_)
            | Selection::Uncommitted(_)
            | Selection::Commit(..)
            | Selection::Branches(_)
            | Selection::History(_)
            | Selection::Graph(_)
            | Selection::Worktree(..)
            | Selection::Stack(..)
            | Selection::Mesh(..)
            | Selection::Upstream(..)
            | Selection::Section(..) => None,
        };
        if store.selection.as_ref() == Some(item) && why.is_none() {
            return;
        }
        self.renaming = None;
        if let Some(why) = why {
            snack(&self.store, why, cx);
        }
    }

    /// The field the name of `item`'s row is being edited in, in place of the name.
    pub(super) fn rename_field(&self, item: &Selection, theme: &Theme) -> Option<AnyElement> {
        let renaming = self.renaming.as_ref().filter(|r| r.item == *item)?;
        Some(
            div()
                .flex_grow(1.)
                .min_w_0()
                // A press in it edits the name, rather than select, open or drag the row.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                // Drawn focused, as it ends once the keys go elsewhere in the window. Its text
                // where the name was, past its border and padding.
                .child(
                    field(&renaming.input, true, theme)
                        .ml(dp(-5.))
                        .px(dp(4.))
                        .py(dp(1.))
                        .text_color(theme.text_1)
                        // As a branch's name is.
                        .when(matches!(item, Selection::Branch(..)), |field| {
                            field.font_weight(gpui::FontWeight::SEMIBOLD)
                        }),
                )
                .into_any_element(),
        )
    }
}
