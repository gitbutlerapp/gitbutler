//! Inactive worktrees' page: what's left in the linked worktrees nothing happened in lately,
//! so what's done can go.

use gpui::{AnyElement, Context, prelude::*};

use super::span_said;
use crate::backend::{self, Branch, CommitState, PushState};
use crate::detail::Detail;
use crate::detail::panes::{
    Lane, grouped, overview_split, overview_tiles, overview_title, worktree_lanes,
};
use crate::state::{ProjectId, Selection};
use crate::theme::Theme;
use crate::ui::{Icon, dp, v_flex};

/// What's left in a quiet worktree, which says whether it can go.
enum Left {
    /// Commits no remote has, and how many.
    Unpushed(usize),
    /// Its branch landed.
    Landed,
    /// Commits, every one pushed.
    Pushed,
    /// No commits of its own.
    Nothing,
}

impl Left {
    fn of(branches: &[Branch]) -> Left {
        let commits = || branches.iter().flat_map(|b| &b.commits);
        let unpushed = commits().filter(|c| c.state == CommitState::Local).count();
        if unpushed > 0 {
            Left::Unpushed(unpushed)
        } else if (branches.iter()).any(|b| b.push == PushState::Integrated) {
            Left::Landed
        } else if commits().next().is_some() {
            Left::Pushed
        } else {
            Left::Nothing
        }
    }
}

impl Detail {
    /// Inactive worktrees' page: how many there are, their commits, those no remote has and
    /// how long the quietest has been quiet; how many hold commits only here, landed, have
    /// every commit pushed or have none; then a lane for each, which opens it.
    pub(in crate::detail) fn render_inactive_overview(
        &self,
        project: &ProjectId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let mut worktrees = store.inactive_worktrees(project);
        worktrees.sort_by_key(|wt| std::cmp::Reverse(wt.updated_at));
        let ws = store.workspace(project);
        let branches: Vec<&[Branch]> = (worktrees.iter())
            .map(|wt| ws.and_then(|ws| ws.worktree_lane(wt.name.as_ref())))
            .map(|lane| lane.map_or(&[][..], |lane| &lane.branches))
            .collect();
        let now = crate::ui::now();
        let lefts: Vec<Left> = branches.iter().map(|branches| Left::of(branches)).collect();
        let commits: usize = (branches.iter().copied().flatten())
            .map(|b| b.commits.len())
            .sum();
        let unpushed: usize = (lefts.iter())
            .map(|left| match left {
                Left::Unpushed(n) => *n,
                _ => 0,
            })
            .sum();
        let quietest = worktrees.iter().filter_map(|wt| wt.updated_at).min();
        let tiles = [
            (
                "Worktrees",
                grouped(worktrees.len()),
                "Linked worktrees with nothing going on lately",
            ),
            (
                "Commits",
                grouped(commits),
                "Commits made in them that the target doesn't have",
            ),
            (
                "Unpushed",
                grouped(unpushed),
                "Of those, the ones no remote has",
            ),
            (
                "Quietest",
                quietest.map_or_else(|| "–".to_owned(), |at| span_said(now - at)),
                "How long the one quiet longest has had nothing done in it",
            ),
        ];
        let count = |keep: fn(&Left) -> bool| lefts.iter().filter(|l| keep(l)).count();
        let parts = [
            (
                Icon::Commit,
                theme.blue,
                theme.blue,
                "with unpushed commits",
                count(|l| matches!(l, Left::Unpushed(_))),
            ),
            (
                Icon::BranchMerge,
                theme.purple,
                theme.purple,
                "landed",
                count(|l| matches!(l, Left::Landed)),
            ),
            (
                Icon::CommitFill,
                theme.text_2,
                theme.text_2,
                "pushed",
                count(|l| matches!(l, Left::Pushed)),
            ),
            (
                Icon::FolderCopy,
                theme.text_3,
                theme.border_1,
                "with no commits",
                count(|l| matches!(l, Left::Nothing)),
            ),
        ]
        .map(|(glyph, color, fill, said, n)| (glyph, color, fill, said.to_owned(), n));
        let shown = worktrees.iter().zip(&lefts).zip(&branches);
        let lanes = shown.map(|((wt, left), branches)| Lane {
            at: Selection::Worktree(project.clone(), wt.name.clone()),
            glyph: Icon::FolderCopy,
            name: wt.name.to_string(),
            tag: Some(match left {
                Left::Unpushed(n) => (format!("{n} unpushed"), theme.blue),
                Left::Landed => ("landed".to_owned(), theme.purple),
                Left::Pushed => ("pushed".to_owned(), theme.text_3),
                Left::Nothing => ("no commits".to_owned(), theme.text_3),
            }),
            branches: (wt.branch.as_ref())
                .map_or("detached".into(), backend::short_ref)
                .into_owned(),
            commits: (branches.iter().flat_map(|b| &b.commits))
                .map(|c| c.time)
                .collect(),
            changed: wt.updated_at,
        });
        v_flex()
            .id("inactive-overview")
            .flex_1()
            .h_full()
            .min_w_0()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .w_full()
                    .px(dp(14.))
                    .py(dp(16.))
                    .gap(dp(6.))
                    .child(overview_title(
                        "Inactive worktrees",
                        "Nothing uncommitted, and no commits or checkouts in 12 hours".to_owned(),
                        theme,
                    ))
                    .child(overview_tiles(tiles, theme))
                    .children(overview_split(parts, theme))
                    .child(worktree_lanes(lanes, worktrees.len(), theme, cx)),
            )
            .into_any_element()
    }
}
