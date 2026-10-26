//! The strip along the bottom of the window: what's running, what's cut, and the keys
//! that matter right now.

use gpui::{Action, AnyElement, Context, Focusable as _, Window, div, prelude::*};

use crate::app::{
    AppView, CommandPalette, CutSelection, Fetch, FocusDiff, OpenPullRequest, PasteHere, Push,
    RenameSelection, StartCommit, UncommitSelection, Undo, UpdateFromTarget,
};
use crate::backend::CommitId;
use crate::detail::{AmendTop, DiscardHunk, NextFile, NextHunk};
use crate::state::{Cut, SLOW, Section, Selection};
use crate::theme::Theme;
use crate::ui::{Icon, STATUS_BAR_HEIGHT, dp, duration, h_flex, shortcut, small_icon};

impl AppView {
    pub fn render_status_bar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let store = self.store.read(cx);
        let detail = self.detail.read(cx);
        // A conflicted commit is resolved, not uncommitted, as its files are one side's.
        let conflicted = |id: &CommitId| {
            (store.selection.as_ref())
                .and_then(|s| store.find_commit(s.project(), *id))
                .is_some_and(|(_, commit)| commit.conflicted)
        };
        let picks_conflicted = store.picked.iter().any(conflicted);
        // A text field has focus, where keys type rather than act.
        let typing = window.focused(cx).is_some_and(|focused| {
            !detail.owns_focus(&focused) && focused != self.focus_handle(cx)
        });
        let working = store.busy().map(|(project, busy, took)| {
            // Another project's says which, as its failure card would.
            let doing = store.card_title(project, under_way(busy));
            // How long it's taking once that's noticeable, and what else runs or waits.
            let elapsed = match took >= SLOW {
                true => format!(" {}", duration(took.as_secs_f32().round() as i64)),
                false => String::new(),
            };
            let others = match store.others_busy() {
                0 => String::new(),
                n => format!(" · {n} more"),
            };
            format!("{doing}…{elapsed}{others}")
        });
        let state: AnyElement = if let Some(working) = working {
            h_flex()
                .gap(dp(6.))
                .text_color(theme.pop)
                .child(small_icon(Icon::Clock).text_color(theme.pop))
                .child(working)
                .into_any_element()
        } else if let Some(cut) = &store.cut {
            // On another commit, a commit goes above it, below it with ⇧, or one that's cut
            // rather than copied into it with ⌥.
            let onto_commit = matches!((cut, &store.selection),
                (Cut::Commit(_, ids) | Cut::CherryPick(_, ids), Some(Selection::Commit(_, id)))
                    if !ids.contains(id));
            let paste = match (shortcut(&PasteHere, cx), onto_commit) {
                (None, _) => Vec::new(),
                (Some(keys), false) => vec![(keys, "paste here")],
                (Some(keys), true) => [
                    (keys.clone(), "above"),
                    (format!("⇧{keys}").into(), "below"),
                ]
                .into_iter()
                .chain(
                    matches!(cut, Cut::Commit(..)).then(|| (format!("⌥{keys}").into(), "squash")),
                )
                .collect(),
            };
            // Its keys apart from what's cut, as far as the hints on the right are apart.
            h_flex()
                .gap(dp(10.))
                .min_w_0()
                // What's cut gives way first, then its verb is cut short, never drawn under
                // the keys.
                .child(
                    h_flex()
                        .gap(dp(6.))
                        .min_w_0()
                        .overflow_hidden()
                        .child(div().flex_none().text_color(theme.pop).child(cut.verb()))
                        .child(div().min_w_0().truncate().child(cut.describe(store))),
                )
                // Not while typing, where ⌘V pastes text and esc leaves the field.
                .when(!typing, |el| {
                    (el.children(paste.iter().map(|(keys, label)| hint(keys, label, &theme))))
                        .child(hint("esc", "cancel", &theme))
                })
                .into_any_element()
        } else if let Some((n, what, how)) = [
            // Only the navigator's highlight shows them otherwise.
            (
                store.picked.len(),
                "commit",
                Some(match picks_conflicted {
                    true => "drag them together, or press M to squash or discard them",
                    false => "drag them together, or press M to squash, uncommit or discard them",
                }),
            ),
            (
                detail.picked_line_count(),
                "line",
                Some("drag a grip or right-click to move the selection"),
            ),
            (detail.picked_files(), "file", None),
        ]
        .into_iter()
        .find(|(n, ..)| *n > 0)
        {
            h_flex()
                .gap(dp(10.))
                .child(
                    h_flex()
                        .gap(dp(6.))
                        .child(
                            div()
                                .text_color(theme.pop)
                                .child(format!("{} selected", crate::ui::plural(n, what))),
                        )
                        .children(how),
                )
                .child(hint("esc", "cancel", &theme))
                .into_any_element()
        } else if let Some((path, place)) = (detail.is_focused(window, cx)
            && !detail.diff_hidden(cx))
        .then(|| detail.cursor_place())
        .flatten()
        {
            // In the diff or the lists beside it, where the cursor is, as an editor's status
            // bar names the file and the line.
            h_flex()
                .gap(dp(6.))
                .min_w_0()
                .child(div().min_w_0().truncate().child(path))
                .child(div().flex_none().text_color(theme.text_3).child(place))
                .into_any_element()
        } else {
            // The selected project's standing against its target, and how fresh that is.
            let summary = store.selection.as_ref().and_then(|selection| {
                let entry = store.project(selection.project())?;
                let target = entry.workspace.as_ref()?.target.as_ref()?;
                // A target with no upstream stands where it is whatever a fetch brings. After
                // a failed fetch, what the remote has now is anyone's guess.
                let failed = store.fetch_failed_at(&entry.id).is_some() && !target.local;
                let fetched = match failed {
                    true => Some(" · couldn't fetch".to_owned()),
                    false => (store.fetched_at(&entry.id))
                        .filter(|_| !target.local)
                        .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|at| format!(" · fetched {}", crate::ui::ago(at.as_secs() as i64))),
                };
                // A stack can be on an older target commit, as one just applied can.
                let standing = match target.behind {
                    0 if target.needs_update => {
                        format!(
                            "{} · some branches are behind {}",
                            entry.project.name, target.name
                        )
                    }
                    0 => match &target.untracked {
                        Some(remote) => format!(
                            "{} · {} doesn't track {remote}",
                            entry.project.name, target.name
                        ),
                        // Not up to date for all it knows.
                        None if failed => {
                            let said =
                                format!("{} · couldn't fetch {}", entry.project.name, target.name);
                            return Some(said);
                        }
                        None => format!("{} · up to date with {}", entry.project.name, target.name),
                    },
                    n => format!(
                        "{} · {} behind {}{}",
                        entry.project.name,
                        crate::ui::plural(n, "commit"),
                        target.name,
                        crate::navigator::with_merged(target)
                    ),
                };
                Some(standing + fetched.as_deref().unwrap_or_default())
            });
            div().children(summary).into_any_element()
        };

        let detail_focused = detail.is_focused(window, cx);
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
            || (detail_focused && (!detail.has_files() || detail.diff_hidden(cx)))
            || detail.side_list_focused(window)
            || store.selection.is_none()
        {
            Vec::new()
        } else if detail.files_focused(window) {
            vec![(&NextFile, "next file")]
        } else if detail_focused && uncommitted {
            // ⌘X takes what's picked, or the hunk or file under the cursor; ⌥⌘Z the picked lines,
            // or the hunk, and on a file's header there's no hunk for it.
            let discard = match detail.picked_line_count() {
                0 => detail.on_hunk().then_some("discard hunk"),
                _ => Some("discard lines"),
            };
            // First, as the hints line up from the right, so the others stay put as the cursor
            // moves on and off a hunk.
            let discard = discard.map(|label| (&DiscardHunk as &dyn Action, label));
            (discard.into_iter())
                .chain([
                    (&NextHunk as &dyn Action, "next hunk"),
                    (&CutSelection, "cut"),
                ])
                .collect()
        } else if detail_focused && detail.cursor_cut(cx).is_some() {
            vec![(&NextHunk, "next hunk"), (&CutSelection, "cut")]
        } else if detail_focused {
            vec![(&NextFile, "next file"), (&NextHunk, "next hunk")]
        } else {
            // In the navigator, the keys for what's selected; what's cut says its own.
            let mut hints: Vec<(&dyn Action, &str)> = match &store.selection {
                Some(
                    Selection::Project(project)
                    | Selection::Section(project, Section::Upstream(None)),
                ) => {
                    let behind = (store.workspace(project))
                        .and_then(|ws| ws.target.as_ref())
                        .is_some_and(|target| target.needs_update);
                    [(&Fetch as &dyn Action, "fetch")]
                        .into_iter()
                        .chain(behind.then_some((&UpdateFromTarget as &dyn Action, "update")))
                        .collect()
                }
                Some(Selection::Branch(project, ref_name)) => {
                    let workspace = store.workspace(project);
                    let push = workspace.is_some_and(|ws| {
                        ws.can_push(ref_name) && ws.push_refused(ref_name).is_none()
                    });
                    let review = (workspace.and_then(|ws| ws.find_branch(ref_name)))
                        .is_some_and(|branch| branch.review.is_some());
                    (push.then_some((&Push as &dyn Action, "push")).into_iter())
                        .chain(review.then_some((&OpenPullRequest as &dyn Action, "pull request")))
                        .chain([(&RenameSelection as &dyn Action, "rename")])
                        .collect()
                }
                Some(Selection::Commit(_, id)) => {
                    let conflicted = picks_conflicted || conflicted(id);
                    [
                        (&CutSelection as &dyn Action, "cut"),
                        (&RenameSelection, "edit message"),
                    ]
                    .into_iter()
                    .chain((!conflicted).then_some((&UncommitSelection as &dyn Action, "uncommit")))
                    .collect()
                }
                Some(Selection::Uncommitted(_) | Selection::Worktree(..)) => {
                    vec![(&StartCommit, "commit")]
                }
                _ => Vec::new(),
            };
            if store.can_undo() {
                hints.push((&Undo, "undo"));
            }
            hints
        };

        h_flex()
            .flex_none()
            .h(dp(STATUS_BAR_HEIGHT))
            .px(dp(12.))
            .gap(dp(10.))
            .border_t_1()
            .border_color(theme.border_3)
            .bg(theme.bg_2)
            .text_size(dp(12.))
            .text_color(theme.text_2)
            .child(div().flex_1().min_w_0().truncate().child(state))
            .children(hints.into_iter().filter_map(|(action, label)| {
                let keys = shortcut(action, cx)?;
                Some(hint(&keys, label, &theme))
            }))
            .child(
                h_flex()
                    .id("open-commands")
                    .flex_none()
                    .gap(dp(4.))
                    .px(dp(6.))
                    .h(dp(22.))
                    .rounded(dp(4.))
                    .hover(|el| el.bg(theme.hover()))
                    .child(hint(
                        shortcut(&CommandPalette, cx).as_deref().unwrap_or("⌘K"),
                        "Commands",
                        &theme,
                    ))
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(CommandPalette), cx)),
            )
    }
}

/// An operation's label, which names it as a command ("Push main"), as what's under way
/// ("Pushing main"). An update rebases the stacks onto the target.
fn under_way(label: &str) -> String {
    let (verb, rest) = label.split_once(' ').unwrap_or((label, ""));
    let doing = match verb {
        "Update" => "Rebasing".to_owned(),
        "Snapshot" => "Taking a snapshot".to_owned(),
        // A short stressed last syllable doubles its consonant.
        "Commit" | "Uncommit" | "Split" | "Rewrap" => {
            let last = verb.get(verb.len() - 1..).unwrap_or_default();
            format!("{verb}{last}ing")
        }
        _ => format!("{}ing", verb.strip_suffix('e').unwrap_or(verb)),
    };
    match rest {
        "" => doing,
        rest => format!("{doing} {rest}"),
    }
}

/// A key and what it does, as a menu's shortcuts are: the words read as the bar's other text,
/// the key, plain rather than a keycap, a step quieter.
fn hint(keys: &str, label: &str, theme: &Theme) -> gpui::Div {
    h_flex()
        .flex_none()
        .gap(dp(4.))
        .child(
            div()
                .text_color(theme.text_3)
                .child(crate::ui::keys_text(keys)),
        )
        .child(label.to_owned())
}

#[cfg(test)]
mod tests {
    use super::under_way;

    #[test]
    fn labels_read_as_what_is_under_way() {
        let read = [
            "Update",
            "Push main",
            "Fetch",
            "Commit",
            "Cherry-pick 2 commits",
            "Resolve",
            "Undo",
            "Snapshot",
        ]
        .map(under_way);
        assert_eq!(
            read,
            [
                "Rebasing",
                "Pushing main",
                "Fetching",
                "Committing",
                "Cherry-picking 2 commits",
                "Resolving",
                "Undoing",
                "Taking a snapshot"
            ],
            "an update rebases, and each verb takes -ing as English spells it"
        );
    }
}
