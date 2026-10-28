//! The strip along the bottom of the window: what's running, what's cut, and the keys
//! that matter right now.

use gpui::{
    Action, AnyElement, Context, Div, Focusable as _, Hsla, Role, Stateful, Window, div, prelude::*,
};

use crate::app::{
    AppView, CommandPalette, CutSelection, Fetch, FocusDiff, OpenPullRequest, PasteHere, Push,
    RenameSelection, StartCommit, UncommitSelection, Undo, UpdateFromTarget,
};
use crate::backend::{Branch, Checkout, CommitId, FileChange, ReviewState, Workspace};
use crate::detail::{AmendTop, DiscardHunk, NextFile, NextHunk};
use crate::pr::{CreatePullRequest, create_offer};
use crate::state::{Cut, ProjectId, SLOW, Section, Selection};
use crate::theme::Theme;
use crate::ui::{
    Icon, STATUS_BAR_HEIGHT, Tooltip, ago, dot, dp, duration, h_flex, plural, shortcut, small_icon,
};

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
        } else if let Some(place) = (detail.is_focused(window, cx) && !detail.diff_hidden(cx))
            .then(|| detail.cursor_place(cx))
            .flatten()
        {
            // In the diff or the lists beside it, where the cursor is, as an editor's status
            // bar says the line.
            div().child(place).into_any_element()
        } else {
            self.render_notes(&theme, cx)
        };
        // How fresh the selected project's view of its remote is, which a click fetches again.
        // A target with no upstream stands where it is whatever a fetch brings. Not on the
        // project's page, whose header says it, with its own Fetch.
        let fetched = (store.selection.as_ref()).and_then(|selection| {
            if let Selection::Project(_) | Selection::Section(_, Section::Upstream(None)) =
                selection
            {
                return None;
            }
            let id = selection.project();
            store.workspace(id)?.target.as_ref().filter(|t| !t.local)?;
            match store.fetch_failed_at(id) {
                Some(_) => Some((true, "Couldn't fetch".to_owned())),
                None => (store.fetched_at(id))
                    .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|at| (false, format!("Fetched {}", ago(at.as_secs() as i64)))),
            }
        });

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
                Some(selection @ Selection::Branch(project, ref_name)) => {
                    let workspace = store.workspace(project);
                    let push =
                        workspace.is_some_and(|ws| ws.push_blocked(ref_name.as_ref()).is_none());
                    let review = (workspace.and_then(|ws| ws.find_branch(ref_name.as_ref())))
                        .is_some_and(|branch| branch.review.is_some());
                    let create = (create_offer(store, selection) == Some(Ok(())))
                        .then_some((&CreatePullRequest as &dyn Action, "create pull request"));
                    (push.then_some((&Push as &dyn Action, "push")).into_iter())
                        .chain(review.then_some((&OpenPullRequest as &dyn Action, "pull request")))
                        .chain(create)
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
            .when_some(fetched, |el, (failed, said)| {
                let tip = if failed { "Fetch again" } else { "Fetch" };
                el.child(
                    clickable("fetched", &said, &theme)
                        .text_color(if failed { theme.warn } else { theme.text_3 })
                        .when(failed, |el| {
                            el.child(small_icon(Icon::Warning).text_color(theme.warn))
                        })
                        .child(said)
                        .tooltip(Tooltip::keyed(tip, shortcut(&Fetch, cx)))
                        .on_click(|_, window, cx| window.dispatch_action(Box::new(Fetch), cx)),
                )
            })
            .child(
                clickable("open-commands", "Commands", &theme)
                    .child(hint(
                        shortcut(&CommandPalette, cx).as_deref().unwrap_or("⌘K"),
                        "Commands",
                        &theme,
                    ))
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(CommandPalette), cx)),
            )
            .children(self.rebuild.is_some().then(|| self.build_button(cx)))
    }

    /// The bar's left while nothing is under way, picked or cut: what in the selected project
    /// needs the user, where something is writing, the commits not read yet and what ⌘Z takes
    /// back, in that order, each a click from what it names. What doesn't fit goes whole,
    /// from the end.
    fn render_notes(&self, theme: &Theme, cx: &Context<Self>) -> AnyElement {
        let store = self.store.read(cx);
        let Some(project) = (store.selection.as_ref()).map(|s| s.project().clone()) else {
            return div().into_any_element();
        };
        let shown = store.selection.as_ref();
        let mut notes = Vec::new();
        if let Some(workspace) = store.workspace(&project) {
            notes.extend(
                needs(&project, workspace, shown)
                    .into_iter()
                    .map(|(need, text, to)| {
                        let (id, glyph, color, tip) = match need {
                            Need::ConflictedCommits => (
                                "conflicted-commits",
                                Icon::Warning,
                                theme.warn,
                                "Show the conflict",
                            ),
                            Need::ConflictedFiles => (
                                "conflicted-files",
                                Icon::Warning,
                                theme.warn,
                                "Show the conflict",
                            ),
                            Need::FailedChecks => (
                                "failed-checks",
                                Icon::Cross,
                                theme.danger,
                                "Show the branch",
                            ),
                        };
                        Note {
                            id,
                            mark: small_icon(glyph).text_color(color).into_any_element(),
                            color: Some(color),
                            text,
                            tip,
                            to,
                        }
                    }),
            );
            // The checkout selected says both of its own, so only the others' count.
            let here = (self.detail.read(cx).selected_checkout(cx)).map(|(_, checkout)| checkout);
            let writing: Vec<Checkout> = (store.being_written(&project).into_iter())
                .filter(|checkout| Some(checkout) != here.as_ref())
                .collect();
            let names: Vec<String> = (writing.iter())
                .map(|checkout| match checkout {
                    Checkout::Main => "Main worktree".to_owned(),
                    Checkout::Linked(name) => name.to_string(),
                })
                .collect();
            if let (Some(text), Some(first)) = (written_in(&names), writing.first()) {
                notes.push(Note {
                    id: "writing",
                    mark: dot(theme.pop).into_any_element(),
                    color: None,
                    text,
                    tip: "Show the worktree",
                    to: match first {
                        Checkout::Main => Selection::Uncommitted(project.clone()),
                        Checkout::Linked(name) => {
                            Selection::Worktree(project.clone(), name.clone())
                        }
                    },
                });
            }
            let unread: Vec<CommitId> = (workspace.lanes.iter())
                .filter(|lane| Some(&lane.checkout) != here.as_ref())
                .flat_map(|lane| lane.branches.iter().flat_map(|branch| &branch.commits))
                .filter(|commit| store.is_unread(&project, *commit))
                .map(|commit| commit.id)
                .collect();
            if let Some(first) = unread.first() {
                notes.push(Note {
                    id: "unread",
                    mark: small_icon(Icon::Commit)
                        .text_color(theme.text_3)
                        .into_any_element(),
                    color: None,
                    text: format!("{} unread", unread.len()),
                    tip: "Read the first one",
                    to: Selection::Commit(project.clone(), *first),
                });
            }
        }
        // ⌘Z follows the user, so this may be another project's, which it names. Operations
        // history lists it first.
        if let Some((done_in, said, at)) = store.last_done()
            && store.project(done_in).is_some()
            && shown != Some(&Selection::History(done_in.clone()))
        {
            notes.push(Note {
                id: "last-done",
                mark: small_icon(Icon::Undo)
                    .text_color(theme.text_3)
                    .into_any_element(),
                color: None,
                text: format!("{said}{} · {}", store.in_project(done_in), ago(at / 1000)),
                tip: "Show the operations history",
                to: Selection::History(done_in.clone()),
            });
        }
        // Each a whole line's height, so what wraps to a second line is out of sight. Pulled
        // left by its hover's padding, so its words start where the other states' do.
        h_flex()
            .w_full()
            .h(dp(22.))
            .ml(dp(-6.))
            .flex_wrap()
            .overflow_hidden()
            .children(notes.into_iter().map(|note| {
                let to = note.to;
                clickable(note.id, &note.text, theme)
                    .when_some(note.color, |el, color| el.text_color(color))
                    .child(note.mark)
                    .child(note.text)
                    .tooltip(Tooltip::text(note.tip))
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.select(to.clone(), window, cx)),
                    )
            }))
            .into_any_element()
    }
}

/// Something the bar's left says while nothing else is under way, and where a click on it
/// goes. Its words take `color`, or the bar's.
struct Note {
    id: &'static str,
    mark: AnyElement,
    color: Option<Hsla>,
    text: String,
    tip: &'static str,
    to: Selection,
}

/// What kind of thing needs the user, which says how the bar marks it.
#[derive(Debug, PartialEq)]
enum Need {
    ConflictedCommits,
    ConflictedFiles,
    FailedChecks,
}

/// What in `workspace` needs the user, first things first: its conflicted commits, its
/// checkouts' conflicted files, and the main checkout's branches whose CI failed, as the
/// forge last said. Linked worktrees' checks aren't refreshed, so they'd be stale. What the
/// page `shown` says of itself is left out. Each says how many, with where a click goes: the
/// first of them.
fn needs(
    project: &ProjectId,
    workspace: &Workspace,
    shown: Option<&Selection>,
) -> Vec<(Need, String, Selection)> {
    let elsewhere = |to: &Selection| shown != Some(to);
    let mut needs = Vec::new();
    let commits: Vec<Selection> = (workspace.commits())
        .filter(|commit| commit.conflicted)
        .map(|commit| Selection::Commit(project.clone(), commit.id))
        .filter(elsewhere)
        .collect();
    if let Some(first) = commits.first() {
        let text = plural(commits.len(), "conflicted commit");
        needs.push((Need::ConflictedCommits, text, first.clone()));
    }
    let count = |files: &[FileChange]| files.iter().filter(|file| file.conflicted).count();
    let files: Vec<(usize, Selection)> = std::iter::once((
        count(&workspace.uncommitted),
        Selection::Uncommitted(project.clone()),
    ))
    .chain((workspace.worktrees.iter()).map(|worktree| {
        let to = Selection::Worktree(project.clone(), worktree.name.clone());
        (count(&worktree.uncommitted), to)
    }))
    .filter(|(n, to)| *n > 0 && elsewhere(to))
    .collect();
    if let Some((_, first)) = files.first() {
        let text = plural(files.iter().map(|(n, _)| n).sum(), "conflicted file");
        needs.push((Need::ConflictedFiles, text, first.clone()));
    }
    let failed: Vec<(&Branch, Selection)> = (workspace.branches_in(&Checkout::Main))
        .filter(|branch| {
            branch.review.as_ref().is_some_and(|review| {
                matches!(review.state, ReviewState::Open | ReviewState::Draft)
                    && review.checks.is_some_and(|checks| checks.failed > 0)
            })
        })
        .map(|branch| {
            (
                branch,
                Selection::Branch(project.clone(), branch.ref_name.clone()),
            )
        })
        .filter(|(_, to)| elsewhere(to))
        .collect();
    if let Some((first, to)) = failed.first() {
        let text = match failed.len() {
            1 => format!("CI failed on {}", first.name),
            n => format!("CI failed on {n} branches"),
        };
        needs.push((Need::FailedChecks, text, to.clone()));
    }
    needs
}

/// Where files are being written, as a checkout's overview says it of its own, named as the
/// breadcrumbs name each checkout.
fn written_in(names: &[String]) -> Option<String> {
    Some(match names {
        [] => return None,
        [one] => format!("Being written now in {one}"),
        [one, other] => format!("Being written now in {one} and {other}"),
        many => format!("Being written now in {} worktrees", many.len()),
    })
}

/// A part of the bar a click acts on, which a hover marks as it does the Commands button.
fn clickable(id: &'static str, label: &str, theme: &Theme) -> Stateful<Div> {
    h_flex()
        .id(id)
        .role(Role::Button)
        .aria_label(label.to_owned())
        .flex_none()
        .gap(dp(6.))
        .px(dp(6.))
        .h(dp(22.))
        .rounded(dp(4.))
        .hover(|el| el.bg(theme.hover()))
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
    use super::{Need, needs, under_way};
    use crate::backend::model::build::{branch, full, id, workspace, worktree};
    use crate::backend::{Checks, FileChange, Review, ReviewState};
    use crate::state::{ProjectId, Selection};

    #[test]
    fn needs_list_conflicts_then_failed_checks_each_going_to_the_first() {
        let failing = |state| {
            let checks = Checks {
                passed: 3,
                failed: 1,
                running: 0,
            };
            Some(Review {
                state,
                checks: Some(checks),
                ..Review::default()
            })
        };
        let mut stack = vec![
            branch("auth", &[1, 2]),
            branch("login", &[3]),
            branch("old", &[4]),
        ];
        stack[0].commits[1].conflicted = true;
        stack[1].review = failing(ReviewState::Open);
        stack[2].review = failing(ReviewState::Merged);
        let mut agent = branch("agent", &[5]);
        agent.review = failing(ReviewState::Open);
        let mut ws = workspace(vec![stack], vec![worktree("agent-x", None, vec![agent])]);
        let conflicted = |path| FileChange {
            conflicted: true,
            ..FileChange::bare(path)
        };
        ws.uncommitted = vec![conflicted("main.rs")];
        ws.worktrees[0].uncommitted = vec![conflicted("a.rs"), conflicted("b.rs")];

        let project = ProjectId::from("shop");
        assert_eq!(
            needs(&project, &ws, None),
            [
                (
                    Need::ConflictedCommits,
                    "1 conflicted commit".to_owned(),
                    Selection::Commit(project.clone(), id(2))
                ),
                (
                    Need::ConflictedFiles,
                    "3 conflicted files".to_owned(),
                    Selection::Uncommitted(project.clone())
                ),
                (
                    Need::FailedChecks,
                    "CI failed on login".to_owned(),
                    Selection::Branch(project.clone(), Some(full("refs/heads/login")))
                ),
            ],
            "a merged pull request's checks and a linked worktree's, which go stale, don't count"
        );
        let main = Selection::Uncommitted(project.clone());
        assert_eq!(
            needs(&project, &ws, Some(&main))[1],
            (
                Need::ConflictedFiles,
                "2 conflicted files".to_owned(),
                Selection::Worktree(project.clone(), "agent-x".into())
            ),
            "Main worktree's page shows its own conflicts, and the linked worktree's still count"
        );
    }

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
