//! A pull request's title and description edited in place on its tab, where a click on the
//! title starts it, as a commit's message is edited in its header.

use gpui::{App, Context, Entity, FocusHandle, Focusable as _, Subscription, Window, prelude::*};

use super::Detail;
use crate::backend::{Review, reviews};
use crate::state::{Overlay, ProjectId, snack};
use crate::ui::message_editor::{MessageEditor, MessageSize, split};
use crate::ui::text_input::InputEvent;

/// The pull request whose title and description are being edited, in the editor, with what the
/// forge has as it last knew, where the keys go back to, and whether the editor holds just what
/// was saved, which closes it once the reload after shows it.
pub(super) struct ReviewEdit {
    project: ProjectId,
    number: u64,
    pub(super) input: Entity<MessageEditor>,
    opened: String,
    back: FocusHandle,
    saved: bool,
    _subscription: Subscription,
}

impl Detail {
    /// Edit the title and description of the pull request the tab shows. Looking at another
    /// keeps what's written, to pick up again on coming back, and refuses to start on another
    /// while it holds changes.
    pub(super) fn start_review_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(project), Some(review)) = (self.selected_project(cx), self.selected_review(cx))
        else {
            return;
        };
        let number = review.number;
        if let Some(edit) = self.review_edit.as_ref().filter(|edit| !edit.saved) {
            if edit.project == project && edit.number == number {
                return window.focus(&edit.input.focus_handle(cx), cx);
            }
            if !edit.input.read(cx).holds(&edit.opened, cx) {
                let why = format!("Save or cancel the edit of #{} first", edit.number);
                return snack(&self.store, why, cx);
            }
        }
        let message = format!("{}\n\n{}", review.title, review.body);
        let input = cx.new(|cx| {
            let mut input = MessageEditor::new(MessageSize::Large, ["Title", "Description"], cx);
            input.load(message, cx);
            input
        });
        // As the editor holds them, so only what's changed goes to the forge.
        let opened = input.read(cx).text(cx);
        let back = window.focused(cx).unwrap_or(self.focus.clone());
        let subscription =
            cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
                InputEvent::Submit => this.save_review_edit(window, cx),
                InputEvent::Cancel => this.leave_review_edit(window, cx),
                // Written on after a save, it stays open to save again.
                InputEvent::Changed => (this.review_edit.iter_mut()).for_each(|e| e.saved = false),
            });
        window.focus(&input.focus_handle(cx), cx);
        self.review_edit = Some(ReviewEdit {
            project,
            number,
            input,
            opened,
            back,
            saved: false,
            _subscription: subscription,
        });
        cx.notify();
    }

    /// Close the editor, giving the keys back to where they were when it opened.
    fn leave_review_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(edit) = self.review_edit.take() {
            window.focus(&edit.back, cx);
        }
        cx.notify();
    }

    /// Send what's been changed of the title and description to the forge. The editor stays,
    /// saying so, until the reload after shows it, or with what was written if the forge
    /// refuses it.
    fn save_review_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(edit) = &self.review_edit else {
            return;
        };
        let (project, number) = (edit.project.clone(), edit.number);
        if self.store.read(cx).editing_review(&project, number) {
            return;
        }
        let written = edit.input.read(cx).text(cx);
        let (title, body) = split(&written);
        if title.is_empty() {
            return snack(&self.store, "Give it a title first", cx);
        }
        let changed = |now: &str, was: &str| (now != was).then(|| now.to_owned());
        let (was_title, was_body) = split(&edit.opened);
        let (title, body) = (changed(title, was_title), changed(body, was_body));
        if title.is_none() && body.is_none() {
            return self.leave_review_edit(window, cx);
        }
        window.focus(&edit.back, cx);
        let (detail, saved) = (cx.entity().downgrade(), project.clone());
        (self.store).update(cx, |store, cx| {
            store.apply(
                &project,
                "Edit pull request",
                Overlay {
                    edits_review: Some(number),
                    ..Overlay::default()
                },
                reviews::EditReview {
                    number,
                    title,
                    body,
                },
                move |store, (), cx| {
                    store.snack("Pull request updated", cx);
                    // What's written since stays to save again, compared with what the forge
                    // has now.
                    (detail.update(cx, |detail, cx| {
                        if let Some(edit) = (detail.review_edit.as_mut())
                            .filter(|edit| edit.project == saved && edit.number == number)
                        {
                            edit.saved = edit.input.read(cx).text(cx) == written;
                            edit.opened = written;
                            cx.notify();
                        }
                    }))
                    .ok();
                },
                cx,
            )
        });
        cx.notify();
    }

    /// The edit of `review` and whether it's being saved, while the tab shows its editor: from
    /// the click on its title until the reload after saving shows what was saved.
    pub(super) fn shown_review_edit(
        &self,
        project: &ProjectId,
        review: &Review,
        cx: &App,
    ) -> Option<(&ReviewEdit, bool)> {
        let edit = (self.review_edit.as_ref())
            .filter(|edit| edit.project == *project && edit.number == review.number)?;
        let saving = self.store.read(cx).editing_review(project, review.number);
        (saving || !edit.saved).then_some((edit, saving))
    }
}
