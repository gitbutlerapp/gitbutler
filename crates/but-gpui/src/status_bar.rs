//! The strip along the bottom of the window: what's running, what's cut, and the keys
//! that matter right now.

use gpui::{Action, AnyElement, Context, Focusable as _, Window, div, prelude::*, px};

use crate::app::{AppView, CommandPalette, CutSelection, FocusDiff, PasteHere, StartCommit, Undo};
use crate::detail::{AmendTop, DiscardHunk, NextFile, NextHunk};
use crate::state::{SLOW, Selection};
use crate::theme::Theme;
use crate::ui::{Icon, STATUS_BAR_HEIGHT, h_flex, keycap, shortcut, small_icon};

impl AppView {
    pub fn render_status_bar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let store = self.store.read(cx);
        let detail = self.detail.read(cx);
        let working = store.busy().map(|(busy, took)| {
            // How long it's taking once that's noticeable, and what else runs or waits.
            let elapsed = match took >= SLOW {
                true => format!(" {:.1}s", took.as_secs_f32()),
                false => String::new(),
            };
            let others = match store.others_busy() {
                0 => String::new(),
                n => format!(" · {n} more"),
            };
            format!("{busy}…{elapsed}{others}")
        });
        let state: AnyElement = if let Some(working) = working {
            h_flex()
                .gap(px(6.))
                .text_color(theme.pop)
                .child(small_icon(Icon::Clock).text_color(theme.pop))
                .child(working)
                .into_any_element()
        } else if let Some(cut) = &store.cut {
            h_flex()
                .gap(px(6.))
                .min_w(px(0.))
                .child(div().text_color(theme.pop).child(cut.verb()))
                .child(div().truncate().child(cut.describe()))
                .children(shortcut(&PasteHere, cx).map(|keys| keycap(&keys, &theme)))
                .child("paste here")
                .child(keycap("esc", &theme))
                .into_any_element()
        } else if !store.picked.is_empty() {
            // Only the navigator's highlight shows them otherwise.
            h_flex()
                .gap(px(6.))
                .child(div().text_color(theme.pop).child(format!(
                    "{} selected",
                    crate::ui::plural(store.picked.len(), "commit")
                )))
                .child("drag them together, or right-click to squash or uncommit them")
                .child(keycap("esc", &theme))
                .into_any_element()
        } else if detail.picked_line_count() > 0 {
            h_flex()
                .gap(px(6.))
                .child(div().text_color(theme.pop).child(format!(
                    "{} selected",
                    crate::ui::plural(detail.picked_line_count(), "line")
                )))
                .child("drag a grip or right-click to move them")
                .child(keycap("esc", &theme))
                .into_any_element()
        } else if detail.picked_files() > 0 {
            h_flex()
                .gap(px(6.))
                .child(div().text_color(theme.pop).child(format!(
                    "{} selected",
                    crate::ui::plural(detail.picked_files(), "file")
                )))
                .child(keycap("esc", &theme))
                .into_any_element()
        } else {
            // The selected project's standing against its target.
            let summary = store.selection.as_ref().and_then(|selection| {
                let entry = store.project(selection.project())?;
                let target = entry.workspace.as_ref()?.target.as_ref()?;
                // A stack can be on an older target commit, as one just applied can.
                Some(match target.behind {
                    0 if target.needs_update => {
                        format!(
                            "{} · some branches are behind {}",
                            entry.project.name, target.name
                        )
                    }
                    0 => format!("{} · up to date with {}", entry.project.name, target.name),
                    n => format!(
                        "{} · {} behind {}",
                        entry.project.name,
                        crate::ui::plural(n, "commit"),
                        target.name
                    ),
                })
            });
            div().children(summary).into_any_element()
        };

        let detail_focused = detail.is_focused(window, cx);
        // A text field has focus, where keys type rather than act.
        let typing = window.focused(cx).is_some_and(|focused| {
            !detail.owns_focus(&focused) && focused != self.focus_handle(cx)
        });
        // A linked worktree's changes commit and discard as the main checkout's do.
        let uncommitted = matches!(
            store.selection,
            Some(Selection::Uncommitted(_) | Selection::Worktree(..))
        );
        let hints: Vec<(&dyn Action, &str)> = if uncommitted && detail.composing() {
            let amend = detail
                .can_amend(cx)
                .then_some((&AmendTop as &dyn Action, "amend"));
            [(&StartCommit as &dyn Action, "commit")]
                .into_iter()
                .chain(amend)
                .collect()
        } else if !typing && !detail_focused && detail.shows_conflicts() {
            // Picking takes the keys of the diff, so it needs the focus.
            vec![(&FocusDiff, "pick conflicts")]
        } else if typing
            || (detail_focused && (!detail.has_files() || detail.showing_review(cx)))
            || store.selection.is_none()
        {
            Vec::new()
        } else if detail.files_focused(window) {
            vec![(&NextFile, "next file")]
        } else if detail_focused && uncommitted {
            // ⌘X takes what's picked, or the hunk or file under the cursor, and on a file's
            // header there's no hunk for ⌥⌘Z.
            let discard =
                (detail.on_hunk()).then_some((&DiscardHunk as &dyn Action, "discard hunk"));
            [
                (&NextHunk as &dyn Action, "next hunk"),
                (&CutSelection, "cut"),
            ]
            .into_iter()
            .chain(discard)
            .collect()
        } else if detail_focused && detail.cursor_cut(cx).is_some() {
            vec![(&NextHunk, "next hunk"), (&CutSelection, "cut")]
        } else if detail_focused {
            vec![(&NextFile, "next file"), (&NextHunk, "next hunk")]
        } else {
            vec![
                (&CutSelection, "cut"),
                (&PasteHere, "paste"),
                (&Undo, "undo"),
            ]
        };

        h_flex()
            .flex_none()
            .h(px(STATUS_BAR_HEIGHT))
            .px(px(12.))
            .gap(px(10.))
            .border_t_1()
            .border_color(theme.border_3)
            .bg(theme.bg_2)
            .text_size(px(12.))
            .text_color(theme.text_2)
            .child(div().flex_1().min_w(px(0.)).truncate().child(state))
            .children(hints.into_iter().filter_map(|(action, label)| {
                let keys = shortcut(action, cx)?;
                Some(
                    h_flex()
                        .flex_none()
                        .gap(px(4.))
                        .child(keycap(&keys, &theme))
                        .child(label),
                )
            }))
            .child(
                h_flex()
                    .id("open-commands")
                    .flex_none()
                    .gap(px(4.))
                    .px(px(6.))
                    .h(px(22.))
                    .rounded(px(4.))
                    .hover(|el| el.bg(theme.hover()))
                    .child(keycap(
                        shortcut(&CommandPalette, cx).as_deref().unwrap_or("⌘K"),
                        &theme,
                    ))
                    .child("Commands")
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(CommandPalette), cx)),
            )
    }
}
