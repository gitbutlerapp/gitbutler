use gpui::{AnyElement, Context, Div, Window, div, prelude::*};

use super::{Example, example, page};
use crate::navigator::authors_stack;
use crate::theme::Theme;
use crate::ui::person::author_avatar;
use crate::ui::{
    ButtonStyle, CommitPlace, Fold, Icon, MetaCountType, MetaStatusVariant, PullRequestState,
    SidebarRow, branch_item, button, commit_item, dp, icon_at, loading_item, machine_item,
    machine_status, meta_count, meta_status, more_item, repo_item, stack_caption, terminal_item,
    tree_guides, uncommitted_item, upstream_item, v_flex, worktree_item,
};

/// A sidebar card, as the Workspace tree stories' `Card`: a machine's header over its
/// repositories, 334 wide, in a subtle ring.
fn card(theme: &Theme, repositories: impl IntoIterator<Item = Div>) -> Div {
    let header = machine_item(
        "machine",
        "pavel--macbook-pro-2",
        icon_at(Icon::Laptop, 14.).text_color(theme.muted_foreground),
        Some(machine_status("online", true, theme).into_any_element()),
    )
    .fold(Fold::Open)
    .build(theme);
    v_flex()
        .w(dp(334.))
        .flex_none()
        .overflow_hidden()
        .border_1()
        .border_color(theme.border_subtle)
        .rounded(dp(theme.shape.card))
        .bg(theme.background)
        .child(
            div()
                .p(dp(8.))
                .border_b_1()
                .border_color(theme.border_subtle)
                .child(header),
        )
        .children(repositories)
}

/// One repository's rows, with their guides worked out from how deep each sits, as the stories'
/// `Repository` does, ruled off from the next unless `last`.
pub(super) fn repository(theme: &Theme, last: bool, rows: Vec<(usize, SidebarRow)>) -> Div {
    let depths: Vec<usize> = rows.iter().map(|(depth, _)| *depth).collect();
    let guides = tree_guides(&depths);
    v_flex()
        .gap(dp(1.))
        .p(dp(8.))
        .when(!last, |el| {
            el.border_b_1().border_color(theme.border_subtle)
        })
        .children(
            rows.into_iter()
                .zip(guides)
                .map(|((depth, row), guides)| row.depth(depth, guides).build(theme)),
        )
}

fn age(value: &'static str, theme: &Theme) -> Div {
    meta_count(MetaCountType::Age, Some(value.into()), theme)
}

/// A count's next step in its place, as the stories' `push` and `update`.
fn step(id: &'static str, n: usize, style: ButtonStyle, glyph: Icon) -> AnyElement {
    button(id, n.to_string())
        .small()
        .style(style)
        .icon(glyph)
        .into_any_element()
}

fn push(id: &'static str, n: usize) -> AnyElement {
    step(id, n, ButtonStyle::Info, Icon::ArrowUp)
}

fn update(id: &'static str, n: usize) -> AnyElement {
    step(id, n, ButtonStyle::Warning, Icon::ArrowDown)
}

/// The Workspace tree stories' `Tree`, `selected` naming the row picked, `solid` while its card
/// holds the keys.
fn tree(selected: &str, solid: bool, theme: &Theme) -> [Div; 2] {
    let is = |name: &str| selected == name;
    let but_dev = vec![
        (0, repo_item("repo", "but-dev", theme).fold(Fold::Open)),
        (
            1,
            worktree_item("main", "Main worktree", true, false, theme)
                .fold(Fold::Open)
                .meta(age("1d", theme))
                .action(update("update-main", 4)),
        ),
        (
            2,
            uncommitted_item("uncommitted", 19, None, theme).selected(is("uncommitted"), solid),
        ),
        (
            2,
            branch_item(
                "ios",
                "ios-push-handoff",
                None,
                false,
                is("ios-push-handoff"),
                theme,
            )
            .fold(Fold::Folded)
            .selected(is("ios-push-handoff"), solid)
            .action(push("push-ios", 7)),
        ),
        (
            2,
            stack_caption(
                "stack",
                2,
                "fix: retry stalled transfers",
                Some(false),
                theme,
            ),
        ),
        (
            3,
            branch_item(
                "retry",
                "fix: retry stalled transfers",
                Some((377, PullRequestState::Open)),
                false,
                false,
                theme,
            )
            .fold(Fold::Folded)
            .meta(meta_status(
                MetaStatusVariant::Working,
                Some("CI".into()),
                theme,
            ))
            .meta(age("8m", theme))
            .action(push("push-retry", 4)),
        ),
        (
            3,
            branch_item(
                "resume",
                "feat: resume work on another machine",
                Some((280, PullRequestState::Open)),
                false,
                false,
                theme,
            )
            .fold(Fold::Open)
            .meta(meta_status(
                MetaStatusVariant::Working,
                Some("CI".into()),
                theme,
            ))
            .action(push("push-resume", 7)),
        ),
        (
            4,
            commit_item(
                "c1",
                "chore: separator tokens",
                CommitPlace::Local,
                false,
                false,
                theme,
            ),
        ),
        (
            4,
            commit_item(
                "c2",
                "feat: 28px hunk separator",
                CommitPlace::Pushed,
                false,
                false,
                theme,
            )
            .selected(is("commit"), solid),
        ),
        (
            4,
            commit_item(
                "c3",
                "feat: elapsed time for agents",
                CommitPlace::Pushed,
                false,
                false,
                theme,
            ),
        ),
        (
            2,
            branch_item(
                "bump",
                "chore: bump dependencies",
                Some((292, PullRequestState::Merged)),
                false,
                false,
                theme,
            )
            .fold(Fold::Folded)
            .meta(meta_count(MetaCountType::Commits, Some("12".into()), theme))
            .meta(age("8m", theme)),
        ),
        (2, more_item("more", "3 more", false, theme)),
        (
            1,
            worktree_item("archive", "mesh-archive", false, false, theme)
                .fold(Fold::Open)
                .meta(age("5h", theme))
                .action(update("update-archive", 2)),
        ),
        (2, uncommitted_item("uncommitted-2", 2, None, theme)),
        (
            2,
            branch_item("laptop", "resume-on-laptop", None, false, false, theme)
                .fold(Fold::Folded)
                .action(push("push-laptop", 9)),
        ),
        (
            2,
            branch_item("offline", "offline-cache", None, false, false, theme)
                .fold(Fold::Empty)
                .meta(meta_count(
                    MetaCountType::Empty,
                    Some("empty".into()),
                    theme,
                )),
        ),
    ];
    let berlin = vec![
        (
            0,
            repo_item("repo-2", "berlin-brutalism", theme)
                .fold(Fold::Open)
                .meta(age("2h", theme))
                .action(update("update-berlin", 8)),
        ),
        (
            1,
            branch_item("idle", "daemon-idle-cpu", None, false, false, theme)
                .fold(Fold::Folded)
                .meta(meta_count(MetaCountType::Commits, Some("27".into()), theme))
                .meta(age("5h", theme))
                .action(button("create-pr", "Create PR").small().into_any_element()),
        ),
    ];
    [
        repository(theme, false, but_dev),
        repository(theme, true, berlin),
    ]
}

const INACTIVE: [(&str, &str); 2] = [
    ("dazzling-sinoussi-031b8a", "6d"),
    ("quirky-hopper-7f2e10", "9d"),
];

/// The stories' `InactiveWorktreesCard`: a repository whose quiet linked worktrees fold away
/// behind a MoreItem after the active ones, shown while `open`.
fn inactive_worktrees(open: bool, theme: &Theme) -> Div {
    let mut rows = vec![
        (0, repo_item("repo", "but-dev", theme).fold(Fold::Open)),
        (
            1,
            worktree_item("main", "Main worktree", true, false, theme)
                .fold(Fold::Folded)
                .meta(age("1d", theme)),
        ),
        (
            1,
            worktree_item("archive", "mesh-archive", false, false, theme)
                .fold(Fold::Folded)
                .meta(age("5h", theme)),
        ),
    ];
    if open {
        rows.extend(INACTIVE.into_iter().map(|(name, since)| {
            (
                1,
                worktree_item(name, name, false, false, theme)
                    .fold(Fold::Folded)
                    .meta(age(since, theme)),
            )
        }));
    }
    let label = match open {
        true => "Hide inactive worktrees".to_owned(),
        false => format!("{} inactive worktrees", INACTIVE.len()),
    };
    rows.push((1, more_item("inactive", label, open, theme)));
    card(theme, [repository(theme, true, rows)])
}

/// The Unread story's tree as it opens, every branch folded: a branch with new commits inside
/// carries their dot.
fn unread(theme: &Theme) -> Div {
    let rows = vec![
        (0, repo_item("repo", "but-dev", theme).fold(Fold::Open)),
        (1, uncommitted_item("uncommitted", 10, None, theme)),
        (
            1,
            branch_item("review-cards", "review-cards", None, false, false, theme)
                .fold(Fold::Folded)
                .unread(true)
                .action(push("push-review", 3)),
        ),
        (
            1,
            branch_item(
                "454",
                "fix: unique cursor names",
                Some((454, PullRequestState::Open)),
                false,
                false,
                theme,
            )
            .fold(Fold::Folded)
            .action(push("push-454", 2)),
        ),
        (
            1,
            branch_item("cursor-api", "cursor-api", None, false, false, theme)
                .fold(Fold::Folded)
                .unread(true)
                .meta(meta_count(MetaCountType::Commits, Some("3".into()), theme)),
        ),
        (
            1,
            branch_item(
                "132",
                "feat: archive old worktrees",
                Some((132, PullRequestState::Merged)),
                false,
                false,
                theme,
            )
            .fold(Fold::Folded)
            .meta(meta_count(MetaCountType::Commits, Some("2".into()), theme)),
        ),
    ];
    card(theme, [repository(theme, true, rows)])
}

/// The Upstream story's tree with the heading open or folded, and the branch open or folded, as
/// 🍑 But UI's Sidebar · upstream draws the three. Folded, a row shows who the commits it hides
/// are from, then Update.
fn upstream(heading_open: bool, branch_open: bool, theme: &Theme) -> Div {
    // Each card's rows their own ids: a chevron turns by its row's id, so one card's fold would
    // turn another's.
    let card_ix = usize::from(heading_open) + 2 * usize::from(branch_open);
    let pavel = ("Pavel Laptev", "pavel@example.com");
    let kiril = ("Kiril Videlov", "kiril@example.com");
    let face = |(name, email): (&str, &str)| author_avatar(name, email, 14., theme).mx(dp(3.));
    let fold = |open: bool| match open {
        true => Fold::Open,
        false => Fold::Folded,
    };
    let faces = |row: SidebarRow| {
        authors_stack([pavel, kiril].into_iter(), theme)
            .into_iter()
            .fold(row, SidebarRow::meta)
    };
    let branch = branch_item(
        ("perf", card_ix),
        "perf: cheaper status polling",
        None,
        false,
        false,
        theme,
    )
    .fold(fold(branch_open))
    .meta(meta_status(
        MetaStatusVariant::Working,
        Some("CI".into()),
        theme,
    ));
    let mut rows = vec![
        (
            0,
            repo_item(("repo", card_ix), "but-dev", theme).fold(Fold::Open),
        ),
        (
            1,
            match branch_open {
                true => branch,
                false => faces(branch).action(update("update-perf", 3)),
            },
        ),
    ];
    if branch_open {
        let heading =
            upstream_item(("upstream", card_ix), "origin", None, theme).fold(fold(heading_open));
        let heading = match heading_open {
            true => heading,
            false => faces(heading),
        };
        rows.push((2, heading.action(update("update-upstream", 3))));
        if heading_open {
            let upstream = |id: &'static str, message: &'static str, merge: bool| {
                commit_item(
                    (id, card_ix),
                    message,
                    CommitPlace::Upstream,
                    merge,
                    false,
                    theme,
                )
            };
            rows.extend([
                (
                    3,
                    upstream("merge", "Merge #12: retry on timeout", true).fold(Fold::Open),
                ),
                (
                    4,
                    upstream("fix", "fix: retry on timeout", false).meta(face(kiril)),
                ),
                (
                    3,
                    upstream("test", "test: cover a stalled transfer", false).meta(face(pavel)),
                ),
            ]);
        }
        let own = |id: &'static str, message: &'static str, place: CommitPlace| {
            commit_item((id, card_ix), message, place, false, false, theme)
        };
        rows.extend([
            (2, own("docs", "docs: handoff notes", CommitPlace::Pushed)),
            (
                2,
                own("hunk", "feat: 28px hunk separator", CommitPlace::Local),
            ),
            (
                2,
                own("tokens", "chore: separator tokens", CommitPlace::Local),
            ),
        ]);
    }
    card(theme, [repository(theme, true, rows)])
}

/// The stories of `SidebarRow.stories.tsx` this client can draw so far.
pub(super) const EXAMPLES: [Example; 5] = [
    Example {
        id: "components-sidebarrow--selection",
        name: "Selection",
        draw: |theme| {
            gpui::div()
                .flex()
                .flex_wrap()
                .items_start()
                .gap(dp(24.))
                .child(card(theme, tree("commit", true, theme)))
                .child(card(theme, tree("ios-push-handoff", false, theme)))
                .into_any_element()
        },
    },
    Example {
        id: "components-sidebarrow--inactive-worktrees",
        name: "Inactive Worktrees",
        draw: |theme| {
            gpui::div()
                .flex()
                .flex_wrap()
                .items_start()
                .gap(dp(24.))
                .child(inactive_worktrees(false, theme))
                .child(inactive_worktrees(true, theme))
                .into_any_element()
        },
    },
    Example {
        id: "components-sidebarrow--unread",
        name: "Unread",
        draw: |theme| unread(theme).into_any_element(),
    },
    Example {
        id: "components-sidebarrow--loading",
        name: "Loading",
        draw: |theme| loading(theme).into_any_element(),
    },
    Example {
        id: "components-sidebarrow--upstream",
        name: "Upstream",
        draw: |theme| {
            gpui::div()
                .flex()
                .flex_wrap()
                .items_start()
                .gap(dp(24.))
                .child(upstream(true, true, theme))
                .child(upstream(false, true, theme))
                .child(upstream(false, false, theme))
                .into_any_element()
        },
    },
];

/// A repository's card while its rows load, as the `Loading` story: the header as it will be, over
/// rows of bones, each a glyph and a name.
fn loading(theme: &Theme) -> Div {
    v_flex()
        .w(dp(334.))
        .flex_none()
        .overflow_hidden()
        .border_1()
        .border_color(theme.border_subtle)
        .rounded(dp(theme.shape.card))
        .bg(theme.background)
        .child(
            div()
                .p(dp(8.))
                .border_b_1()
                .border_color(theme.border_subtle)
                .child(
                    repo_item("repo", "but-dev", theme)
                        .fold(Fold::Open)
                        .build(theme),
                ),
        )
        .child(
            v_flex().gap(dp(1.)).p(dp(8.)).children(
                [156., 76., 199., 116.]
                    .into_iter()
                    .enumerate()
                    .map(|(ix, width)| loading_item(("loading", ix), width).build(theme)),
            ),
        )
}

/// What a terminal's program is at, as `TerminalItem.stories.tsx`'s `Statuses` gives each.
fn statuses(theme: &Theme) -> Div {
    let status = |variant| meta_status(variant, None, theme);
    let rows = [
        (
            "claude",
            "gpui-row-buttons",
            Some(meta_count(MetaCountType::AgentWaiting, None, theme)),
        ),
        (
            "pi",
            "review-cards",
            Some(meta_count(MetaCountType::AgentWorking, None, theme)),
        ),
        (
            "cargo build",
            "Main worktree",
            Some(meta_count(MetaCountType::Running, None, theme)),
        ),
        (
            "cargo test",
            "Main worktree",
            Some(status(MetaStatusVariant::Passed)),
        ),
        (
            "codex",
            "Main worktree",
            Some(status(MetaStatusVariant::Failed)),
        ),
        ("Terminal", "Main worktree", None),
    ];
    column(rows.into_iter().map(|(name, place, meta)| {
        let row = terminal_item(name, name, Some(place.into()), theme);
        match meta {
            Some(meta) => row.meta(meta),
            None => row,
        }
        .build(theme)
    }))
}

/// The terminal stories' `Column`, 316 wide as their page is.
fn column(rows: impl IntoIterator<Item = impl IntoElement>) -> Div {
    v_flex().w(dp(316.)).gap(dp(1.)).children(rows)
}

/// The stories of `TerminalItem.stories.tsx` this client can draw so far.
pub(super) const TERMINAL_EXAMPLES: [Example; 2] = [
    Example {
        id: "components-sidebarrow-terminal--default",
        name: "Default",
        draw: |theme| {
            column([terminal_item(
                "terminal",
                "review-cards",
                Some("gpui-row-buttons".into()),
                theme,
            )
            .build(theme)])
            .into_any_element()
        },
    },
    Example {
        id: "components-sidebarrow-terminal--statuses",
        name: "Statuses",
        draw: |theme| statuses(theme).into_any_element(),
    },
];

pub struct SidebarRowsStory;

impl Render for SidebarRowsStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        page(
            (EXAMPLES.into_iter())
                .chain(TERMINAL_EXAMPLES)
                .map(|e| example(e, theme)),
        )
    }
}
