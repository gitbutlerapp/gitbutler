//! The Branches page: branches outside the workspace, newest first, local ones and the ones
//! only a remote has alike, each with what it would bring in, ready to apply or delete. Beside
//! the list, an overview of the team's branches until one is opened, then its diff.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui::{
    AnyElement, App, ClickEvent, Context, MouseButton, MouseDownEvent, Pixels, Point, SharedString,
    Window, div, prelude::*, px, relative,
};

use super::Detail;
use super::panes::{
    Crumb, CrumbTo, list_empty, measured, meta_item, meta_row, page_title, side_list, title_text,
};
use super::upstream::{DayGesture, DayPick, activity_strip, commit_author};

use crate::app::{Collapse, Expand, SelectNext, SelectPrevious};
use crate::backend::branches::{self, ApplyOutcome, OtherBranch};
use crate::backend::upstream::{self, ACTIVITY_DAYS, TargetCommit};
use crate::backend::{self, CommitId, DiffSource, ReviewState};
use crate::edits::CherryPick;
use crate::overlays::MenuItem;
use crate::state::{BranchStatus, Cut, Overlay, Picks, ProjectId, Selection};
use crate::theme::Theme;
use crate::transfer::drag_preview;
use crate::ui::illustration::Illustration;
use crate::ui::person::{author_avatar, byline, byline_avatar};
use crate::ui::text_input::TextInput;
use crate::ui::{
    ButtonStyle, Icon, ROW_HEIGHT, Tooltip, age_column, button, empty_state, faint, fold_button,
    guide, h_flex, icon, list_row, plural, quiet, review, section_label, side_row, small_icon,
    v_flex,
};

pub(super) struct Branches {
    project: ProjectId,
    /// The project's branches when the list loaded; a reload that changes none of them
    /// leaves the list as it is. `None` until it has loaded.
    seen: u64,
    list: Vec<OtherBranch>,
    /// The selected branch, by ref, whose diff shows; none while the overview does.
    selected: Option<String>,
    /// The selection is the caption of `selected`'s stack, which names its top branch: that
    /// one's changes are the whole stack's.
    stack: bool,
    /// The branches whose commits show under them, by ref: folded until opened, as the
    /// navigator's are.
    open: HashSet<String>,
    /// The stacks whose branches are folded away under their caption, by their top branch.
    folded: HashSet<String>,
    /// Open branches' commits, by ref and what they were loaded for.
    commits: HashMap<String, (Loaded, Vec<TargetCommit>)>,
    /// One of the selected branch's commits, to see on its own and to copy into the workspace.
    commit: Option<CommitId>,
    /// Commits of the selected branch picked along with the selected one (⌘- or ⇧-click), to
    /// cherry-pick together.
    picked: Picks<CommitId>,
    /// The one person whose branches are listed, by name, as `OtherBranch::people` has it.
    who: Option<String>,
    /// The days picked off the activity, which the list keeps the branches last changed on.
    days: DayPick,
}

impl Branches {
    /// Branch `ref_name`, while it's listed.
    fn branch(&self, ref_name: &str) -> Option<&OtherBranch> {
        self.list.iter().find(|b| b.ref_name == ref_name)
    }

    fn selected_branch(&self) -> Option<&OtherBranch> {
        self.branch(self.selected.as_ref()?)
    }

    /// The commits listed under branch `ref_name`: its own, not those of the branches below it
    /// in a stack.
    fn listed_commits(&self, ref_name: &str) -> &[TargetCommit] {
        let own = self.branch(ref_name).and_then(|b| b.own_commits);
        match self.commits.get(ref_name) {
            Some((_, commits)) => &commits[..own.unwrap_or(commits.len()).min(commits.len())],
            None => &[],
        }
    }

    /// What cherry-picking commit `id` takes: the picks, in the order they're listed, when
    /// it's one of them, or else just it.
    fn cherry_picks(&self, id: CommitId) -> Arc<[CommitId]> {
        let Some(ref_name) = self
            .selected
            .as_deref()
            .filter(|_| self.picked.contains(&id))
        else {
            return [id].into();
        };
        (self.listed_commits(ref_name).iter())
            .map(|commit| commit.id)
            .filter(|id| self.picked.contains(id))
            .collect()
    }
}

/// A branch's tip and how many commits it has that the target doesn't: a fetch can land
/// some of its commits on the target without moving its tip.
type Loaded = (CommitId, Option<usize>);

/// Where the middle of a stack's chevron sits in its row, and an unstacked branch's: the
/// row's left padding and half the chevron's step.
const STACK_CHEVRON: f32 = 2. + 7.;
/// Where a stacked branch's sits.
const STACKED_CHEVRON: f32 = 16. + 7.;

/// How many people the filter menu offers, the most active first.
const PEOPLE_IN_MENU: usize = 10;

/// How many people the overview lists, the most active first.
const PEOPLE_IN_OVERVIEW: usize = 8;

/// The list's filters, besides the text: whose branches, by their pull request, and from
/// which days; one left out counts what each of its choices would show.
#[derive(Clone, Copy, PartialEq)]
enum Facet {
    Who,
    Status,
    Days,
}

/// The pull request choices, as the filter menu and the chip name them. Not only merged ones:
/// the page is for what's under way, which the merged would read as a list of.
const STATUSES: [(BranchStatus, &str); 6] = [
    (BranchStatus::NotMerged, "Not merged"),
    (BranchStatus::Open, "Open"),
    (BranchStatus::Draft, "Draft"),
    (BranchStatus::Closed, "Closed"),
    (BranchStatus::NoPullRequest, "No pull request"),
    (BranchStatus::Any, "Any"),
];

/// A row of the list: a heading over the branches changed today, this week, in the last 30
/// days or before; a stack's caption, by its top branch's index, with how many of its
/// branches show and whether they're folded away; a branch by index into the list; or a
/// commit of an open one, by the branch's index and its own. A branch in a stack shown with
/// others, and its commits, sit a step in (`stacked`).
enum Row {
    Heading(&'static str),
    Stack(usize, usize, bool),
    Branch(usize, bool),
    Commit(usize, usize, bool),
}

impl Detail {
    /// List `project`'s other branches; unless `force`, only when its workspace changed.
    pub(super) fn load_branches(
        &mut self,
        project: &ProjectId,
        force: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.store.read(cx).path(project) else {
            return;
        };
        let seen = self.branches_fingerprint(project, cx);
        if !force && self.branches_for(project).is_some_and(|b| b.seen == seen) {
            return;
        }
        let project = project.clone();
        self._branches_load = cx.spawn(async move |this, cx| {
            let result = backend::run(move || branches::other_branches(&path)).await;
            this.update(cx, |this, cx| {
                let list = match result {
                    Ok(list) => list,
                    // What's listed stays, to be read again on the next change.
                    Err(err) => {
                        return this.store.update(cx, |store, cx| {
                            store.error("Couldn't list the branches", format!("{err:#}"), cx)
                        });
                    }
                };
                let previous = this.branches.take().filter(|b| b.project == project);
                // The branch opened, while it's listed: once it goes, as an agent renamed or
                // deleted it, the overview shows rather than another branch in its place.
                let selected = (previous.as_ref())
                    .and_then(|b| b.selected.clone())
                    .filter(|ref_name| list.iter().any(|b| b.ref_name == *ref_name));
                // Which branches were open and their commits, of those still listed, and the
                // selected commit while its branch stays selected.
                let listed = |ref_name: &String| list.iter().any(|b| b.ref_name == *ref_name);
                let (who, days) = (previous.as_ref())
                    .map(|b| (b.who.clone(), b.days.clone()))
                    .unwrap_or_default();
                let (open, folded, commits, commit, picked, stack) = match previous {
                    Some(b) => {
                        let same = b.selected == selected;
                        (
                            b.open.into_iter().filter(|r| listed(r)).collect(),
                            b.folded.into_iter().filter(|r| listed(r)).collect(),
                            b.commits.into_iter().filter(|(r, _)| listed(r)).collect(),
                            b.commit.filter(|_| same),
                            if same { b.picked } else { Picks::default() },
                            b.stack && same,
                        )
                    }
                    None => Default::default(),
                };
                this.branches = Some(Branches {
                    project,
                    seen,
                    list,
                    selected,
                    stack,
                    open,
                    folded,
                    commits,
                    commit,
                    picked,
                    who,
                    days,
                });
                // The filter may hide what's selected now.
                this.select_shown_branch(cx);
                this.load_open_commits(cx);
                this.refresh_diff(false, cx);
                cx.notify();
            })
            .ok();
        });
    }

    /// What a branch listing depends on: the project's branches, as its last full load saw
    /// them.
    fn branches_fingerprint(&self, project: &ProjectId, cx: &Context<Self>) -> u64 {
        self.store
            .read(cx)
            .workspace(project)
            .map_or(0, |ws| ws.refs)
    }

    fn branches_for(&self, project: &ProjectId) -> Option<&Branches> {
        self.branches.as_ref().filter(|b| b.project == *project)
    }

    /// The branch whose diff shows; none while the overview does.
    pub(crate) fn selected_other_branch(&self, project: &ProjectId) -> Option<&OtherBranch> {
        self.branches_for(project)?.selected_branch()
    }

    /// Whether the Branches page shows its overview beside the list, with no branch opened.
    pub(crate) fn showing_branches_overview(&self, cx: &App) -> bool {
        match &self.store.read(cx).selection {
            Some(Selection::Branches(project)) => self
                .branches_for(project)
                .is_none_or(|b| b.selected.is_none()),
            _ => false,
        }
    }

    /// Go back to the overview, letting go of the branch opened.
    pub(super) fn close_branch(&mut self, cx: &mut Context<Self>) {
        self.select_other_branch(None, cx)
    }

    /// Esc on the Branches page with a branch opened: back to the overview, whether it did. The
    /// app, which Esc is handled in, redraws for the diff that goes.
    pub fn close_opened_branch(&mut self, cx: &mut Context<Self>) -> bool {
        let opened = matches!(&self.store.read(cx).selection,
            Some(Selection::Branches(project)) if self.selected_other_branch(project).is_some());
        if opened {
            self.close_branch(cx);
        }
        opened
    }

    /// The commit picked of the opened branch, which shows its message over the tabs.
    pub(super) fn picked_branch_commit(&self, project: &ProjectId) -> Option<&TargetCommit> {
        let branch = self.selected_other_branch(project)?;
        let branches = self.branches_for(project)?;
        let id = branches.commit?;
        let (_, commits) = branches.commits.get(&branch.ref_name)?;
        commits.iter().find(|c| c.id == id)
    }

    /// Branch `ref_name` of `project`'s list, while it's still listed: what a menu opened on
    /// it acts on, even once a reload has selected another.
    fn listed_branch(&self, project: &ProjectId, ref_name: &str) -> Option<&OtherBranch> {
        self.branches_for(project)?.branch(ref_name)
    }

    /// None while the overview shows.
    pub(super) fn branches_source(&self, project: &ProjectId) -> Option<DiffSource> {
        let branch = self.selected_other_branch(project)?;
        if let Some(id) = self.selected_branch_commit(project) {
            return Some(DiffSource::Commit(id));
        }
        Some(DiffSource::Branch {
            tip: branch.tip,
            ahead: branch.ahead,
        })
    }

    fn selected_branch_commit(&self, project: &ProjectId) -> Option<CommitId> {
        self.branches_for(project)?.commit
    }

    /// Load the open branches' commits that aren't loaded for them as they are now. A selected
    /// commit its branch no longer has is let go.
    fn load_open_commits(&mut self, cx: &mut Context<Self>) {
        let Some(branches) = &self.branches else {
            return;
        };
        let wanted: Vec<(String, Loaded)> = (branches.list.iter())
            .filter(|b| branches.open.contains(&b.ref_name))
            .map(|b| (b.ref_name.clone(), (b.tip, b.ahead)))
            .filter(|(r, key)| (branches.commits.get(r)).is_none_or(|(loaded, _)| loaded != key))
            .collect();
        let Some(path) = self
            .store
            .read(cx)
            .path(&branches.project)
            .filter(|_| !wanted.is_empty())
        else {
            return;
        };
        self._branch_commits_load = cx.spawn(async move |this, cx| {
            let loaded = backend::run(move || {
                Ok(wanted
                    .into_iter()
                    .map(|(r, key)| {
                        let commits = branches::branch_commits(&path, &r);
                        (r, key, commits)
                    })
                    .collect::<Vec<_>>())
            })
            .await;
            this.update(cx, |this, cx| {
                let (Ok(loaded), Some(branches)) = (loaded, this.branches.as_mut()) else {
                    return;
                };
                for (r, key, commits) in loaded {
                    // Unless the branch moved meanwhile. One that failed shows none rather than
                    // loading for good, and is read again once it moves.
                    let current =
                        (branches.list.iter()).any(|b| b.ref_name == r && (b.tip, b.ahead) == key);
                    let commits = commits.unwrap_or_else(|err| {
                        tracing::warn!("couldn't list the branch's commits: {err:#}");
                        Vec::new()
                    });
                    if current {
                        branches.commits.insert(r, (key, commits));
                    }
                }
                let gone = (branches.commit.zip(branches.selected.as_ref()))
                    .and_then(|(id, r)| Some((id, &branches.commits.get(r)?.1)))
                    .is_some_and(|(id, commits)| commits.iter().all(|c| c.id != id));
                if gone {
                    branches.commit = None;
                    branches.picked.clear();
                    this.refresh_diff(true, cx);
                }
                cx.notify();
            })
            .ok();
        });
    }

    /// Fold a stack's branches away under its caption, or show them again, by its top branch.
    /// Folding it over its selected branch or commit selects the caption.
    fn toggle_stack(&mut self, top: &str, cx: &mut Context<Self>) {
        let Some(branches) = &mut self.branches else {
            return;
        };
        if branches.folded.remove(top) {
            return cx.notify();
        }
        branches.folded.insert(top.to_owned());
        let stack = branches.branch(top).map(|b| b.stack);
        let inside = (branches.selected_branch()).is_some_and(|b| Some(b.stack) == stack);
        if inside && !branches.stack {
            self.select_stack(top.to_owned(), cx);
        }
        cx.notify();
    }

    /// Show a branch's commits under it, or fold them away, as a navigator branch does.
    /// Folding it over its selected commit selects the branch.
    fn toggle_branch(&mut self, ref_name: &str, cx: &mut Context<Self>) {
        let Some(branches) = &mut self.branches else {
            return;
        };
        if branches.open.insert(ref_name.to_owned()) {
            self.load_open_commits(cx);
        } else {
            branches.open.remove(ref_name);
            if branches.selected.as_deref() == Some(ref_name) && branches.commit.take().is_some() {
                branches.picked.clear();
                self.show_picked(cx);
            }
        }
        cx.notify();
    }

    /// The branches the filters keep, newest first, under headings by when they last
    /// changed, a stack's branches together under a caption while more than one of them shows.
    fn branch_rows(&self, branches: &Branches, cx: &Context<Self>) -> Vec<Row> {
        let shown = self.shown_branches(branches, cx);
        // A stack goes by its newest branch, as the listing orders them.
        let mut newest: HashMap<usize, i64> = HashMap::new();
        for branch in &branches.list {
            if let Some(time) = branch.time {
                let at = newest.entry(branch.stack).or_insert(time);
                *at = (*at).max(time);
            }
        }
        let today = upstream::day_of(crate::ui::now());
        let mut rows = Vec::new();
        let mut group = None;
        for stack in shown.chunk_by(|a, b| branches.list[*a].stack == branches.list[*b].stack) {
            // A day ahead, from a machine whose clock runs fast, counts as today, as the
            // activity strip has it.
            let ago = (newest.get(&branches.list[stack[0]].stack))
                .map(|time| (today - upstream::day_of(*time)).max(0));
            let heading = match ago {
                Some(0) => "Today",
                Some(days) if days < 7 => "This week",
                Some(days) if days < ACTIVITY_DAYS as i64 => "Last 30 days",
                _ => "Older",
            };
            if group != Some(heading) {
                rows.push(Row::Heading(heading));
                group = Some(heading);
            }
            let stacked = stack.len() > 1;
            if stacked {
                let top = (branches.list.iter())
                    .position(|b| b.stack == branches.list[stack[0]].stack)
                    .unwrap_or(stack[0]);
                let folded = branches.folded.contains(&branches.list[top].ref_name);
                rows.push(Row::Stack(top, stack.len(), folded));
                if folded {
                    continue;
                }
            }
            for &ix in stack {
                rows.push(Row::Branch(ix, stacked));
                let branch = &branches.list[ix];
                // By its tip only, so the rows stay while a fetch has them reload. Only its
                // own commits: a stacked branch's listing goes on through the ones below it.
                if branches.open.contains(&branch.ref_name)
                    && let Some(((tip, _), commits)) = branches.commits.get(&branch.ref_name)
                    && *tip == branch.tip
                {
                    let own = branch
                        .own_commits
                        .unwrap_or(commits.len())
                        .min(commits.len());
                    rows.extend((0..own).map(|commit| Row::Commit(ix, commit, stacked)));
                }
            }
        }
        rows
    }

    /// The branches the filters keep, by index into the list, folded away or not.
    fn shown_branches(&self, branches: &Branches, cx: &App) -> Vec<usize> {
        self.kept_branches(branches, None, cx)
    }

    /// The branches the filters keep but `skip`, which is left out to count its choices.
    fn kept_branches(&self, branches: &Branches, skip: Option<Facet>, cx: &App) -> Vec<usize> {
        let query = self.branch_filter.read(cx).text().trim().to_lowercase();
        let store = self.store.read(cx);
        let prefs = &store.prefs;
        let today = upstream::day_of(crate::ui::now());
        let says = |text: &str| text.to_lowercase().contains(&query);
        let keeps = |branch: &OtherBranch| {
            let named = !query.is_empty() && says(&branch.name);
            let found = query.is_empty()
                || named
                || says(title(branch))
                || branch.description.as_deref().is_some_and(says)
                || branch.people.iter().any(|person| says(person));
            // One being deleted leaves at once, and one with no commits of its own shows only
            // when its name is searched for.
            !store.hides_branch(&branches.project, &branch.ref_name)
                && (branch.own_commits != Some(0) || named)
                && (branch.local || !prefs.branches_local_only)
                && found
                && (skip == Some(Facet::Status) || status_keeps(prefs.branches_status, branch))
                && (skip == Some(Facet::Who)
                    || (branches.who.as_ref()).is_none_or(|who| branch.people.contains(who)))
                && (skip == Some(Facet::Days) || branches.days.keeps(branch.time, today))
        };
        (0..branches.list.len())
            .filter(|ix| keeps(&branches.list[*ix]))
            .collect()
    }

    /// The people at work lately: whose branches the filters keep but the person, of those
    /// changed in the activity's days, most branches first, with how many and an email for
    /// their picture. Not everyone who ever made one, which would put those long gone first
    /// and leave out the newly arrived.
    fn branch_people(&self, branches: &Branches, cx: &App) -> Vec<(String, String, usize)> {
        let first_day = upstream::day_of(crate::ui::now()) - (ACTIVITY_DAYS as i64 - 1);
        let mut people: Vec<(String, String, usize)> = Vec::new();
        for ix in self.kept_branches(branches, Some(Facet::Who), cx) {
            let branch = &branches.list[ix];
            if branch
                .time
                .is_none_or(|time| upstream::day_of(time) < first_day)
            {
                continue;
            }
            for person in &branch.people {
                match people.iter_mut().find(|(name, ..)| name == person) {
                    Some((.., n)) => *n += 1,
                    None => people.push((person.clone(), String::new(), 1)),
                }
            }
        }
        // Their email from any branch they made the newest commit of, filtered or not.
        for (name, email, _) in &mut people {
            *email = (branches.list.iter())
                .find(|b| b.author.as_ref() == Some(name))
                .and_then(|b| b.email.clone())
                .unwrap_or_default();
        }
        people.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
        people
    }

    /// How many branches the filters keep but the pull request one, for each choice of it.
    fn status_counts(&self, branches: &Branches, cx: &App) -> Vec<(BranchStatus, usize)> {
        let kept = self.kept_branches(branches, Some(Facet::Status), cx);
        (STATUSES.iter())
            .map(|(status, _)| {
                let n = (kept.iter())
                    .filter(|ix| status_keeps(*status, &branches.list[**ix]))
                    .count();
                (*status, n)
            })
            .collect()
    }

    /// List only `who`'s branches, or everyone's.
    pub(crate) fn filter_person(&mut self, who: Option<String>, cx: &mut Context<Self>) {
        if let Some(branches) = &mut self.branches {
            branches.who = who;
        }
        self.select_shown_branch(cx);
        cx.notify();
    }

    /// List the branches by their pull request, as `status` picks.
    pub(crate) fn filter_status(&mut self, status: BranchStatus, cx: &mut Context<Self>) {
        (self.store).update(cx, |store, cx| {
            store.set_prefs(|prefs| prefs.branches_status = status, cx)
        });
        self.select_shown_branch(cx);
        cx.notify();
    }

    /// Pick the activity's days as `gesture` does, which the list keeps the branches of.
    fn branch_day_gesture(&mut self, gesture: DayGesture, cx: &mut Context<Self>) {
        let Some(branches) = &mut self.branches else {
            return;
        };
        if branches.days.apply(gesture) {
            cx.notify();
        }
        // Once the pointer lets go, so a drag passing over other days doesn't close the
        // branch partway.
        if let DayGesture::Release = gesture {
            self.select_shown_branch(cx);
        }
    }

    fn clear_branch_days(&mut self, cx: &mut Context<Self>) {
        if let Some(branches) = &mut self.branches {
            branches.days = DayPick::default();
        }
        cx.notify();
    }

    /// Let go of the opened branch once the filters hide it, so the overview shows rather
    /// than the diff of one the list doesn't.
    pub(super) fn select_shown_branch(&mut self, cx: &mut Context<Self>) {
        let Some(branches) = &self.branches else {
            return;
        };
        // A caption stays while it's listed, though the filter hides its top branch.
        let rows = self.branch_rows(branches, cx);
        if branches.stack && rows.iter().any(|row| is_selected(branches, row)) {
            return;
        }
        // One folded away in its stack still counts.
        let selected =
            |ix: &usize| Some(&branches.list[*ix].ref_name) == branches.selected.as_ref();
        if self.shown_branches(branches, cx).iter().any(selected) {
            // A stack the filter leaves one branch of has no caption to stay on.
            if branches.stack {
                self.select_other_branch(branches.selected.clone(), cx);
            }
        } else if branches.selected.is_some() {
            self.select_other_branch(None, cx);
        }
    }

    pub(super) fn select_other_branch(&mut self, ref_name: Option<String>, cx: &mut Context<Self>) {
        self.select_listed(ref_name, false, cx)
    }

    /// Select the caption of the stack whose top branch is `top`.
    fn select_stack(&mut self, top: String, cx: &mut Context<Self>) {
        self.select_listed(Some(top), true, cx)
    }

    fn select_listed(&mut self, ref_name: Option<String>, stack: bool, cx: &mut Context<Self>) {
        self.message_open = false;
        if let Some(branches) = &mut self.branches {
            branches.selected = ref_name;
            branches.stack = stack;
            branches.commit = None;
            branches.picked.clear();
        }
        self.load_open_commits(cx);
        self.show_picked(cx);
    }

    /// Select commit `id` of the branch `ref_name`, which it's listed under, alone.
    fn select_branch_commit(&mut self, ref_name: &str, id: CommitId, cx: &mut Context<Self>) {
        self.message_open = false;
        if let Some(branches) = &mut self.branches {
            branches.selected = Some(ref_name.to_owned());
            branches.stack = false;
            branches.commit = Some(id);
            branches.picked.clear();
        }
        self.show_picked(cx);
    }

    /// ⌘-click picks commit `id` along with the selected one, or drops it; ⇧-click (`run`)
    /// picks the commits between them, as the navigator does. On another branch than the
    /// selected commit's, it selects just it.
    fn pick_branch_commit(
        &mut self,
        ref_name: &str,
        id: CommitId,
        run: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(branches) = &mut self.branches else {
            return;
        };
        let Some(at) = branches
            .commit
            .filter(|_| branches.selected.as_deref() == Some(ref_name))
        else {
            return self.select_branch_commit(ref_name, id, cx);
        };
        let next = match run {
            true => {
                let order: Vec<CommitId> = (branches.listed_commits(ref_name).iter())
                    .map(|c| c.id)
                    .collect();
                branches.picked.run(&order, &at, &id)
            }
            false => Some(branches.picked.toggle(&at, &id)),
        };
        if let Some(next) = next {
            branches.commit = Some(next);
        }
        self.show_picked(cx);
    }

    /// ↓ or ↑ (`delta`) in the list: the next or previous stack, branch or commit shown, kept
    /// in view.
    fn step_branches(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(branches) = &self.branches else {
            return;
        };
        let rows = self.branch_rows(branches, cx);
        // A selected branch folded away in its stack steps from the stack's row.
        let in_stack = |top: usize| {
            (branches.selected_branch()).is_some_and(|b| b.stack == branches.list[top].stack)
        };
        let at = rows.iter().position(|row| {
            is_selected(branches, row) || matches!(*row, Row::Stack(top, _, true) if in_stack(top))
        });
        // Over the headings, which aren't selected.
        let selectable = |ix: &usize| !matches!(rows[*ix], Row::Heading(_));
        let next = match at {
            Some(at) => (1..rows.len() as isize)
                .map(|step| at as isize + delta * step)
                .take_while(|ix| (0..rows.len() as isize).contains(ix))
                .map(|ix| ix as usize)
                .find(selectable),
            None => (0..rows.len()).find(selectable),
        };
        if let Some(next) = next {
            self.select_branch_row(&rows[next], cx);
            (self.branches_scroll).scroll_to_item(next, gpui::ScrollStrategy::Nearest);
        }
    }

    /// → in the list: open the selected stack or branch, or step into it when it's open.
    /// ← folds it, or steps from one of its commits or branches back up to it.
    fn fold_branches(&mut self, open: bool, cx: &mut Context<Self>) {
        let Some(branches) = &self.branches else {
            return;
        };
        let Some(branch) = branches.selected_branch() else {
            return;
        };
        let ref_name = branch.ref_name.clone();
        if branches.stack {
            let folded = branches.folded.contains(&ref_name);
            return match (open, folded) {
                (true, true) | (false, false) => self.toggle_stack(&ref_name, cx),
                (true, false) => self.step_branches(1, cx),
                (false, true) => {}
            };
        }
        let is_open = branches.open.contains(&ref_name);
        match (open, branches.commit.is_some(), is_open) {
            (true, false, false) if branch.own_commits != Some(0) => {
                self.toggle_branch(&ref_name, cx)
            }
            (true, false, true) => self.step_branches(1, cx),
            (false, true, _) => self.select_other_branch(Some(ref_name), cx),
            (false, false, true) => self.toggle_branch(&ref_name, cx),
            // A branch in a stack shown with others goes up to its caption.
            (false, false, false) => {
                let rows = self.branch_rows(branches, cx);
                let at = rows.iter().position(|row| is_selected(branches, row));
                let caption =
                    (at.filter(|at| matches!(rows[*at], Row::Branch(_, true)))).and_then(|at| {
                        rows[..at].iter().rev().find_map(|row| match row {
                            Row::Stack(top, ..) => Some(branches.list[*top].ref_name.clone()),
                            _ => None,
                        })
                    });
                if let Some(top) = caption {
                    self.select_stack(top, cx);
                }
            }
            _ => {}
        }
    }

    fn select_branch_row(&mut self, row: &Row, cx: &mut Context<Self>) {
        let Some(branches) = &self.branches else {
            return;
        };
        match *row {
            Row::Heading(_) => {}
            Row::Stack(top, ..) => {
                let top = branches.list[top].ref_name.clone();
                self.select_stack(top, cx)
            }
            Row::Branch(ix, _) => {
                let ref_name = branches.list[ix].ref_name.clone();
                self.select_other_branch(Some(ref_name), cx)
            }
            Row::Commit(ix, commit, _) => {
                let ref_name = branches.list[ix].ref_name.clone();
                let Some((_, commits)) = branches.commits.get(&ref_name) else {
                    return;
                };
                let id = commits[commit].id;
                self.select_branch_commit(&ref_name, id, cx)
            }
        }
    }

    /// Select a branch and offer what its header does, and its pull request and name.
    fn branch_menu(
        &mut self,
        project: &ProjectId,
        ref_name: &str,
        at: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.select_other_branch(Some(ref_name.to_owned()), cx);
        let items = self.branch_menu_items(project, ref_name, cx);
        if !items.is_empty() {
            self.app
                .update(cx, |app, cx| app.open_menu(at, items, cx))
                .ok();
        }
    }

    /// What the Branches page offers for its branch `ref_name`, as right-clicking it does.
    pub(crate) fn branch_menu_items(
        &self,
        project: &ProjectId,
        ref_name: &str,
        cx: &App,
    ) -> Vec<MenuItem> {
        // By its name, as the table has none opened.
        let Some(branch) = self.listed_branch(project, ref_name) else {
            return Vec::new();
        };
        let mut items = Vec::new();
        let worktree = self.checked_out_in(project, branch, cx);
        let (p, r) = (project.clone(), ref_name.to_owned());
        items.push(match worktree.clone() {
            Some(name) => MenuItem::new("Open worktree", move |app, _, cx| {
                let shown = Selection::Worktree(p.clone(), name.clone());
                app.store
                    .update(cx, |store, cx| store.select(Some(shown), cx))
            }),
            None => MenuItem::new("Apply", move |app, _, cx| {
                app.detail
                    .update(cx, |detail, cx| detail.apply_branch(&p, &r, cx))
            }),
        });
        if let Some(url) = (branch.review.as_ref()).and_then(|r| r.url.clone()) {
            items.push(MenuItem::new("Open pull request", move |_, _, cx| {
                cx.open_url(&url)
            }));
        }
        let name = branch.name.clone();
        items.push(
            MenuItem::new("Copy name", move |app, _, cx| {
                app.copy_text(name.clone(), cx)
            })
            .separated(),
        );
        if branch.local {
            let (p, r) = (project.clone(), ref_name.to_owned());
            // Once the menu's done with the app, which the prompt updates too.
            items.push(MenuItem::new(
                "Edit description…",
                move |app, window, cx| {
                    let (detail, p, r) = (app.detail.clone(), p.clone(), r.clone());
                    window.defer(cx, move |window, cx| {
                        detail.update(cx, |detail, cx| {
                            detail.edit_branch_description(&p, &r, window, cx)
                        })
                    })
                },
            ));
        }
        if branch.local && worktree.is_none() {
            let (p, r) = (project.clone(), ref_name.to_owned());
            items.push(
                // Once the menu's done with the app, which asking to confirm updates too.
                MenuItem::new("Delete branch…", move |app, window, cx| {
                    let (detail, p, r) = (app.detail.clone(), p.clone(), r.clone());
                    window.defer(cx, move |window, cx| {
                        detail.update(cx, |detail, cx| {
                            detail.delete_other_branch(&p, &r, window, cx)
                        })
                    })
                })
                .danger()
                .separated(),
            );
        }
        items
    }

    /// The linked worktree that has `branch` checked out, if one does: applying or deleting it
    /// would pull it out from under that worktree.
    fn checked_out_in(
        &self,
        project: &ProjectId,
        branch: &OtherBranch,
        cx: &App,
    ) -> Option<Arc<str>> {
        let ws = self
            .store
            .read(cx)
            .workspace(project)
            .filter(|_| branch.local)?;
        (ws.worktrees.iter())
            .find(|w| w.branch.as_deref() == Some(branch.name.as_str()))
            .map(|worktree| worktree.name.clone())
    }

    /// Select commit `id` of `ref_name`, keeping the picks when it's one of them, and offer
    /// to copy it, or them, into the workspace, or its ID.
    fn branch_commit_menu(
        &mut self,
        project: &ProjectId,
        ref_name: &str,
        id: CommitId,
        at: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        match &mut self.branches {
            Some(branches) if branches.picked.contains(&id) => {
                branches.commit = Some(id);
                self.show_picked(cx);
            }
            _ => self.select_branch_commit(ref_name, id, cx),
        }
        if let Some(ids) = self.branches.as_ref().map(|b| b.cherry_picks(id)) {
            self.commit_menu(project, ids, id, at, cx);
        }
    }

    /// Pick up the selected commit, with the others picked, to put copies of them where ⌘V
    /// lands in the workspace.
    fn copy_branch_commit(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        let Some((branches, id)) =
            (self.branches_for(project)).and_then(|branches| Some((branches, branches.commit?)))
        else {
            return;
        };
        let cut = Cut::CherryPick(project.clone(), branches.cherry_picks(id));
        self.app.update(cx, |app, cx| app.cut(cut, cx)).ok();
    }

    fn apply_branch(&mut self, project: &ProjectId, ref_name: &str, cx: &mut Context<Self>) {
        let Some(branch) = self.listed_branch(project, ref_name) else {
            return;
        };
        let (ref_name, name) = (branch.ref_name.clone(), branch.name.clone());
        let selected = project.clone();
        self.store.update(cx, |store, cx| {
            store.run(
                project,
                format!("Apply {name}"),
                Overlay::default(),
                move |path, _| branches::apply(path, &ref_name),
                move |store, applied, cx| match applied {
                    ApplyOutcome::Applied | ApplyOutcome::AlreadyApplied => {
                        let local = format!("refs/heads/{name}");
                        store.select(Some(Selection::Branch(selected, local)), cx);
                        // As when an agent applied it meanwhile.
                        if matches!(applied, ApplyOutcome::AlreadyApplied) {
                            return store.snack(format!("{name} is already applied"), cx);
                        }
                        store.snack_undoable(format!("Applied {name}"), cx)
                    }
                    ApplyOutcome::Conflicts(stacks) => store.error(
                        format!("{name} wasn't applied"),
                        format!("It conflicts with {}", stacks.join(", ")),
                        cx,
                    ),
                },
                cx,
            )
        });
    }

    fn delete_other_branch(
        &mut self,
        project: &ProjectId,
        ref_name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(branch) = self.listed_branch(project, ref_name).filter(|b| b.local) else {
            return;
        };
        let ref_name = format!("refs/heads/{}", branch.name);
        let project = project.clone();
        let name = branch.name.clone();
        let detail = cx.entity().downgrade();
        self.app
            .update(cx, |app, cx| {
                app.confirm_then(
                    Some(Selection::Branches(project.clone())),
                    format!("Delete branch {name}?"),
                    "Its commits go too, unless a remote has them. Undo can't bring back a branch \
                     outside the workspace.",
                    "Delete",
                    window,
                    cx,
                    move |app, cx| {
                        app.store.update(cx, |store, cx| {
                            let overlay = Overlay {
                                branches: HashSet::from([ref_name.clone()]),
                                ..Overlay::default()
                            };
                            let (gone, listed_in) = (ref_name.clone(), project.clone());
                            store.run(
                                &project,
                                "Delete branch",
                                overlay,
                                move |path, _| backend::remove_branch(path, &ref_name),
                                // Out of the list now: reading the branches again, as their
                                // refs changed, lands after the overlay is gone.
                                move |store, (), cx| {
                                    (detail.update(cx, |detail, _| {
                                        let branches = (detail.branches.as_mut())
                                            .filter(|b| b.project == listed_in);
                                        if let Some(branches) = branches {
                                            branches.list.retain(|b| b.ref_name != gone);
                                        }
                                    }))
                                    .ok();
                                    // The overview, rather than its diff, once it's gone: after
                                    // this update of the store, which the list reads.
                                    let detail = detail.clone();
                                    cx.defer(move |cx| {
                                        (detail.update(cx, |detail, cx| {
                                            detail.select_shown_branch(cx)
                                        }))
                                        .ok();
                                    });
                                    // The oplog can't bring it back, so ⌘Z won't try.
                                    store.keep_last_snapshot();
                                    store.snack("Branch deleted", cx)
                                },
                                cx,
                            )
                        })
                    },
                )
            })
            .ok();
    }

    /// Ask for local branch `ref_name`'s description and keep it where Git does.
    fn edit_branch_description(
        &mut self,
        project: &ProjectId,
        ref_name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(branch) = self.listed_branch(project, ref_name).filter(|b| b.local) else {
            return;
        };
        let title = format!("Describe {}", branch.name);
        let current = branch.description.clone().unwrap_or_default();
        let input = cx.new(|cx| {
            let mut input = TextInput::new("What this branch is for", true, cx);
            input.load(current, cx);
            input.select_all_text(cx);
            input
        });
        let (project, ref_name) = (project.clone(), ref_name.to_owned());
        let (detail, app) = (cx.entity().downgrade(), self.app.clone());
        // Opened from a click, inside this view's update.
        window.defer(cx, move |window, cx| {
            app.update(cx, |app, cx| {
                let place = app
                    .store
                    .read(cx)
                    .place(&Selection::Project(project.clone()));
                app.open_prompt(
                    Some(place),
                    &title,
                    Some(
                        "Git keeps it as the branch's description, where git branch \
                         --edit-description does. Leave it empty to remove it. ⌘↩ saves it."
                            .to_owned(),
                    ),
                    "Save",
                    input,
                    None,
                    None,
                    window,
                    cx,
                    move |app, text, _, cx| {
                        let text = text.trim().to_owned();
                        let (saved, gone, listed_in) =
                            (text.clone(), ref_name.clone(), project.clone());
                        app.store.update(cx, |store, cx| {
                            store.run(
                                &project,
                                "Describe branch",
                                Overlay::default(),
                                move |path, _| {
                                    branches::set_branch_description(path, &ref_name, &text)
                                },
                                // Its refs didn't change, so the list isn't read again.
                                move |_, (), cx| {
                                    (detail.update(cx, |detail, cx| {
                                        let listed = (detail.branches.as_mut())
                                            .filter(|b| b.project == listed_in)
                                            .and_then(|b| {
                                                b.list.iter_mut().find(|b| b.ref_name == gone)
                                            });
                                        if let Some(branch) = listed {
                                            branch.description =
                                                (!saved.is_empty()).then_some(saved);
                                        }
                                        cx.notify();
                                    }))
                                    .ok();
                                },
                                cx,
                            )
                        })
                    },
                )
            })
            .ok();
        });
    }

    pub(super) fn render_branches_side(
        &self,
        project: &ProjectId,
        theme: &Theme,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(branches) = self.branches_for(project) else {
            return empty_state(None, "Loading branches…", "", theme, cx);
        };
        let filter = self.branch_filter_bar(project, window, cx);
        let chips = self.branch_filter_chips(branches, cx);
        let activity = self.branch_activity(branches, theme, cx);
        let rows = self.branch_rows(branches, cx);
        let target = (self.store.read(cx).workspace(project))
            .and_then(|ws| ws.target.as_ref())
            .map(|target| target.name.clone());
        let list = if rows.is_empty() {
            self.no_branches(branches, theme, cx)
        } else {
            side_list(
                "other-branches",
                rows.len(),
                &self.branches_scroll,
                "branches-scrollbar",
                cx,
                move |this, ix, theme, window, cx| {
                    let branches = this.branches.as_ref()?;
                    let solid = this.side_focus.is_focused(window);
                    Some(match rows.get(ix)? {
                        // On the rows under it, as a side list's group headings are.
                        Row::Heading(heading) => div()
                            .h(px(ROW_HEIGHT))
                            .px(px(14.))
                            .pb(px(2.))
                            .flex()
                            .items_end()
                            .child(section_label(*heading, theme))
                            .into_any_element(),
                        // As the navigator's caption, its branches a step in under it: a
                        // click selects it and its chevron folds them away; folded, it names
                        // its top branch.
                        row @ Row::Stack(top, count, folded) => {
                            let selected = is_selected(branches, row);
                            let on = selected && solid;
                            let ink = |color| if on { theme.text_inverted } else { color };
                            let top_ref = branches.list[*top].ref_name.clone();
                            let fold = top_ref.clone();
                            let step = fold_button(
                                ("stack-fold", ix),
                                Some(!*folded),
                                ink(theme.text_3),
                                cx,
                                move |this, _, cx| this.toggle_stack(&fold, cx),
                            );
                            side_row(("stack-caption", ix), selected, solid, theme)
                                .pl(px(2.))
                                .text_size(px(12.))
                                .text_color(ink(theme.text_2))
                                .child(step)
                                .child(small_icon(Icon::Stack).text_color(ink(theme.text_3)))
                                .child(div().flex_none().child(format!("Stack of {count}")))
                                .when(*folded, |el| {
                                    el.child(
                                        div()
                                            .min_w(px(0.))
                                            .truncate()
                                            .text_color(ink(theme.text_1))
                                            .child(title(&branches.list[*top]).to_owned()),
                                    )
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.select_stack(top_ref.clone(), cx)
                                }))
                                .into_any_element()
                        }
                        row @ Row::Commit(branch_ix, commit_ix, stacked) => {
                            let ref_name = branches.list[*branch_ix].ref_name.clone();
                            let (_, commits) = branches.commits.get(&ref_name)?;
                            let commit = commits.get(*commit_ix)?;
                            let id = commit.id;
                            let label: SharedString = commit.title.clone().into();
                            let project = branches.project.clone();
                            // A picked one takes the others picked along.
                            let ids = branches.cherry_picks(id);
                            let dragged: SharedString = match ids.len() {
                                1 => label.clone(),
                                n => format!("{n} commits").into(),
                            };
                            let payload = Cut::CherryPick(project.clone(), ids);
                            let selected = is_selected(branches, row) || is_picked(branches, row);
                            let joined = match selected && !branches.picked.is_empty() {
                                true => {
                                    let picked_at = |ix: Option<usize>| {
                                        (ix.and_then(|ix| rows.get(ix)))
                                            .is_some_and(|row| is_picked(branches, row))
                                    };
                                    (picked_at(ix.checked_sub(1)), picked_at(Some(ix + 1)))
                                }
                                false => (false, false),
                            };
                            let on = selected && solid;
                            let menu_ref = ref_name.clone();
                            side_row(("other-commit", ix), selected, solid, theme)
                                .map(|el| crate::ui::joined(el, joined, solid, theme))
                                // Its glyph under its branch's, its title under the name.
                                .pl(px(if *stacked { 38. } else { 24. }))
                                // Lines down from its stack's chevron and its branch's.
                                .children(stacked.then(|| guide(STACK_CHEVRON, theme)))
                                .child(guide(
                                    match stacked {
                                        true => STACKED_CHEVRON,
                                        false => STACK_CHEVRON,
                                    },
                                    theme,
                                ))
                                .aria_label(label.clone())
                                .child(small_icon(Icon::Commit).text_color(match on {
                                    true => theme.text_inverted,
                                    false => theme.text_3,
                                }))
                                .child(div().flex_1().min_w(px(0.)).truncate().child(label.clone()))
                                // Who wrote it and when, in the branch rows' columns, so the
                                // two line up.
                                .child(commit_author(
                                    ("other-commit-author", ix),
                                    commit,
                                    on,
                                    theme,
                                ))
                                .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                                    let held = event.modifiers();
                                    match held.secondary() || held.shift {
                                        true => {
                                            this.pick_branch_commit(&ref_name, id, held.shift, cx)
                                        }
                                        false => this.select_branch_commit(&ref_name, id, cx),
                                    }
                                }))
                                .on_mouse_down(
                                    MouseButton::Right,
                                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                        let at = event.position;
                                        this.branch_commit_menu(&project, &menu_ref, id, at, cx)
                                    }),
                                )
                                .on_drag(payload, drag_preview(this.store.clone(), dragged))
                                .into_any_element()
                        }
                        row @ Row::Branch(branch_ix, stacked) => {
                            let branch = &branches.list[*branch_ix];
                            let selected = is_selected(branches, row);
                            let ref_name = branch.ref_name.clone();
                            let (glyph, glyph_color) = match &branch.review {
                                Some(review) => review::state_glyph(review.state, theme),
                                None => (Icon::Branch, theme.text_2),
                            };
                            let on = selected && solid;
                            let ink = |color| if on { theme.text_inverted } else { color };
                            // As in the navigator: a chevron folds its commits away, without
                            // selecting it, and a branch with none to show has a dot there.
                            let open = (branch.own_commits != Some(0))
                                .then(|| branches.open.contains(&ref_name));
                            let fold = ref_name.clone();
                            let step = fold_button(
                                ("other-branch-fold", ix),
                                open,
                                ink(theme.text_3),
                                cx,
                                move |this, _, cx| this.toggle_branch(&fold, cx),
                            );
                            // As the navigator colours a pull request's title; fainter once
                            // the target has it.
                            let title_color = match &branch.review {
                                Some(review) => review::title_color(review.state, theme),
                                None if branch.ahead == Some(0) => theme.text_2,
                                None => theme.text_1,
                            };
                            let name = div()
                                .flex_1()
                                .min_w(px(0.))
                                .truncate()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(ink(title_color))
                                .child(title(branch).to_owned());
                            // The target has it all: said, as the navigator says it of a branch
                            // that landed.
                            let merged = (branch.ahead == Some(0)).then(|| {
                                format!("in {}", target.as_deref().unwrap_or("the target"))
                            });
                            let (project, menu_ref) = (branches.project.clone(), ref_name.clone());
                            side_row(("other-branch", ix), selected, solid, theme)
                                .pl(px(if *stacked { 16. } else { 2. }))
                                .children(stacked.then(|| guide(STACK_CHEVRON, theme)))
                                .child(step)
                                .child(small_icon(glyph).text_color(ink(glyph_color)))
                                .child(name)
                                .when_some(merged, |el, merged| {
                                    el.child(quiet(faint(merged, theme), on, theme))
                                })
                                // Who made its newest commit, to pick out a teammate's at a
                                // glance; a click lists only theirs.
                                .when_some(branch.author.clone(), |el, author| {
                                    let tip = match &branch.agent {
                                        Some(agent) => format!(
                                            "Last commit by {author}, written by {}. Click \
                                             to list only theirs.",
                                            agent.name
                                        ),
                                        None => format!(
                                            "Last commit by {author}. Click to list only theirs."
                                        ),
                                    };
                                    el.child(
                                        div()
                                            .id(("other-branch-author", ix))
                                            .flex_none()
                                            .child(byline_avatar(
                                                &author,
                                                branch.email.as_deref().unwrap_or_default(),
                                                branch.agent.as_ref(),
                                                14.,
                                                theme,
                                            ))
                                            .tooltip(Tooltip::text(tip))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                cx.stop_propagation();
                                                this.filter_person(Some(author.clone()), cx)
                                            })),
                                    )
                                })
                                // Its slot kept for an author without a time, so the
                                // avatars line up.
                                .when(branch.time.is_some() || branch.author.is_some(), |el| {
                                    el.child(quiet(age_column(branch.time, theme), on, theme))
                                })
                                // A click selects it; only the chevron, ← and → fold its commits,
                                // which are often not what's wanted.
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.select_other_branch(Some(ref_name.clone()), cx)
                                }))
                                .on_mouse_down(
                                    MouseButton::Right,
                                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                        this.branch_menu(&project, &menu_ref, event.position, cx)
                                    }),
                                )
                                .into_any_element()
                        }
                    })
                },
            )
            .flex_1()
            .min_h(px(0.))
            .into_any_element()
        };
        v_flex()
            .size_full()
            .track_focus(&self.side_focus)
            .key_context("SideList")
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.step_branches(1, cx)))
            .on_action(cx.listener(|this, _: &SelectPrevious, _, cx| this.step_branches(-1, cx)))
            .on_action(cx.listener(|this, _: &Expand, _, cx| this.fold_branches(true, cx)))
            .on_action(cx.listener(|this, _: &Collapse, _, cx| this.fold_branches(false, cx)))
            .when(self.selected_branch_commit(project).is_some(), |el| {
                let project = project.clone();
                el.on_action(cx.listener(move |this, _: &CherryPick, _, cx| {
                    this.copy_branch_commit(&project, cx)
                }))
            })
            .child(filter)
            .children(chips)
            .child(activity)
            .child(list)
            .into_any_element()
    }

    /// The filter bar over the list, with a fetch button.
    fn branch_filter_bar(
        &self,
        project: &ProjectId,
        window: &Window,
        cx: &Context<Self>,
    ) -> gpui::Div {
        let prefs = &self.store.read(cx).prefs;
        // Lit while the filters aren't as they are at rest; the days have their own Clear.
        let narrowed = (self.branches_for(project)).is_some_and(|b| b.who.is_some())
            || prefs.branches_status != BranchStatus::NotMerged
            || prefs.branches_local_only;
        self.filter_bar(
            &self.branch_filter,
            "Branch filters",
            Self::branch_filters_menu,
            narrowed,
            window,
            cx,
        )
        .children(self.fetch_button(project, cx))
    }

    /// The filters narrowing the list, a chip each that lets go of it; none at rest.
    fn branch_filter_chips(&self, branches: &Branches, cx: &Context<Self>) -> Option<gpui::Div> {
        let prefs = &self.store.read(cx).prefs;
        let chip = |id: &'static str, label: String| {
            button(id, label)
                .small()
                .icon(Icon::Cross)
                .style(ButtonStyle::Gray)
                .tooltip("Stop filtering by this")
        };
        let status = (STATUSES.iter())
            .find(|(status, _)| *status == prefs.branches_status)
            .filter(|(status, _)| *status != BranchStatus::NotMerged)
            .map(|(_, name)| *name);
        let chips: Vec<_> = (branches.who.clone())
            .map(|who| {
                chip("branches-who", who)
                    .on_click(cx.listener(|this, _, _, cx| this.filter_person(None, cx)))
            })
            .into_iter()
            .chain(status.map(|status| {
                chip("branches-status", status.to_owned()).on_click(
                    cx.listener(|this, _, _, cx| this.filter_status(BranchStatus::NotMerged, cx)),
                )
            }))
            .chain(prefs.branches_local_only.then(|| {
                chip("branches-local", "Only local".to_owned()).on_click(cx.listener(
                    |this, _, _, cx| {
                        (this.store).update(cx, |store, cx| {
                            store.set_prefs(|prefs| prefs.branches_local_only = false, cx)
                        });
                        cx.notify();
                    },
                ))
            }))
            .collect();
        (!chips.is_empty()).then(|| {
            h_flex()
                .flex_none()
                .flex_wrap()
                .gap(px(4.))
                .px(px(10.))
                .children(chips)
        })
    }

    /// How many branches the filters but the days keep changed on each of the last 30 days.
    /// Clicking a day, shift-clicking another or dragging across them keeps those changed
    /// then.
    fn branch_activity(
        &self,
        branches: &Branches,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let today = upstream::day_of(crate::ui::now());
        let first_day = today - (ACTIVITY_DAYS as i64 - 1);
        let mut daily = vec![0; ACTIVITY_DAYS];
        for ix in self.kept_branches(branches, Some(Facet::Days), cx) {
            if let Some(day) =
                (branches.list[ix].time.map(upstream::day_of)).filter(|day| *day >= first_day)
            {
                daily[((day - first_day) as usize).min(ACTIVITY_DAYS - 1)] += 1;
            }
        }
        activity_strip(
            &daily,
            first_day,
            &branches.days.days,
            "branch",
            theme,
            cx,
            |this, gesture, cx| this.branch_day_gesture(gesture, cx),
            |this, cx| this.clear_branch_days(cx),
        )
    }

    /// What the list shows without a branch to list: that every one is applied, or that the
    /// filters hide them.
    fn no_branches(&self, branches: &Branches, theme: &Theme, cx: &Context<Self>) -> AnyElement {
        let (drawing, empty) = if branches.list.is_empty() {
            (Illustration::Cactus, "Every branch is applied".to_owned())
        } else {
            let filter = self.branch_filter.read(cx).text();
            let miss = match filter.trim() {
                "" => "The filters hide every branch".to_owned(),
                filter => format!("No branches match \"{filter}\""),
            };
            (Illustration::Papers, miss)
        };
        list_empty(drawing, empty, "", theme, cx)
    }

    /// Whose branches the list shows, and which by their pull request, each choice with how
    /// many it would show; and whether only local ones.
    fn branch_filters_menu(&self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(branches) = self.branches.as_ref() else {
            return;
        };
        let prefs = &self.store.read(cx).prefs;
        let everyone = self.kept_branches(branches, Some(Facet::Who), cx).len();
        let person = |who: Option<String>, label: String, n: usize| {
            let ticked = branches.who == who;
            MenuItem::new(label, move |app, _, cx| {
                let who = who.clone();
                (app.detail).update(cx, |detail, cx| detail.filter_person(who, cx));
            })
            .ticked(ticked)
            .detail(n.to_string())
        };
        let people = self.branch_people(branches, cx);
        // The one picked stays offered, however few branches it has.
        let offered = people
            .iter()
            .enumerate()
            .filter(|(ix, (name, ..))| *ix < PEOPLE_IN_MENU || branches.who.as_ref() == Some(name));
        let mut items = vec![person(None, "Anyone".to_owned(), everyone).under("Person")];
        items.extend(offered.map(|(_, (name, _, n))| person(Some(name.clone()), name.clone(), *n)));
        for (ix, (status, n)) in self.status_counts(branches, cx).into_iter().enumerate() {
            let item = MenuItem::new(STATUSES[ix].1, move |app, _, cx| {
                (app.detail).update(cx, |detail, cx| detail.filter_status(status, cx));
            })
            .ticked(prefs.branches_status == status)
            .detail(n.to_string());
            items.push(match ix {
                0 => item.under("Pull request"),
                _ => item,
            });
        }
        items.push(
            MenuItem::new("Only local branches", |app, _, cx| {
                (app.store).update(cx, |store, cx| {
                    store.set_prefs(|p| p.branches_local_only = !p.branches_local_only, cx)
                });
                // The branch opened may have just been filtered out.
                (app.detail).update(cx, |detail, cx| detail.select_shown_branch(cx));
            })
            .ticked(prefs.branches_local_only)
            .separated(),
        );
        (self.app)
            .update(cx, |app, cx| app.open_menu(position, items, cx))
            .ok();
    }

    /// Beside the list until a branch is opened: how the branches the filters keep stand by
    /// their pull request, and whose they are, most first, each a filter of its own.
    pub(super) fn render_branches_overview(
        &self,
        project: &ProjectId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(branches) = self.branches_for(project) else {
            return div().flex_1().into_any_element();
        };
        let shown = self.shown_branches(branches, cx).len();
        let picked_status = self.store.read(cx).prefs.branches_status;
        let statuses = (self.status_counts(branches, cx).into_iter().enumerate())
            .filter(|(_, (status, _))| {
                !matches!(status, BranchStatus::NotMerged | BranchStatus::Any)
            })
            .map(|(ix, (status, n))| {
                let (glyph, color) = match status {
                    BranchStatus::Open => review::state_glyph(ReviewState::Open, theme),
                    BranchStatus::Draft => review::state_glyph(ReviewState::Draft, theme),
                    BranchStatus::Closed => review::state_glyph(ReviewState::Closed, theme),
                    _ => (Icon::Branch, theme.text_2),
                };
                let picked = picked_status == status;
                // One with none to list is quiet and can't be picked, unless it is.
                let none = n == 0 && !picked;
                list_row(("overview-status", ix), picked, theme)
                    .when(none, |el| el.text_color(theme.text_3))
                    .child(small_icon(glyph).text_color(match none {
                        true => theme.text_3,
                        false => color,
                    }))
                    .child(div().flex_1().child(STATUSES[ix].1))
                    .child(faint(n.to_string(), theme))
                    .when(!none, |el| {
                        el.on_click(cx.listener(move |this, _, _, cx| {
                            let status = match picked {
                                true => BranchStatus::NotMerged,
                                false => status,
                            };
                            this.filter_status(status, cx)
                        }))
                    })
            });
        let people = self.branch_people(branches, cx);
        let most = people.first().map_or(1, |(.., n)| (*n).max(1));
        // The one picked stays listed, however few branches it has, as in the filter menu.
        let listed = |ix: usize, name: &String| {
            ix < PEOPLE_IN_OVERVIEW || branches.who.as_ref() == Some(name)
        };
        let rest = (people.iter().enumerate())
            .filter(|(ix, (name, ..))| !listed(*ix, name))
            .count();
        let people = (people.into_iter().enumerate())
            .filter(|(ix, (name, ..))| listed(*ix, name))
            .map(|(ix, (name, email, n))| {
                let picked = branches.who.as_ref() == Some(&name);
                list_row(("overview-person", ix), picked, theme)
                    .h_auto()
                    .py(px(5.))
                    .child(author_avatar(&name, &email, 20., theme))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w(px(0.))
                            .gap(px(3.))
                            .child(
                                h_flex()
                                    .gap(px(8.))
                                    .child(
                                        div().flex_1().min_w(px(0.)).truncate().child(name.clone()),
                                    )
                                    .child(faint(n.to_string(), theme)),
                            )
                            .child(
                                div().h(px(4.)).rounded(px(2.)).bg(theme.border_3).child(
                                    div()
                                        .h_full()
                                        .rounded(px(2.))
                                        .w(relative(n as f32 / most as f32))
                                        .bg(theme.text_3),
                                ),
                            ),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.filter_person((!picked).then(|| name.clone()), cx)
                    }))
            });
        let people: Vec<_> = people.collect();
        let nobody = people.is_empty();
        let counted = match branches.days.days.is_empty() {
            true => "branches · 30 days",
            false => "branches · days picked",
        };
        // Who first, as the page is for what the team is at, then their pull requests below.
        v_flex()
            .id("branches-overview")
            .flex_1()
            .h_full()
            .min_w(px(0.))
            .overflow_y_scroll()
            .child(
                v_flex()
                    .p(px(6.))
                    .child(
                        overview_box("People", counted.to_owned(), people, theme)
                            .when(nobody, |el| {
                                el.child(faint("No branches listed", theme).px(px(14.)).py(px(4.)))
                            })
                            .when(rest > 0, |el| {
                                el.child(
                                    faint(format!("and {}", plural(rest, "other")), theme)
                                        .px(px(14.)),
                                )
                            }),
                    )
                    .child(overview_box(
                        "Pull requests",
                        format!("{} listed", plural(shown, "branch")),
                        statuses,
                        theme,
                    )),
            )
            .into_any_element()
    }

    /// The opened branch's header, beside the list over its diff where a picked commit's
    /// message goes, as the navigator heads a branch: what it's called, then its name, its
    /// pull request, its commits and who last worked on it, and its description under them.
    /// A stack's caption heads the stack: its branches, top first.
    pub(super) fn render_other_branch_header(
        &self,
        project: &ProjectId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let branches = self.branches_for(project)?;
        let branch = branches.selected_branch()?;
        let target = (self.store.read(cx).workspace(project))
            .and_then(|ws| ws.target.as_ref())
            .map_or("the target".to_owned(), |target| target.name.clone());
        let stack: Vec<&OtherBranch> = (branches.list.iter())
            .filter(|b| b.stack == branch.stack)
            .collect();
        let (glyph, title) = match &branch.review {
            _ if branches.stack => (Icon::Stack, format!("Stack of {}", stack.len())),
            Some(review) if !review.title.is_empty() => (
                review::state_glyph(review.state, theme).0,
                review.title.clone(),
            ),
            _ => (Icon::Branch, shown_name(branch).to_owned()),
        };
        let name = match branches.stack {
            true => (stack.iter().map(|b| shown_name(b)))
                .collect::<Vec<_>>()
                .join(" ← "),
            false => shown_name(branch).to_owned(),
        };
        let review = (branch.review.as_ref())
            .filter(|_| !branches.stack)
            .map(|review| {
                let (glyph, color) = review::state_glyph(review.state, theme);
                let state = match review.state {
                    ReviewState::Open => "Open",
                    ReviewState::Draft => "Draft",
                    ReviewState::Merged => "Merged",
                    ReviewState::Closed => "Closed",
                };
                h_flex()
                    .flex_none()
                    .gap(px(3.))
                    .child(icon(glyph).size(px(14.)).text_color(color))
                    .child(format!("{state} #{}", review.number))
            });
        let commits = match (branch.ahead, branches.stack) {
            (Some(0), false) => format!("in {target}"),
            (_, true) => plural(stack.iter().filter_map(|b| b.own_commits).sum(), "commit"),
            (_, false) => plural(branch.own_commits.unwrap_or(0), "commit"),
        };
        let byline = (branch.author.as_ref())
            .zip(branch.time)
            .map(|(author, time)| {
                let email = branch.email.as_deref().unwrap_or_default();
                byline(author, email, branch.agent.as_ref(), time, theme).flex_none()
            });
        let description = (branch.description.clone()).filter(|_| !branches.stack);
        // And the rule along its bottom, which the diff starts below, as a commit's message.
        let height = measured(&self.target_message_height, &self.app, |bounds| {
            f32::from(bounds.size.height) + 1.
        });
        Some(
            v_flex()
                .relative()
                .flex_none()
                .gap(px(8.))
                .px(px(16.))
                .py(px(12.))
                .border_b_1()
                .border_color(theme.border_3)
                .child(page_title(glyph, theme).child(title_text(title.clone()).truncate()))
                .child(
                    meta_row(theme)
                        // Its name, unless the title already is.
                        .when(name != title, |el| {
                            el.child(
                                meta_item(Icon::Branch, theme)
                                    .min_w(px(0.))
                                    .child(div().min_w(px(0.)).truncate().child(name)),
                            )
                        })
                        .children(review)
                        .child(meta_item(Icon::Commit, theme).flex_none().child(commits))
                        .children(byline),
                )
                .children(description.map(|description| {
                    div()
                        .text_size(px(13.))
                        .text_color(theme.text_2)
                        .whitespace_normal()
                        .child(description)
                }))
                .child(height)
                .into_any_element(),
        )
    }

    /// The page's crumbs after "Branches": the selected stack or branch, and the commit of it
    /// selected.
    pub(super) fn branches_crumbs(&self, project: &ProjectId, crumbs: &mut Vec<Crumb>) {
        let Some(branch) = self.selected_other_branch(project) else {
            return;
        };
        if let Some(branches) = self.branches_for(project).filter(|b| b.stack) {
            let count = (branches.list.iter())
                .filter(|b| b.stack == branch.stack)
                .count();
            return crumbs.push((format!("Stack of {count}").into(), None));
        }
        let commit = self.picked_branch_commit(project);
        // Over a commit of it, the branch goes back to the branch.
        let back = commit.map(|_| CrumbTo::OtherBranch(branch.ref_name.clone()));
        crumbs.push((branch.name.clone().into(), back));
        if let Some(commit) = commit {
            crumbs.push((commit.id.short().into(), None));
        }
    }
}

fn is_selected(branches: &Branches, row: &Row) -> bool {
    let selected = |ix: usize| branches.selected.as_ref() == Some(&branches.list[ix].ref_name);
    match *row {
        Row::Heading(_) => false,
        Row::Stack(top, ..) => selected(top) && branches.stack,
        Row::Branch(ix, _) => selected(ix) && !branches.stack && branches.commit.is_none(),
        Row::Commit(ix, commit, _) => {
            let commits = branches.commits.get(&branches.list[ix].ref_name);
            selected(ix)
                && commits.and_then(|(_, c)| c.get(commit)).map(|c| c.id) == branches.commit
        }
    }
}

/// Whether `row` is a commit picked with others on the selected branch.
fn is_picked(branches: &Branches, row: &Row) -> bool {
    let Row::Commit(ix, commit, _) = *row else {
        return false;
    };
    let ref_name = &branches.list[ix].ref_name;
    branches.selected.as_ref() == Some(ref_name)
        && (branches.commits.get(ref_name))
            .and_then(|(_, commits)| commits.get(commit))
            .is_some_and(|c| branches.picked.contains(&c.id))
}

/// What the list calls a branch, as the navigator does: its pull request's title once known,
/// else its name, a remote's as the remote names it (origin/…).
fn title(branch: &OtherBranch) -> &str {
    (branch.review.as_ref())
        .map(|review| review.title.as_str())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| shown_name(branch))
}

/// A branch's name, a remote's as the remote names it (origin/…).
fn shown_name(branch: &OtherBranch) -> &str {
    (branch.ref_name.strip_prefix("refs/remotes/"))
        .filter(|_| !branch.local)
        .unwrap_or(&branch.name)
}

/// Whether `status` keeps `branch`, by its pull request.
fn status_keeps(status: BranchStatus, branch: &OtherBranch) -> bool {
    let state = branch.review.as_ref().map(|review| review.state);
    match status {
        BranchStatus::NotMerged => state != Some(ReviewState::Merged),
        BranchStatus::Open => state == Some(ReviewState::Open),
        BranchStatus::Draft => state == Some(ReviewState::Draft),
        BranchStatus::Closed => state == Some(ReviewState::Closed),
        BranchStatus::NoPullRequest => state.is_none(),
        BranchStatus::Any => true,
    }
}

/// A card of the overview: `title` and `detail` over its `rows`.
fn overview_box(
    title: &str,
    detail: String,
    rows: impl IntoIterator<Item = impl IntoElement>,
    theme: &Theme,
) -> gpui::Div {
    v_flex()
        .m(px(6.))
        .py(px(8.))
        .gap(px(2.))
        .border_1()
        .border_color(theme.border_3)
        .rounded(px(8.))
        .child(
            h_flex()
                .px(px(14.))
                .pb(px(4.))
                .gap(px(8.))
                .child(section_label(title.to_owned(), theme).flex_grow(1.))
                .child(faint(detail, theme)),
        )
        .children(rows)
}
