//! A commit or amend that wasn't made, as some of its changes couldn't go where they were
//! asked to, as when they build on another branch's commits. A modal says which and why, what
//! doing it anyway with conflicts would leave, and offers where they could all go instead, or
//! to go ahead without them. Nothing is left out or conflicted without the user saying so here.

use gpui::{
    App, Context, DismissEvent, Div, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    ScrollHandle, SharedString, Window, div, prelude::*,
};

use crate::app::{AppView, Confirm};
use crate::backend::{
    BLOCKED_FILES_SHOWN, BLOCKED_HUNKS_SHOWN, Blocked, BlockedChange, BlockedHunk, CommitId,
    CommitTo, Conflicts, FullName, Stacking, WithConflicts, short_ref,
};
use crate::state::{Asked, BlockedCommit, Instead, Store};
use crate::theme::Theme;
use crate::ui::diff::excerpt;
use crate::ui::modal::{ModalSize, modal};
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::{ButtonStyle, button, commit_title, dp, h_flex, muted, plural, v_flex};

/// How many of a hunk's lines it shows.
const LINES: usize = 6;
/// How tall the list of what was left out grows before it scrolls.
const LIST_HEIGHT: f32 = 320.;

impl AppView {
    /// Ask where the commit the store last blocked goes instead, if one is waiting.
    pub(crate) fn ask_where_blocked_commit_goes(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(blocked) = self
            .store
            .update(cx, |store, _| store.take_blocked_commit())
        else {
            return;
        };
        let store = self.store.clone();
        let view = cx.new(|cx| CommitBlocked::new(store, blocked, cx));
        self.open_modal(view, window, cx);
    }
}

/// What the user can have a blocked commit or amend do instead.
struct Offer {
    label: SharedString,
    instead: Instead,
    leave_out: bool,
    /// What happens to changes that don't merge there.
    conflicts: Conflicts,
    /// Branches to stack first.
    restack: Option<Stacking>,
    primary: bool,
    /// It names a branch, so a long name cuts it short rather than push the row past the
    /// dialog's edge.
    shrinks: bool,
}

/// What can be done instead of what `blocked` asked, naming commits with `named`, the one
/// primary choice last.
fn offers(blocked: &BlockedCommit, named: impl Fn(CommitId) -> String) -> Vec<Offer> {
    let mut offers = match &blocked.asked {
        Asked::Commit { commit, .. } => commit_offers(&blocked.blocked, &commit.to),
        Asked::Amend { amend, .. } => amend_offers(&blocked.blocked, amend.into, named),
    };
    offers.sort_by_key(|offer| offer.primary);
    offers
}

/// What can be done instead of committing to `asked`, as `blocked` says.
fn commit_offers(blocked: &Blocked, asked: &CommitTo) -> Vec<Offer> {
    let into = into(asked);
    let mut offers = Vec::new();
    if blocked.takes_others {
        offers.push(Offer {
            label: "Leave them out".into(),
            instead: Instead::Commit(asked.clone()),
            leave_out: true,
            conflicts: Conflicts::Reject,
            restack: None,
            primary: false,
            shrinks: false,
        });
    }
    // Where it was asked to go, as the dry run found it would conflict.
    if blocked.with_conflicts.is_some() {
        offers.push(Offer {
            label: "Commit with conflicts".into(),
            instead: Instead::Commit(asked.clone()),
            leave_out: false,
            conflicts: Conflicts::Keep,
            restack: None,
            primary: false,
            shrinks: false,
        });
    }
    // A new branch keeps the name typed for it.
    let name = match asked {
        CommitTo::NewBranchAt { name, .. } => name.clone(),
        _ => None,
    };
    // A merged branch takes no commits, only a new branch above it.
    let merged = blocked.builds_on_merged;
    match &blocked.builds_on {
        // Built on later commits of the branch it goes below one of.
        Some(branch) if Some(branch) == into && !merged => {
            if matches!(asked, CommitTo::BelowCommit { .. }) {
                offers.push(Offer {
                    label: format!("Commit on top of {}", short_ref(branch)).into(),
                    instead: Instead::Commit(CommitTo::Branch(branch.clone())),
                    leave_out: false,
                    conflicts: Conflicts::Reject,
                    restack: None,
                    primary: true,
                    shrinks: true,
                });
            }
        }
        Some(branch) => {
            offers.push(Offer {
                label: format!("New branch above {}", short_ref(branch)).into(),
                instead: Instead::Commit(CommitTo::NewBranchAt {
                    beside: Some((branch.clone(), false)),
                    first: false,
                    name: name.clone(),
                }),
                leave_out: false,
                conflicts: Conflicts::Reject,
                restack: None,
                primary: merged,
                shrinks: true,
            });
            if !merged {
                offers.push(Offer {
                    label: format!("Commit to {}", short_ref(branch)).into(),
                    instead: Instead::Commit(CommitTo::Branch(branch.clone())),
                    leave_out: false,
                    conflicts: Conflicts::Reject,
                    restack: None,
                    primary: true,
                    shrinks: true,
                });
            }
        }
        None => {}
    }
    // Stacked under where it was asked to go, it goes there.
    if let Some(stacking) = &blocked.restack {
        let primary = !offers.iter().any(|offer: &Offer| offer.primary);
        offers.push(Offer {
            label: format!("Stack on {} and commit", short_ref(&stacking.onto)).into(),
            instead: Instead::Commit(asked.clone()),
            leave_out: false,
            conflicts: Conflicts::Reject,
            restack: Some(stacking.clone()),
            primary,
            shrinks: true,
        });
    }
    // Stacked, the branch on top takes them all, as does a new branch above it, which is the
    // default.
    if let Some(stacking) = &blocked.stack {
        offers.push(Offer {
            label: format!("Stack and commit to {}", short_ref(&stacking.top)).into(),
            instead: Instead::Commit(CommitTo::Branch(stacking.top.clone())),
            leave_out: false,
            conflicts: Conflicts::Reject,
            restack: Some(stacking.clone()),
            primary: false,
            shrinks: true,
        });
        let primary = !offers.iter().any(|offer| offer.primary);
        offers.push(Offer {
            label: "Stack and commit to a new branch".into(),
            instead: Instead::Commit(CommitTo::NewBranchAt {
                beside: Some((stacking.top.clone(), false)),
                first: false,
                name,
            }),
            leave_out: false,
            conflicts: Conflicts::Reject,
            restack: Some(stacking.clone()),
            primary,
            shrinks: false,
        });
    }
    offers
}

/// What can be done instead of amending `into`, as `blocked` says, naming commits with
/// `named`: amend the commit they build on, or `into` once what they build on is stacked below
/// it.
fn amend_offers(
    blocked: &Blocked,
    into: CommitId,
    named: impl Fn(CommitId) -> String,
) -> Vec<Offer> {
    let offer = |label: String, instead, leave_out, conflicts, restack, primary| Offer {
        label: label.into(),
        instead: Instead::Amend(instead),
        leave_out,
        conflicts,
        restack,
        primary,
        shrinks: true,
    };
    let mut offers = Vec::new();
    if blocked.takes_others {
        let label = "Amend unlocked changes".to_owned();
        offers.push(offer(label, into, true, Conflicts::Reject, None, false));
    }
    if blocked.with_conflicts.is_some() {
        let label = "Amend with conflicts".to_owned();
        offers.push(offer(label, into, false, Conflicts::Keep, None, false));
    }
    if let Some(commit) = blocked.instead {
        let label = format!("Amend {}", named(commit));
        offers.push(offer(label, commit, false, Conflicts::Reject, None, true));
    }
    if let Some(stacking) = &blocked.restack {
        let primary = blocked.instead.is_none();
        let label = "Restack and amend".to_owned();
        let restack = Some(stacking.clone());
        offers.push(offer(
            label,
            into,
            false,
            Conflicts::Reject,
            restack,
            primary,
        ));
    }
    offers
}

/// Where `to` commits, as the dialog's title says it.
fn target(to: &CommitTo) -> String {
    match to {
        CommitTo::Branch(branch) | CommitTo::BelowCommit { branch, .. } => {
            short_ref(branch).into_owned()
        }
        CommitTo::NewBranchAt {
            beside: Some((branch, below)),
            ..
        } => match below {
            true => format!("a new branch below {}", short_ref(branch)),
            false => format!("a new branch above {}", short_ref(branch)),
        },
        CommitTo::NewBranchAt { beside: None, .. } => "a new branch".to_owned(),
    }
}

/// The branch `to` commits to that's there already.
fn into(to: &CommitTo) -> Option<&FullName> {
    match to {
        CommitTo::Branch(branch) | CommitTo::BelowCommit { branch, .. } => Some(branch),
        CommitTo::NewBranchAt { .. } => None,
    }
}

/// Why the commit wasn't made, and what would make it, given where it was asked to go.
fn explanation(blocked: &Blocked, to: &CommitTo) -> String {
    let into = into(to);
    let named = |branch: &FullName| short_ref(branch).into_owned();
    match &blocked.builds_on {
        Some(branch) if blocked.builds_on_merged => format!(
            "The changes below build on commits in {}, which is merged, so they can only be \
             committed to a new branch above it.",
            named(branch)
        ),
        Some(branch) if Some(branch) == into => format!(
            "The changes below build on later commits in {}, so they can only go on top of \
             it.",
            named(branch)
        ),
        Some(branch) => format!(
            "The changes below build on commits in {}, so they can only be committed there or \
             to a branch stacked on it.",
            named(branch)
        ),
        None => {
            let mut branches: Vec<&FullName> = Vec::new();
            for branch in blocked.changes.iter().flat_map(|c| &c.builds_on) {
                if !branches.contains(&branch) {
                    branches.push(branch);
                }
            }
            match (branches.as_slice(), &blocked.stack) {
                (_, Some(stacking)) => {
                    let (moved, onto, top) = (
                        listed(stacking.branches.iter().map(named)),
                        named(&stacking.onto),
                        named(&stacking.top),
                    );
                    let pushed = match stacking.pushed.as_slice() {
                        [] => String::new(),
                        [branch] => format!(
                            " {} is pushed, so stacking rewrites its commits and it needs a \
                             force push.",
                            named(branch)
                        ),
                        branches => format!(
                            " {} are pushed, so stacking rewrites their commits and they need a \
                             force push.",
                            listed(branches.iter().map(named))
                        ),
                    };
                    format!(
                        "The changes below build on branches that aren't stacked. Stacking \
                         {moved} on {onto} lets {top}, or a new branch above it, take them \
                         all.{pushed}"
                    )
                }
                ([] | [_], None) => "The changes below can't be committed there.".to_owned(),
                (_, None) => "The changes below build on branches that aren't stacked, so no \
                              one branch can take them all. Stack those branches to commit them \
                              together."
                    .to_owned(),
            }
        }
    }
}

/// Why the amend into `named` wasn't made, as `blocked` says.
fn amend_explanation(blocked: &Blocked, named: &str) -> String {
    match &blocked.builds_on {
        Some(branch) if blocked.builds_on_merged => format!(
            "The changes below build on commits in {}, which is merged, so no commit there \
             takes them.",
            short_ref(branch)
        ),
        Some(branch) => format!(
            "The changes below build on commits in {} that {named} doesn't have.",
            short_ref(branch)
        ),
        None => format!("The changes below build on commits {named} doesn't have."),
    }
}

/// What stacking as `stacking` says does, before what it's for lands on top.
fn restack_note(stacking: &Stacking) -> String {
    let named = |branch: &FullName| short_ref(branch).into_owned();
    let pushed = match stacking.pushed.as_slice() {
        [] => String::new(),
        [branch] => format!(" {} is pushed, so it needs a force push.", named(branch)),
        branches => format!(
            " {} are pushed, so they need a force push.",
            listed(branches.iter().map(named))
        ),
    };
    format!(
        "Stacking {} on {} puts what they build on below them.{pushed}",
        listed(stacking.branches.iter().map(named)),
        named(&stacking.onto)
    )
}

/// What doing it with its conflicts does, as the dry run found: `done` as "Committed", to
/// `named` as "the new commit".
fn with_conflicts(kept: &WithConflicts, done: &str, named: &str) -> String {
    let also = (kept.also.iter()).map(|(branch, count)| {
        let commits = plural(*count, "commit");
        match branch {
            Some(branch) => format!("{commits} in {}", short_ref(branch)),
            None => commits,
        }
    });
    match kept.also.is_empty() {
        true => format!(
            "{done} with its conflicts, only {named} is conflicted. They stay in it, not in \
             your files, until you resolve it."
        ),
        false => format!(
            "{done} with its conflicts, it leaves {} conflicted too. They stay in the commits, \
             not in your files, until you resolve them.",
            listed(also)
        ),
    }
}

/// `names` as a sentence lists them: "a", "a and b", "a, b and c".
fn listed(names: impl IntoIterator<Item = String>) -> String {
    let names: Vec<String> = names.into_iter().collect();
    match names.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    }
}

/// Where `hunk` is in its file, as "Line 5" or "Lines 5–9", and how many lines that is. One
/// that only removes lines is where they were, as "Removed lines 1–30": they're nowhere now.
fn lines_of(hunk: &BlockedHunk) -> (String, u32) {
    let range = |start: u32, lines: u32| match lines {
        1 => format!("line {start}"),
        _ => format!("lines {start}–{}", start.saturating_add(lines - 1)),
    };
    match hunk.new_lines {
        0 => (
            format!("Removed {}", range(hunk.old_start, hunk.old_lines)),
            hunk.old_lines,
        ),
        lines => (capitalized(&range(hunk.new_start, lines)), lines),
    }
}

/// What `hunk` builds on, as `Line 5 builds on "Edit line 5" (vtlrvnl) in feature`, or
/// `Lines 5–9 build on …`.
fn builds_on(hunk: &BlockedHunk) -> String {
    let commits: Vec<String> = (hunk.builds_on.iter())
        .map(|commit| {
            let named = format!("\"{}\" ({})", commit_title(&commit.title), commit.reference);
            match &commit.branch {
                Some(branch) => format!("{named} in {}", short_ref(branch)),
                None => named,
            }
        })
        .collect();
    let (lines, count) = lines_of(hunk);
    let verb = match count {
        1 => "builds",
        _ => "build",
    };
    format!("{lines} {verb} on {}", commits.join(" and "))
}

/// The first few lines of `hunk`, as the diff draws them.
fn hunk_lines(hunk: &BlockedHunk, theme: &Theme) -> Div {
    let shown = hunk.lines.get(..LINES).unwrap_or(&hunk.lines);
    let more = hunk.lines.len().saturating_sub(LINES);
    excerpt(shown, theme).when(more > 0, |el| {
        el.child(
            div()
                .px(dp(8.))
                .py(dp(2.))
                .text_color(theme.text_2)
                .child(format!("{} more", plural(more, "line"))),
        )
    })
}

/// A file left out: each of its hunks that builds on another commit, with what it changes,
/// or why else it was left out.
fn left_out(change: &BlockedChange, theme: &Theme) -> Div {
    let path = div()
        .min_w_0()
        .truncate()
        .text_size(dp(13.))
        .font_weight(FontWeight::MEDIUM)
        .child(change.path.clone());
    let why = match (change.hunks.as_slice(), change.suspects.as_slice()) {
        ([], []) => Some(capitalized(change.why)),
        ([], suspects) => {
            let names: Vec<_> = suspects.iter().map(|b| short_ref(b)).collect();
            Some(format!(
                "Builds on commits in {} that change this file too",
                names.join(", ")
            ))
        }
        _ => None,
    };
    let hunks = change.hunks.iter().take(BLOCKED_HUNKS_SHOWN).map(|hunk| {
        v_flex()
            .gap(dp(4.))
            .child(muted(builds_on(hunk), theme))
            .child(hunk_lines(hunk, theme))
    });
    let more = change.hunks.len().saturating_sub(BLOCKED_HUNKS_SHOWN);
    v_flex()
        .min_w_0()
        .gap(dp(6.))
        .child(path)
        .when_some(why, |el, why| el.child(muted(why, theme)))
        .children(hunks)
        .when(more > 0, |el| {
            el.child(muted(format!("and {} more", plural(more, "change")), theme))
        })
}

/// `text` with its first letter a capital, as a reason starts a line.
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The modal asking where a blocked commit goes instead.
pub struct CommitBlocked {
    store: Entity<Store>,
    /// Until it's acted on.
    blocked: Option<BlockedCommit>,
    focus: FocusHandle,
    scroll: ScrollHandle,
}

impl EventEmitter<DismissEvent> for CommitBlocked {}

impl Focusable for CommitBlocked {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl CommitBlocked {
    fn new(store: Entity<Store>, blocked: BlockedCommit, cx: &mut Context<Self>) -> Self {
        Self {
            store,
            blocked: Some(blocked),
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
        }
    }

    /// Take the highlighted choice, as Return does, while the keys are on the dialog itself
    /// rather than on a button Tab went to, which Return presses instead.
    fn confirm(&mut self, _: &Confirm, window: &mut Window, cx: &mut Context<Self>) {
        if !self.focus.is_focused(window) {
            return cx.propagate();
        }
        let named = |id| self.named(id, cx);
        let primary = (self.blocked.as_ref()).and_then(|blocked| {
            offers(blocked, named)
                .into_iter()
                .find(|offer| offer.primary)
        });
        if let Some(Offer {
            instead,
            leave_out,
            conflicts,
            restack,
            ..
        }) = primary
        {
            self.retry(instead, leave_out, conflicts, restack, cx);
        }
    }

    /// Commit `id` of the project, as the dialog names it: `"title"`.
    fn named(&self, id: CommitId, cx: &App) -> String {
        let project = self.blocked.as_ref().map(|blocked| &blocked.project);
        let commit = project.and_then(|project| self.store.read(cx).find_commit(project, id));
        commit.map_or("the commit".into(), |(_, commit)| {
            format!("\"{}\"", commit_title(&commit.title))
        })
    }

    /// Do it again as `instead` says, leaving out what can't go with `leave_out`, keeping what
    /// doesn't merge as `conflicts` says, or stacking first as `restack` says, and close.
    fn retry(
        &mut self,
        instead: Instead,
        leave_out: bool,
        conflicts: Conflicts,
        restack: Option<Stacking>,
        cx: &mut Context<Self>,
    ) {
        if let Some(blocked) = self.blocked.take() {
            (self.store).update(cx, |store, cx| {
                store.retry(blocked, instead, leave_out, conflicts, restack, cx)
            });
        }
        cx.emit(DismissEvent);
    }
}

impl Render for CommitBlocked {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let card = modal(ModalSize::Medium, theme)
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::confirm));
        let Some(blocked) = &self.blocked else {
            return card;
        };
        let why = &blocked.blocked;
        let (title, explanation, kept) = match &blocked.asked {
            Asked::Commit { commit, .. } => {
                let target = target(&commit.to);
                let title = match &why.with_conflicts {
                    Some(_) => format!("Committing this to {target} leaves it conflicted"),
                    None => format!("Can't commit all of this to {target}"),
                };
                let kept = (why.with_conflicts.as_ref())
                    .map(|kept| with_conflicts(kept, "Committed", "the new commit"));
                (title, explanation(why, &commit.to), kept)
            }
            Asked::Amend { amend, .. } => {
                let named = self.named(amend.into, cx);
                let title = match &why.with_conflicts {
                    Some(_) => format!("Amending {named} leaves it conflicted"),
                    None => format!("Can't amend all of this into {named}"),
                };
                let kept = (why.with_conflicts.as_ref())
                    .map(|kept| with_conflicts(kept, "Amended", &named));
                (title, amend_explanation(why, &named), kept)
            }
        };
        let restacked = why.restack.as_ref().map(restack_note);
        let changes = &blocked.blocked.changes;
        let rows = (changes.iter().take(BLOCKED_FILES_SHOWN)).map(|change| left_out(change, theme));
        let more = changes.len().saturating_sub(BLOCKED_FILES_SHOWN);
        let list = v_flex()
            .id("commit-blocked-changes")
            .max_h(dp(LIST_HEIGHT))
            .gap(dp(14.))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .children(rows)
            .when(more > 0, |el| {
                el.child(muted(format!("and {}", plural(more, "more file")), theme))
            });
        let list = scrolled(
            list,
            "commit-blocked-scrollbar",
            Scroll::Div(self.scroll.clone()),
            theme,
        )
        .flex_none();
        let named = |id| self.named(id, cx);
        let offers = offers(blocked, named)
            .into_iter()
            .enumerate()
            .map(|(ix, offer)| {
                let Offer {
                    label,
                    instead,
                    leave_out,
                    conflicts,
                    restack,
                    primary,
                    shrinks,
                } = offer;
                let style = match primary {
                    true => ButtonStyle::Pop,
                    false => ButtonStyle::Gray,
                };
                let choice = button(("commit-blocked-offer", ix), label.clone())
                    .small()
                    .style(style)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.retry(instead.clone(), leave_out, conflicts, restack.clone(), cx)
                    }));
                // Cut short, its tooltip names the branch in full.
                match shrinks {
                    true => choice.shrinks().tooltip(label),
                    false => choice,
                }
            });
        let actions = h_flex()
            .items_end()
            .gap(dp(6.))
            .child(
                button("commit-blocked-cancel", "Cancel")
                    .small()
                    .style(ButtonStyle::Ghost)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
            )
            .child(div().flex_grow(1.))
            // Wrapped onto more lines as they need, so each reads in full; only a branch name
            // too long for a line of its own cuts short the choice naming it.
            .child(
                h_flex()
                    .min_w_0()
                    .flex_wrap()
                    .justify_end()
                    .gap(dp(6.))
                    .children(offers),
            );
        card.p(dp(16.))
            .gap(dp(12.))
            .child(
                div()
                    .text_size(dp(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(muted(explanation, theme))
            .when_some(restacked, |el, note| el.child(muted(note, theme)))
            .when_some(kept, |el, kept| el.child(muted(kept, theme)))
            .child(list)
            .child(actions)
    }
}
