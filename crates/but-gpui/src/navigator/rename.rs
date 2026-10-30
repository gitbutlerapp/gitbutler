//! A branch's or a terminal's name, or a commit's title, edited in its row, as Lite does, rather
//! than in a prompt.

use gpui::{
    AnyElement, App, Context, Entity, Focusable as _, MouseButton, Subscription, Window, div,
    prelude::*,
};

use super::Navigator;
use crate::backend::{CommitId, FullName, short_ref};
use crate::overlays::{branch_name_input, branch_went, on_branch_name};
use crate::state::{ProjectId, Selection, snack};
use crate::terminal::TerminalId;
use crate::theme::Theme;
use crate::ui::message_editor::{ToBody, joined, split};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{dp, field};

/// The row whose name or title is being edited, a branch's, a terminal's or a commit's, and the
/// field it's edited in.
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
        let input = branch_name_input(&current, cx);
        let typed = on_branch_name(
            &input,
            self.store.clone(),
            project.clone(),
            current,
            submit,
            |this: &mut Self, window, cx| {
                this.renaming = None;
                this.focus_list(window, cx)
            },
            window,
            cx,
        );
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

    /// Edit the title of commit `id` in its row, keeping its body. Enter saves what's typed,
    /// unless it's the title it has, and Tab goes on to the body in the commit's header. Escape,
    /// or a click anywhere else, leaves it as it was. A commit an agent rewrites meanwhile is
    /// followed, as the selection is.
    pub(crate) fn reword(
        &mut self,
        project: ProjectId,
        id: CommitId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        let Some((_, commit)) = store.find_commit(&project, id) else {
            return;
        };
        // Its first line, as saving and the header split it, rather than the row's title, which
        // joins a subject wrapped over several lines.
        let current = split(store.message_of(&project, commit).1).0.to_owned();
        let input = cx.new(|cx| {
            let mut input = TextInput::new("Commit title", false, cx);
            input.load(current, cx);
            input.select_all_text(cx);
            input
        });
        let typed = cx.subscribe_in(&input, window, |this, input, event, window, cx| {
            let title = match event {
                InputEvent::Submit => Some(input.read(cx).text().to_owned()),
                InputEvent::Cancel => None,
                InputEvent::Changed => return,
            };
            // The commit as it is now, should an agent have rewritten it meanwhile.
            let item = this.renaming.take().map(|r| r.item);
            this.focus_list(window, cx);
            let (Some(title), Some(Selection::Commit(project, id))) = (title, item) else {
                return;
            };
            let store = this.store.read(cx);
            let Some((_, commit)) = store.find_commit(&project, id) else {
                return;
            };
            let (was, body) = split(store.message_of(&project, commit).1);
            // Unchanged, rewording would only give it and the commits above it new ids.
            if title.trim() == was {
                return;
            }
            let message = joined(&title, body).trim().to_owned();
            (this.store).update(cx, |store, cx| {
                store.reword(&project, id, message, |_, _| {}, cx)
            })
        });
        self.edit_in_row(Selection::Commit(project, id), input, typed, window, cx)
    }

    /// Go on to the body of the commit whose title is being edited in its row, in its header,
    /// with the title typed. Another commit's new message unsaved there keeps it here.
    fn reword_in_full(&mut self, _: &ToBody, window: &mut Window, cx: &mut Context<Self>) {
        let Some(input) = self.renaming.as_ref().map(|r| r.input.clone()) else {
            return;
        };
        let title = input.read(cx).text().to_owned();
        // Where the header gives the keys back once it's done, as when it's started from here.
        window.focus(&self.focus, cx);
        let opened =
            (self.detail).update(cx, |detail, cx| detail.reword_in_full(&title, window, cx));
        if opened {
            self.renaming = None;
            cx.notify()
        } else {
            window.focus(&input.focus_handle(cx), cx)
        }
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
    /// window coming back from another app with a click elsewhere does, or once a branch or a
    /// commit is gone, as when an agent renamed the one or squashed the other, saying so. A
    /// commit an agent rewrote is followed, as the selection follows it. A terminal that closes
    /// takes the selection with it.
    fn end_rename_if_moved(&mut self, cx: &mut Context<Self>) {
        let Some(renaming) = self.renaming.as_mut() else {
            return;
        };
        let store = self.store.read(cx);
        if let (Selection::Commit(project, from), Some(Selection::Commit(at, to))) =
            (&renaming.item, &store.selection)
            && project == at
            && store.followed(*from, *to)
        {
            renaming.item = Selection::Commit(project.clone(), *to);
            return;
        }
        let item = &renaming.item;
        let why = match item {
            Selection::Branch(project, ref_name) => branch_went(store, project, ref_name.as_ref()),
            Selection::Commit(project, id) => (store.find_commit(project, *id).is_none())
                .then(|| "The commit left the workspace meanwhile".to_owned()),
            Selection::Terminal(..) => None,
            // Not renamed in their row.
            Selection::Project(_)
            | Selection::Checkout(..)
            | Selection::Uncommitted(..)
            | Selection::Branches(_)
            | Selection::History(_)
            | Selection::Graph(_)
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

    /// The field the name of `item`'s row is being edited in, in place of the name, or a
    /// commit's title.
    pub(super) fn rename_field(
        &self,
        item: &Selection,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let renaming = self.renaming.as_ref().filter(|r| r.item == *item)?;
        Some(
            div()
                .flex_grow(1.)
                .min_w_0()
                // A press in it edits the name, rather than select, open or drag the row.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .when(matches!(item, Selection::Commit(..)), |el| {
                    el.key_context("Reword")
                        .on_action(cx.listener(Navigator::reword_in_full))
                })
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
