//! A linked worktree's page: where it is, the branch it has checked out, and its
//! uncommitted changes, like an agent working in it leaves them.

use gpui::{AnyElement, ClickEvent, Context, div, prelude::*, px};

use super::Detail;
use super::panes::{side_header, unfinished_banner};
use crate::state::{ProjectId, Selection};
use crate::theme::Theme;
use crate::ui::{Icon, Tooltip, empty_state, faint, h_flex, icon, plural, short_path, v_flex};

impl Detail {
    pub(super) fn render_worktree_side(
        &self,
        project: &ProjectId,
        name: &str,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(worktree) = (store.workspace(project))
            .and_then(|ws| ws.worktrees.iter().find(|w| &*w.name == name))
        else {
            return empty_state("Worktree not found", "", theme);
        };
        let files = self.render_diff_file_list(theme, cx);
        // Its branch links to its commits: in the navigator when they're listed there, as its
        // own lane's are, else on the Branches page.
        let branch = worktree.branch.clone().map(|branch| {
            let ref_name = format!("refs/heads/{branch}");
            let applied =
                (store.workspace(project)).is_some_and(|ws| ws.find_branch(&ref_name).is_some());
            let project = project.clone();
            div()
                .id("worktree-branch")
                .font_family(theme.mono_font.clone())
                .text_color(theme.blue)
                .cursor_pointer()
                .hover(|el| el.underline())
                .tooltip(Tooltip::text("See its commits"))
                .child(branch)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if applied {
                        let shown = Selection::Branch(project.clone(), ref_name.clone());
                        this.store
                            .update(cx, |store, cx| store.select(Some(shown), cx));
                    } else {
                        this.show_other_branch(&project, ref_name.clone(), cx)
                    }
                }))
        });
        let files_count = plural(worktree.uncommitted.len(), "file");
        v_flex()
            .size_full()
            .child(
                side_header(theme)
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .child(icon(Icon::Folder).text_color(theme.text_2))
                            .child(
                                div()
                                    .font_family(theme.mono_font.clone())
                                    .text_size(px(14.))
                                    .truncate()
                                    .child(name.to_owned()),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap(px(4.))
                            .text_size(px(12.))
                            .text_color(theme.text_2)
                            .child(if branch.is_some() {
                                "On"
                            } else {
                                "No branch checked out"
                            })
                            .children(branch)
                            .child(format!("· {files_count} uncommitted")),
                    )
                    // Cut in the middle, keeping the folder's own name at its end.
                    .child(
                        faint(short_path(&worktree.path), theme)
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis_middle(),
                    ),
            )
            .children(
                worktree
                    .git_operation
                    .map(|op| unfinished_banner(op, theme)),
            )
            .children(
                (self.conflict_banner(cx.entity().downgrade(), cx))
                    .map(|banner| side_header(theme).child(banner)),
            )
            .child(files)
            .into_any_element()
    }
}
