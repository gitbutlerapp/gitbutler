//! A commit that wasn't made, as some of its changes couldn't go where it was asked to, as
//! when they build on another branch's commits. A modal says which and why, and offers where
//! they could all go instead, or to commit without them. Nothing is left out of a commit
//! without the user saying so here.

use gpui::{
    App, Context, DismissEvent, Div, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    ScrollHandle, SharedString, Window, div, prelude::*,
};

use crate::app::{AppView, Confirm};
use crate::backend::{
    BLOCKED_FILES_SHOWN, BLOCKED_HUNKS_SHOWN, Blocked, BlockedChange, BlockedHunk, CommitTo,
    FullName, Stacking, short_ref,
};
use crate::state::{BlockedCommit, Store};
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

/// Where the user can have a blocked commit go instead.
struct Offer {
    label: SharedString,
    to: CommitTo,
    leave_out: bool,
    /// Branches to stack first.
    restack: Option<Stacking>,
    primary: bool,
    /// It names a branch, so a long name cuts it short rather than push the row past the
    /// dialog's edge.
    shrinks: bool,
}

/// What can be done instead of `blocked`'s commit, the one primary choice last.
fn offers(blocked: &BlockedCommit) -> Vec<Offer> {
    let asked = &blocked.commit.to;
    let into = into(asked);
    let mut offers = Vec::new();
    if blocked.blocked.takes_others {
        offers.push(Offer {
            label: "Leave them out".into(),
            to: asked.clone(),
            leave_out: true,
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
    let merged = blocked.blocked.builds_on_merged;
    match &blocked.blocked.builds_on {
        // Built on later commits of the branch it goes below one of.
        Some(branch) if Some(branch) == into && !merged => {
            if matches!(asked, CommitTo::BelowCommit { .. }) {
                offers.push(Offer {
                    label: format!("Commit on top of {}", short_ref(branch)).into(),
                    to: CommitTo::Branch(branch.clone()),
                    leave_out: false,
                    restack: None,
                    primary: true,
                    shrinks: true,
                });
            }
        }
        Some(branch) => {
            offers.push(Offer {
                label: format!("New branch above {}", short_ref(branch)).into(),
                to: CommitTo::NewBranchAt {
                    beside: Some((branch.clone(), false)),
                    first: false,
                    name: name.clone(),
                },
                leave_out: false,
                restack: None,
                primary: merged,
                shrinks: true,
            });
            if !merged {
                offers.push(Offer {
                    label: format!("Commit to {}", short_ref(branch)).into(),
                    to: CommitTo::Branch(branch.clone()),
                    leave_out: false,
                    restack: None,
                    primary: true,
                    shrinks: true,
                });
            }
        }
        None => {}
    }
    // Stacked, the branch on top takes them all, as does a new branch above it, which is the
    // default.
    if let Some(stacking) = &blocked.blocked.stack {
        offers.push(Offer {
            label: format!("Stack and commit to {}", short_ref(&stacking.top)).into(),
            to: CommitTo::Branch(stacking.top.clone()),
            leave_out: false,
            restack: Some(stacking.clone()),
            primary: false,
            shrinks: true,
        });
        offers.push(Offer {
            label: "Stack and commit to a new branch".into(),
            to: CommitTo::NewBranchAt {
                beside: Some((stacking.top.clone(), false)),
                first: false,
                name,
            },
            leave_out: false,
            restack: Some(stacking.clone()),
            primary: true,
            shrinks: false,
        });
    }
    offers
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
        let primary = (self.blocked.as_ref())
            .and_then(|blocked| offers(blocked).into_iter().find(|offer| offer.primary));
        if let Some(Offer {
            to,
            leave_out,
            restack,
            ..
        }) = primary
        {
            self.commit_instead(to, leave_out, restack, cx);
        }
    }

    /// Commit again to `to`, leaving out what can't go with `leave_out`, or stacking first as
    /// `restack` says, and close.
    fn commit_instead(
        &mut self,
        to: CommitTo,
        leave_out: bool,
        restack: Option<Stacking>,
        cx: &mut Context<Self>,
    ) {
        if let Some(blocked) = self.blocked.take() {
            (self.store).update(cx, |store, cx| {
                store.commit_instead(blocked, to, leave_out, restack, cx)
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
        let to = &blocked.commit.to;
        let target = match into(to) {
            Some(branch) => short_ref(branch).into_owned(),
            None => "a new branch".to_owned(),
        };
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
        let offers = offers(blocked).into_iter().enumerate().map(|(ix, offer)| {
            let Offer {
                label,
                to,
                leave_out,
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
                    this.commit_instead(to.clone(), leave_out, restack.clone(), cx)
                }));
            // Cut short, its tooltip names the branch in full.
            match shrinks {
                true => choice.shrinks().tooltip(label),
                false => choice,
            }
        });
        let actions = h_flex()
            .gap(dp(6.))
            .child(
                button("commit-blocked-cancel", "Cancel")
                    .small()
                    .style(ButtonStyle::Ghost)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
            )
            .child(div().flex_grow(1.))
            // On one line: a long branch name cuts short the choices naming it, rather than
            // wrapping them or pushing them past the card's edge.
            .child(
                h_flex()
                    .min_w_0()
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
                    .child(format!("Can't commit all of this to {target}")),
            )
            .child(muted(explanation(&blocked.blocked, to), theme))
            .child(list)
            .child(actions)
    }
}
