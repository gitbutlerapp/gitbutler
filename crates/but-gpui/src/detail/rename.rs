//! A branch's name edited in its page's header, where a click on the name starts it.

use gpui::{
    AnyElement, App, Context, Entity, Focusable as _, FontWeight, Subscription, Window, prelude::*,
};

use super::Detail;
use crate::backend::{FullName, short_ref};
use crate::overlays::{branch_name_input, branch_went, on_branch_name};
use crate::state::{ProjectId, Selection, snack};
use crate::theme::Theme;
use crate::ui::text_input::TextInput;
use crate::ui::{dp, field};

/// The branch whose name is being edited in its header, and the field it's edited in.
pub(super) struct Renaming {
    project: ProjectId,
    ref_name: Option<FullName>,
    input: Entity<TextInput>,
    _subscriptions: [Subscription; 3],
}

impl Detail {
    /// Edit the name of branch `ref_name` in its header. Enter runs `submit` with the name
    /// typed, and Escape, or a click anywhere else, leaves it as it was.
    pub(crate) fn rename_branch(
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
        let back = window.focused(cx).unwrap_or(self.focus.clone());
        let typed = on_branch_name(
            &input,
            self.store.clone(),
            project.clone(),
            current,
            submit,
            move |this: &mut Self, window, cx| {
                this.renaming = None;
                window.focus(&back, cx);
                cx.notify();
            },
            window,
            cx,
        );
        // GPUI blurs it as another app comes in front too, which leaves it for coming back.
        let left = cx.on_blur(&input.focus_handle(cx), window, |this, window, cx| {
            if window.is_window_active() {
                this.renaming = None;
                cx.notify()
            }
        });
        let moved = cx.observe(&self.store, |this, _, cx| this.end_rename_if_moved(cx));
        window.focus(&input.focus_handle(cx), cx);
        self.renaming = Some(Renaming {
            project,
            ref_name,
            input,
            _subscriptions: [typed, left, moved],
        });
        cx.notify();
    }

    /// Stop editing a name once its branch isn't the one shown, or is gone, saying so.
    fn end_rename_if_moved(&mut self, cx: &mut Context<Self>) {
        let Some(Renaming {
            project, ref_name, ..
        }) = &self.renaming
        else {
            return;
        };
        let store = self.store.read(cx);
        let why = branch_went(store, project, ref_name.as_ref());
        let shown = Selection::Branch(project.clone(), ref_name.clone());
        if store.selection.as_ref() == Some(&shown) && why.is_none() {
            return;
        }
        self.renaming = None;
        if let Some(why) = why {
            snack(&self.store, why, cx);
        }
        cx.notify();
    }

    /// The field the name of `project`'s branch `ref_name` is being edited in, in place of its
    /// title, its text where the title's was and as tall.
    pub(super) fn rename_field(
        &self,
        project: &ProjectId,
        ref_name: Option<&FullName>,
        theme: &Theme,
    ) -> Option<AnyElement> {
        let renaming = (self.renaming.as_ref())
            .filter(|r| r.project == *project && r.ref_name.as_ref() == ref_name)?;
        Some(
            field(&renaming.input, true, theme)
                .flex_grow(1.)
                .min_w_0()
                .ml(dp(-5.))
                .my(dp(-1.))
                .px(dp(4.))
                .py_0()
                .text_size(dp(16.))
                .line_height(dp(22.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_1)
                .into_any_element(),
        )
    }
}
