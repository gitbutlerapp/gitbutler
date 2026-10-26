//! The Branches page: branches outside the workspace, newest first, local ones and the ones
//! only a remote has alike, each with what it would bring in, ready to apply or delete. Beside
//! the list, an overview of the work under way until one is opened, then its diff.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use bstr::ByteSlice as _;
use gpui::{
    AnyElement, App, ClickEvent, Context, MouseButton, MouseDownEvent, Pixels, Point, SharedString,
    Window, div, prelude::*,
};

mod overview;

use super::Detail;
use super::panes::{
    Crumb, CrumbTo, LIST_MIN_HEIGHT, list_empty, list_filtered_out, list_heading, measured,
    meta_item, meta_row, page_title, side_list, step_from, title_text,
};
use super::upstream::commit_author;

use crate::app::AppView;
use crate::backend::branches::{self, ApplyOutcome, OtherBranch};
use crate::backend::upstream::{self, ACTIVITY_DAYS, TargetCommit};
use crate::backend::{self, CommitId, DiffSource, FullName, ReviewState};
use crate::edits::CherryPick;
use crate::overlays::MenuItem;
use crate::state::{BranchStatus, Cut, Label, Overlay, Picks, ProjectId, Selection, Store};
use crate::theme::Theme;
use crate::transfer::drag_preview;
use crate::ui::illustration::Illustration;
use crate::ui::person::{byline, byline_avatar};
use crate::ui::text_input::TextInput;
use crate::ui::{
    ButtonStyle, Icon, Tooltip, age_column, button, dp, error_summary, faint, fold_button, guide,
    h_flex, icon, icon_button, loading_state, plural, quiet, review, side_row, small_icon, v_flex,
};

pub(super) struct Branches {
    project: ProjectId,
    /// The project's branches when the list loaded; a reload that changes none of them
    /// leaves the list as it is.
    seen: u64,
    list: Vec<OtherBranch>,
    /// The workspace's applied branches, which the list leaves to the navigator and the
    /// overview counts too.
    applied: Vec<OtherBranch>,
    /// The selected branch, by ref, whose diff shows; none while the overview does.
    selected: Option<FullName>,
    /// The selection is the caption of `selected`'s stack, which names its top branch: that
    /// one's changes are the whole stack's.
    stack: bool,
    /// The branches whose commits show under them, by ref: folded until opened, as the
    /// navigator's are.
    open: HashSet<FullName>,
    /// The stacks whose branches are folded away under their caption, by their top branch.
    folded: HashSet<FullName>,
    /// Open branches' commits, by ref and what they were loaded for.
    commits: HashMap<FullName, (Loaded, Vec<TargetCommit>)>,
    /// One of the selected branch's commits, to see on its own and to copy into the workspace.
    commit: Option<CommitId>,
    /// Commits of the selected branch picked along with the selected one (⌘- or ⇧-click), to
    /// cherry-pick together.
    picked: Picks<CommitId>,
    /// The one person whose branches are listed, by name, as `OtherBranch::people` has it.
    who: Option<String>,
    /// How recently the branches the list keeps changed.
    recency: Recency,
    /// Why reading the branches failed, shown in place of the list.
    failed: Option<String>,
}

impl Branches {
    /// Branch `ref_name`, while it's listed.
    fn branch(&self, ref_name: &FullName) -> Option<&OtherBranch> {
        self.list.iter().find(|b| b.ref_name == *ref_name)
    }

    fn selected_branch(&self) -> Option<&OtherBranch> {
        self.branch(self.selected.as_ref()?)
    }

    /// The commits listed under branch `ref_name`: its own, not those of the branches below it
    /// in a stack.
    fn listed_commits(&self, ref_name: &FullName) -> &[TargetCommit] {
        let own = self.branch(ref_name).and_then(|b| b.own_commits);
        match self.commits.get(ref_name) {
            Some((_, commits)) => own.and_then(|own| commits.get(..own)).unwrap_or(commits),
            None => &[],
        }
    }

    /// What cherry-picking commit `id` takes: the picks, in the order they're listed, when
    /// it's one of them, or else just it.
    fn cherry_picks(&self, id: CommitId) -> Arc<[CommitId]> {
        // Not merges, as `git cherry-pick` refuses them too: which side to take isn't said.
        let merge = |id: &CommitId| {
            (self.commits.values().flat_map(|(_, commits)| commits)).any(|c| c.id == *id && c.merge)
        };
        let Some(ref_name) = self.selected.as_ref().filter(|_| self.picked.contains(&id)) else {
            return [id].into_iter().filter(|id| !merge(id)).collect();
        };
        (self.listed_commits(ref_name).iter())
            .map(|commit| commit.id)
            .filter(|id| self.picked.contains(id) && !merge(id))
            .collect()
    }

    /// `all` of `project`'s branches as read when its workspace was `seen`, the applied ones
    /// apart, keeping what of `previous` still applies: the branch opened, those open and
    /// their commits, and the filters.
    fn read(
        project: ProjectId,
        seen: u64,
        all: Vec<OtherBranch>,
        previous: Option<Branches>,
    ) -> Self {
        let (applied, list): (Vec<_>, Vec<_>) = all.into_iter().partition(|b| b.applied);
        // The branch opened, while it's listed: once it goes, as an agent renamed or
        // deleted it, the overview shows rather than another branch in its place.
        let selected = (previous.as_ref())
            .and_then(|b| b.selected.clone())
            .filter(|ref_name| list.iter().any(|b| b.ref_name == *ref_name));
        // Which branches were open and their commits, of those still listed, and the
        // selected commit while its branch stays selected.
        let listed = |ref_name: &FullName| list.iter().any(|b| b.ref_name == *ref_name);
        let (who, recency) = (previous.as_ref())
            .map(|b| (b.who.clone(), b.recency))
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
        Branches {
            project,
            seen,
            list,
            applied,
            selected,
            stack,
            open,
            folded,
            commits,
            commit,
            picked,
            who,
            recency,
            failed: None,
        }
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

/// The list's filters, besides the text: whose branches, by their pull request, and how
/// recently changed; one left out counts what each of its choices would show.
#[derive(Clone, Copy, PartialEq)]
enum Facet {
    Who,
    Status,
    Recency,
}

/// How recently a branch has to have changed for the list to keep it.
#[derive(Clone, Copy, Default, PartialEq)]
enum Recency {
    #[default]
    Any,
    Today,
    Week,
    Month,
}

impl Recency {
    const ALL: [Recency; 4] = [Recency::Any, Recency::Today, Recency::Week, Recency::Month];

    fn label(self) -> &'static str {
        match self {
            Recency::Any => "Any time",
            Recency::Today => "Today",
            Recency::Week => "Last 7 days",
            Recency::Month => "Last 30 days",
        }
    }

    /// How many days back it reaches, today the first; for any time, the last 30, which is
    /// as far back as the overview and the People card count.
    fn days(self) -> i64 {
        match self {
            Recency::Today => 1,
            Recency::Week => 7,
            Recency::Any | Recency::Month => ACTIVITY_DAYS as i64,
        }
    }

    /// The first day it reaches back to, as `upstream::day_of` numbers them.
    fn since(self, today: i64) -> i64 {
        today - (self.days() - 1)
    }

    /// Whether something last changed at `time`, if that's known, is recent enough; one
    /// after `today` counts as today's.
    fn keeps(self, time: Option<i64>, today: i64) -> bool {
        self == Recency::Any
            || time.is_some_and(|time| upstream::day_of(time).min(today) >= self.since(today))
    }
}

/// The pull request choices, as the filter menu and the chip name them. There's no Merged
/// choice: the page is for work under way, and the project's page lists what landed.
const STATUSES: [(BranchStatus, &str); 6] = [
    (BranchStatus::NotMerged, "In progress"),
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
    /// List `project`'s branches, the applied ones apart; unless `force`, only when its
    /// workspace changed.
    pub(super) fn load_branches(
        &mut self,
        project: &ProjectId,
        force: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.store.read(cx).path(project) else {
            return;
        };
        // What the listing depends on: the project's branches, as its last full load saw them.
        let seen = (self.store.read(cx).workspace(project)).map_or(0, |ws| ws.refs);
        if !force && self.branches_for(project).is_some_and(|b| b.seen == seen) {
            return;
        }
        let project = project.clone();
        self._branches_load = cx.spawn(async move |this, cx| {
            let result = backend::run(move || branches::all_branches(&path)).await;
            this.update(cx, |this, cx| {
                // When read for the project page's count, that page stays as it is.
                let shown =
                    matches!(this.selection(cx), Some(Selection::Branches(p)) if p == project);
                let mut all = match result {
                    Ok(all) => all,
                    // Said in place of the list, which comes back as it was once a read on a
                    // later change works.
                    Err(err) => {
                        tracing::warn!(?err, "couldn't list the branches");
                        let key = project.clone();
                        let empty = || Branches::read(project, seen, Vec::new(), None);
                        let branches = this.branches.entry(key).or_insert_with(empty);
                        branches.failed = Some(format!("{err:#}"));
                        if shown {
                            this.refresh_diff(false, cx);
                        }
                        return cx.notify();
                    }
                };
                // One a linked worktree the navigator shows has checked out is in use there,
                // as applied ones are in the workspace.
                if let Some(ws) = this.store.read(cx).workspace(&project) {
                    for b in all.iter_mut().filter(|b| b.local) {
                        b.applied |= ws.checked_out_in(&b.ref_name).is_some();
                    }
                }
                let previous = this.branches.remove(&project);
                let branches = Branches::read(project.clone(), seen, all, previous);
                this.branches.insert(project.clone(), branches);
                this.load_open_commits(&project, cx);
                if shown {
                    this.open_waiting(true, cx);
                    // The filter may hide what's selected now.
                    this.select_shown_branch(cx);
                    this.refresh_diff(false, cx);
                }
                cx.notify();
            })
            .ok();
        });
    }

    /// How many of `project`'s branches aren't applied, as its Branches page lists them at
    /// first: with commits of their own, and no merged or closed pull request.
    pub(super) fn unapplied(&self, project: &ProjectId) -> Option<usize> {
        let branches = self.branches_for(project)?;
        Some(
            (branches.list.iter())
                .filter(|b| b.own_commits != Some(0))
                .filter(|b| status_keeps(BranchStatus::NotMerged, b))
                .count(),
        )
    }

    /// `project`'s branches, once read.
    fn branches_for(&self, project: &ProjectId) -> Option<&Branches> {
        self.branches.get(project).filter(|b| b.failed.is_none())
    }

    /// The branches of the Branches page selected.
    fn page_branches(&self, cx: &App) -> Option<&Branches> {
        match &self.store.read(cx).selection {
            Some(Selection::Branches(project)) => self.branches_for(project),
            _ => None,
        }
    }

    fn page_branches_mut(&mut self, cx: &App) -> Option<&mut Branches> {
        match &self.store.read(cx).selection {
            Some(Selection::Branches(project)) => self
                .branches
                .get_mut(project)
                .filter(|b| b.failed.is_none()),
            _ => None,
        }
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

    /// Esc on the Branches page with a branch opened: back to the overview, whether it did. The
    /// app, which Esc is handled in, redraws for the diff that goes.
    pub fn close_opened_branch(&mut self, cx: &mut Context<Self>) -> bool {
        let opened = matches!(&self.store.read(cx).selection,
            Some(Selection::Branches(project)) if self.selected_other_branch(project).is_some());
        if opened {
            self.select_other_branch(None, cx);
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
    pub(super) fn listed_branch(
        &self,
        project: &ProjectId,
        ref_name: &FullName,
    ) -> Option<&OtherBranch> {
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

    /// Load `project`'s open branches' commits that aren't loaded for them as they are now. A
    /// selected commit its branch no longer has is let go.
    fn load_open_commits(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        let Some(branches) = self.branches_for(project) else {
            return;
        };
        let wanted: Vec<(FullName, Loaded)> = (branches.list.iter())
            .filter(|b| branches.open.contains(&b.ref_name))
            .map(|b| (b.ref_name.clone(), (b.tip, b.ahead)))
            .filter(|(r, key)| (branches.commits.get(r)).is_none_or(|(loaded, _)| loaded != key))
            .collect();
        let Some(path) = (self.store.read(cx).path(project)).filter(|_| !wanted.is_empty()) else {
            return;
        };
        let project = project.clone();
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
                let (Ok(loaded), Some(branches)) = (loaded, this.branches.get_mut(&project)) else {
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
    fn toggle_stack(&mut self, top: &FullName, cx: &mut Context<Self>) {
        let Some(branches) = self.page_branches_mut(cx) else {
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
    fn toggle_branch(&mut self, ref_name: &FullName, cx: &mut Context<Self>) {
        let Some(branches) = self.page_branches_mut(cx) else {
            return;
        };
        if branches.open.insert(ref_name.to_owned()) {
            let project = branches.project.clone();
            self.load_open_commits(&project, cx);
        } else {
            branches.open.remove(ref_name);
            if branches.selected.as_ref() == Some(ref_name) && branches.commit.take().is_some() {
                branches.picked.clear();
                self.show_picked(cx);
            }
        }
        cx.notify();
    }

    /// The branches the filters keep, newest first, under headings by when they last
    /// changed, a stack's branches together under a caption while more than one of them shows.
    fn branch_rows(&self, branches: &Branches, cx: &Context<Self>) -> Vec<Row> {
        let shown: Vec<(usize, &OtherBranch)> = (self.kept_in(branches, &branches.list, None, cx))
            .into_iter()
            .filter_map(|ix| Some((ix, branches.list.get(ix)?)))
            .collect();
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
        for stack in shown.chunk_by(|(_, a), (_, b)| a.stack == b.stack) {
            let Some(&(first_ix, first)) = stack.first() else {
                continue;
            };
            // A day ahead, from a machine whose clock runs fast, counts as today, as the
            // activity strip has it.
            let ago =
                (newest.get(&first.stack)).map(|time| (today - upstream::day_of(*time)).max(0));
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
                let (top, top_branch) = (branches.list.iter().enumerate())
                    .find(|(_, b)| b.stack == first.stack)
                    .unwrap_or((first_ix, first));
                let folded = branches.folded.contains(&top_branch.ref_name);
                rows.push(Row::Stack(top, stack.len(), folded));
                if folded {
                    continue;
                }
            }
            for &(ix, branch) in stack {
                rows.push(Row::Branch(ix, stacked));
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

    /// Those of `list`, the list's or the applied ones, the filters keep, by index into it and
    /// folded away or not. All but `skip`'s, which is left out to count its choices.
    fn kept_in(
        &self,
        branches: &Branches,
        list: &[OtherBranch],
        skip: Option<Facet>,
        cx: &App,
    ) -> Vec<usize> {
        let query = self.branch_filter.read(cx).text().trim().to_lowercase();
        let store = self.store.read(cx);
        let prefs = &store.prefs;
        let today = upstream::day_of(crate::ui::now());
        let says = |text: &str| text.to_lowercase().contains(&query);
        let keeps = |branch: &OtherBranch| {
            let named = !query.is_empty() && says(&branch.name);
            let found = query.is_empty()
                || named
                || says(&title(branch))
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
                && (skip == Some(Facet::Recency) || branches.recency.keeps(branch.time, today))
        };
        (list.iter().enumerate())
            .filter(|(_, branch)| keeps(branch))
            .map(|(ix, _)| ix)
            .collect()
    }

    /// The people at work lately: who wrote commits in the last activity picked, or the last
    /// 30 days, on the branches the filters keep but the person, applied ones too; most
    /// branches first, with how many and an email for their picture. Not everyone who ever
    /// made one, which would put those long gone first and leave out the newly arrived.
    /// Picking someone lists the branches they have any commit of, applied ones aside, so
    /// that list can be longer or shorter than their count.
    fn branch_people(&self, branches: &Branches, cx: &App) -> Vec<(String, String, usize)> {
        let since = branches.recency.since(upstream::day_of(crate::ui::now()));
        let mut people: Vec<(String, String, usize)> = Vec::new();
        // The applied ones too, as the overview counts them.
        let listed = self.kept_in(branches, &branches.list, Some(Facet::Who), cx);
        let applied = self.kept_in(branches, &branches.applied, Some(Facet::Who), cx);
        let listed = listed.into_iter().filter_map(|ix| branches.list.get(ix));
        let applied = applied
            .into_iter()
            .filter_map(|ix| branches.applied.get(ix));
        let kept = listed.chain(applied);
        for branch in kept {
            // Who wrote its commits in those days, each once, not everyone on a branch only
            // rebased then.
            let mut theirs: Vec<usize> = (branch.commits.iter())
                .filter(|commit| commit.day >= since)
                .map(|commit| commit.person)
                .collect();
            theirs.sort_unstable();
            theirs.dedup();
            for person in theirs.into_iter().filter_map(|p| branch.people.get(p)) {
                match people.iter_mut().find(|(name, ..)| name == person) {
                    Some((.., n)) => *n += 1,
                    None => people.push((person.clone(), String::new(), 1)),
                }
            }
        }
        // The one picked stays, with none, when the other filters leave them nothing.
        if let Some(who) = branches
            .who
            .as_ref()
            .filter(|who| !people.iter().any(|p| p.0 == **who))
        {
            people.push((who.clone(), String::new(), 0));
        }
        for (name, email, _) in &mut people {
            *email = email_of(branches, name);
        }
        people.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
        people
    }

    /// How many branches the filters keep but the pull request one, for each choice of it.
    fn status_counts(&self, branches: &Branches, cx: &App) -> Vec<(BranchStatus, usize)> {
        let kept = self.kept_in(branches, &branches.list, Some(Facet::Status), cx);
        (STATUSES.iter())
            .map(|(status, _)| {
                let n = (kept.iter())
                    .filter_map(|ix| branches.list.get(*ix))
                    .filter(|branch| status_keeps(*status, branch))
                    .count();
                (*status, n)
            })
            .collect()
    }

    /// List only `who`'s branches, or everyone's.
    pub(crate) fn filter_person(&mut self, who: Option<String>, cx: &mut Context<Self>) {
        if let Some(branches) = self.page_branches_mut(cx) {
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

    /// List the branches changed as recently as `recency` says.
    fn filter_recency(&mut self, recency: Recency, cx: &mut Context<Self>) {
        if let Some(branches) = self.page_branches_mut(cx) {
            branches.recency = recency;
        }
        self.select_shown_branch(cx);
        cx.notify();
    }

    /// Let go of the opened branch once the filters hide it, so the overview shows rather
    /// than the diff of one the list doesn't.
    pub(super) fn select_shown_branch(&mut self, cx: &mut Context<Self>) {
        let Some(branches) = self.page_branches(cx) else {
            return;
        };
        // A caption stays while it's listed, though the filter hides its top branch.
        let rows = self.branch_rows(branches, cx);
        if branches.stack && rows.iter().any(|row| is_selected(branches, row)) {
            return;
        }
        // One folded away in its stack still counts.
        let selected = |ix: &usize| {
            (branches.list.get(*ix))
                .is_some_and(|b| Some(&b.ref_name) == branches.selected.as_ref())
        };
        if (self.kept_in(branches, &branches.list, None, cx).iter()).any(selected) {
            // A stack the filter leaves one branch of has no caption to stay on.
            if branches.stack {
                self.select_other_branch(branches.selected.clone(), cx);
            }
        } else if branches.selected.is_some() {
            self.select_other_branch(None, cx);
        }
    }

    pub(super) fn select_other_branch(
        &mut self,
        ref_name: Option<FullName>,
        cx: &mut Context<Self>,
    ) {
        self.select_listed(ref_name, false, cx)
    }

    /// Select the caption of the stack whose top branch is `top`.
    fn select_stack(&mut self, top: FullName, cx: &mut Context<Self>) {
        self.select_listed(Some(top), true, cx)
    }

    fn select_listed(&mut self, ref_name: Option<FullName>, stack: bool, cx: &mut Context<Self>) {
        self.message_open = false;
        if let Some(branches) = self.page_branches_mut(cx) {
            branches.selected = ref_name;
            branches.stack = stack;
            branches.commit = None;
            branches.picked.clear();
            let project = branches.project.clone();
            self.load_open_commits(&project, cx);
        }
        self.show_picked(cx);
    }

    /// Select commit `id` of the branch `ref_name`, which it's listed under, alone.
    fn select_branch_commit(&mut self, ref_name: &FullName, id: CommitId, cx: &mut Context<Self>) {
        self.message_open = false;
        if let Some(branches) = self.page_branches_mut(cx) {
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
        ref_name: &FullName,
        id: CommitId,
        run: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(branches) = self.page_branches_mut(cx) else {
            return;
        };
        let Some(at) = branches
            .commit
            .filter(|_| branches.selected.as_ref() == Some(ref_name))
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
        let Some(branches) = self.page_branches(cx) else {
            return;
        };
        let rows = self.branch_rows(branches, cx);
        // Over the headings, which aren't selected.
        let selectable: Vec<(usize, &Row)> = (rows.iter().enumerate())
            .filter(|(_, row)| !matches!(row, Row::Heading(_)))
            .collect();
        // A selected branch folded away in its stack steps from the stack's row.
        let in_stack = |top: usize| {
            let stack = branches.list.get(top).map(|b| b.stack);
            (branches.selected_branch()).is_some_and(|b| Some(b.stack) == stack)
        };
        let at = selectable.iter().position(|&(_, row)| {
            is_selected(branches, row) || matches!(*row, Row::Stack(top, _, true) if in_stack(top))
        });
        if let Some(&(next, row)) = step_from(&selectable, at, delta) {
            self.select_branch_row(row, cx);
            (self.branches_scroll).scroll_to_item(next, gpui::ScrollStrategy::Nearest);
        }
    }

    /// ⇧→ in the list: open the selected stack or branch, or step into it when it's open.
    /// ⇧← folds it, or steps from one of its commits or branches back up to it.
    fn fold_branches(&mut self, open: bool, cx: &mut Context<Self>) {
        let Some(branches) = self.page_branches(cx) else {
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
                let in_stack = |at: &usize| matches!(rows.get(*at), Some(Row::Branch(_, true)));
                let caption = at.filter(in_stack).and_then(|at| {
                    rows.iter().take(at).rev().find_map(|row| match row {
                        Row::Stack(top, ..) => branches.list.get(*top).map(|b| b.ref_name.clone()),
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
        let Some(branches) = self.page_branches(cx) else {
            return;
        };
        let (Row::Stack(ix, ..) | Row::Branch(ix, _) | Row::Commit(ix, ..)) = *row else {
            return;
        };
        let Some(ref_name) = branches.list.get(ix).map(|b| b.ref_name.clone()) else {
            return;
        };
        match *row {
            Row::Heading(_) => {}
            Row::Stack(..) => self.select_stack(ref_name, cx),
            Row::Branch(..) => self.select_other_branch(Some(ref_name), cx),
            Row::Commit(_, commit, _) => {
                let Some((_, commits)) = branches.commits.get(&ref_name) else {
                    return;
                };
                let Some(id) = commits.get(commit).map(|c| c.id) else {
                    return;
                };
                self.select_branch_commit(&ref_name, id, cx)
            }
        }
    }

    /// Offer what a branch's header does, and its pull request and name.
    fn branch_menu(
        &mut self,
        project: &ProjectId,
        ref_name: &FullName,
        at: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let (project, ref_name) = (project.clone(), ref_name.to_owned());
        self.open_menu(
            at,
            move |this, cx| this.branch_menu_items(&project, &ref_name, cx),
            cx,
        );
    }

    /// What the Branches page offers for its branch `ref_name`, as right-clicking it does.
    pub(crate) fn branch_menu_items(
        &self,
        project: &ProjectId,
        ref_name: &FullName,
        cx: &App,
    ) -> Vec<MenuItem> {
        // By its name, as the table has none opened.
        let Some(branch) = self.listed_branch(project, ref_name) else {
            return Vec::new();
        };
        let (p, r) = (project.clone(), ref_name.to_owned());
        let mut items = vec![MenuItem::keyed(
            "Apply",
            &crate::app::Confirm,
            cx,
            move |app, window, cx| {
                (app.detail).update(cx, |detail, cx| detail.apply_branch(&p, &r, window, cx))
            },
        )];
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
            items.push(MenuItem::new(
                "Edit description…",
                move |app, window, cx| {
                    (app.detail).update(cx, |detail, cx| {
                        detail.edit_branch_description(&p, &r, window, cx)
                    })
                },
            ));
        }
        if branch.local {
            let (p, r) = (project.clone(), ref_name.to_owned());
            items.push(
                MenuItem::new("Delete branch…", move |app, window, cx| {
                    (app.detail).update(cx, |detail, cx| {
                        detail.delete_other_branch(&p, &r, window, cx)
                    })
                })
                .danger()
                .separated(),
            );
        }
        items
    }

    /// Select commit `id` of `ref_name`, keeping the picks when it's one of them, and offer
    /// to copy it, or them, into the workspace, or its ID.
    fn branch_commit_menu(
        &mut self,
        project: &ProjectId,
        ref_name: &FullName,
        id: CommitId,
        at: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        match self.page_branches_mut(cx) {
            Some(branches) if branches.picked.contains(&id) => {
                branches.commit = Some(id);
                self.show_picked(cx);
            }
            _ => self.select_branch_commit(ref_name, id, cx),
        }
        let Some(ids) = self.page_branches(cx).map(|b| b.cherry_picks(id)) else {
            return;
        };
        let label = match ids.len() {
            1 => "Cherry-pick".to_owned(),
            n => format!("Cherry-pick {n} commits"),
        };
        let project = project.clone();
        self.open_menu(
            at,
            move |_, _| {
                // None for a merge, which can't be.
                let pick = (!ids.is_empty()).then(|| {
                    MenuItem::new(label, move |app, _, cx| {
                        app.cut(Cut::CherryPick(project.clone(), ids.clone()), cx)
                    })
                });
                (pick.into_iter())
                    .chain([MenuItem::new("Copy commit ID", move |app, _, cx| {
                        app.copy_text(id.0.to_string(), cx)
                    })])
                    .collect()
            },
            cx,
        );
    }

    /// Pick up the selected commit, with the others picked, to put copies of them where ⌘V
    /// lands in the workspace.
    fn copy_branch_commit(
        &mut self,
        project: &ProjectId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((branches, id)) =
            (self.branches_for(project)).and_then(|branches| Some((branches, branches.commit?)))
        else {
            return;
        };
        let ids = branches.cherry_picks(id);
        if ids.is_empty() {
            return;
        }
        let cut = Cut::CherryPick(project.clone(), ids);
        self.app
            .update_later(window, cx, move |app, _, cx| app.cut(cut, cx));
    }

    /// Return on the list: apply the selected branch, as its menu's Apply does.
    fn apply_selected_branch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Selection::Branches(project)) = self.store.read(cx).selection.clone() else {
            return;
        };
        if let Some(ref_name) = (self.selected_other_branch(&project)).map(|b| b.ref_name.clone()) {
            self.apply_branch(&project, &ref_name, window, cx)
        }
    }

    fn apply_branch(
        &mut self,
        project: &ProjectId,
        ref_name: &FullName,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(branch) = self.listed_branch(project, ref_name) else {
            return;
        };
        // Applied, it would move under the archived worktree that has it checked out, leaving
        // that worktree's index behind for the agent's next commit there to take back, so that
        // worktree lets go of it first, once asked.
        let archived = (self.store.read(cx).workspace(project))
            .and_then(|ws| {
                (ws.archived_worktrees.iter())
                    .find(|(_, checked_out)| checked_out.as_ref() == Some(&branch.ref_name))
            })
            .map(|(worktree, _)| worktree.clone());
        let (ref_name, name) = (branch.ref_name.clone(), branch.name.clone());
        let (title, at) = (
            format!("Apply {name}?"),
            Selection::Branches(project.clone()),
        );
        let (project, selected) = (project.clone(), project.clone());
        let (freeing, ask) = (
            archived.clone(),
            archived.map(|worktree| {
                format!(
                    "{name} is checked out in the archived worktree {worktree}, which stays on its \
                 last commit, without the branch."
                )
            }),
        );
        let apply = move |store: &mut Store, cx: &mut Context<Store>| {
            store.run(
                &project,
                Label::new("Apply", format!("Apply {name}")),
                Overlay::default(),
                move |path, _| {
                    branches::apply(path, &ref_name, freeing.as_ref().map(AsRef::as_ref))
                },
                move |store, applied, cx| match &applied {
                    ApplyOutcome::Applied(local) | ApplyOutcome::AlreadyApplied(local) => {
                        store.select(Some(Selection::Branch(selected, Some(local.clone()))), cx);
                        // As when an agent applied it meanwhile.
                        if matches!(applied, ApplyOutcome::AlreadyApplied(_)) {
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
        };
        match ask {
            None => self.store.update(cx, apply),
            Some(why) => self.app.update_later(window, cx, move |app, window, cx| {
                let run =
                    move |app: &mut AppView, cx: &mut Context<AppView>| app.store.update(cx, apply);
                app.confirm_then(Some(at), title, &why, "Apply", window, cx, run)
            }),
        }
    }

    fn delete_other_branch(
        &mut self,
        project: &ProjectId,
        ref_name: &FullName,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(branch) = self.listed_branch(project, ref_name).filter(|b| b.local) else {
            return;
        };
        let ref_name = branch.ref_name.clone();
        let (project, name, tip) = (project.clone(), branch.name.clone(), branch.tip);
        let detail = cx.entity().downgrade();
        self.app.update_later(window, cx, move |app, window, cx| {
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
                            move |path, _| backend::remove_branch(path, &ref_name, Some(tip)),
                            // Out of the list now: reading the branches again, as their
                            // refs changed, lands after the overlay is gone.
                            move |store, (), cx| {
                                (detail.update(cx, |detail, _| {
                                    if let Some(branches) = detail.branches.get_mut(&listed_in) {
                                        branches.list.retain(|b| b.ref_name != gone);
                                    }
                                }))
                                .ok();
                                // The overview, rather than its diff, once it's gone: after
                                // this update of the store, which the list reads.
                                let detail = detail.clone();
                                cx.defer(move |cx| {
                                    (detail
                                        .update(cx, |detail, cx| detail.select_shown_branch(cx)))
                                    .ok();
                                });
                                // The oplog can't bring it back, so ⌘Z won't try.
                                store.keep_last_snapshot(cx);
                                store.snack("Branch deleted", cx)
                            },
                            cx,
                        )
                    })
                },
            )
        });
    }

    /// Ask for local branch `ref_name`'s description and keep it where Git does.
    fn edit_branch_description(
        &mut self,
        project: &ProjectId,
        ref_name: &FullName,
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
        let detail = cx.entity().downgrade();
        self.app.update_later(window, cx, move |app, window, cx| {
            let place = app
                .store
                .read(cx)
                .place(&Selection::Project(project.clone()));
            app.open_prompt(
                Some(place),
                &title,
                Some(crate::ui::keys_text(
                    "Git keeps it as the branch's description, where git branch \
                     --edit-description does. Leave it empty to remove it. ⌘↩ saves it.",
                )),
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
                            move |path, _| branches::set_branch_description(path, &ref_name, &text),
                            // Its refs didn't change, so the list isn't read again.
                            move |_, (), cx| {
                                (detail.update(cx, |detail, cx| {
                                    let listed =
                                        (detail.branches.get_mut(&listed_in)).and_then(|b| {
                                            b.list.iter_mut().find(|b| b.ref_name == gone)
                                        });
                                    if let Some(branch) = listed {
                                        branch.description = (!saved.is_empty()).then_some(saved);
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
        });
    }

    pub(super) fn render_branches_side(
        &self,
        project: &ProjectId,
        theme: &Theme,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(branches) = self.branches.get(project) else {
            return loading_state("Loading branches…", theme, cx);
        };
        if let Some(err) = &branches.failed {
            let title = "Couldn't list the branches".to_owned();
            return list_empty(Illustration::Papers, title, error_summary(err), theme, cx);
        }
        let filter = self.branch_filter_bar(project, window, cx);
        let chips = self.branch_filter_chips(cx);
        let rows = self.branch_rows(branches, cx);
        let people = self.branch_people_card(branches, window, cx);
        let target = (self.store.read(cx).workspace(project))
            .and_then(|ws| ws.target.as_ref())
            .map(|target| target.name.clone());
        let list = if rows.is_empty() {
            self.no_branches(branches, theme, cx)
        } else {
            let shown = project.clone();
            side_list(
                "other-branches",
                rows.len(),
                &self.branches_scroll,
                "branches-scrollbar",
                cx,
                move |this, ix, theme, window, cx| {
                    let branches = this.branches_for(&shown)?;
                    let solid = this.side_focus.is_focused(window);
                    Some(match rows.get(ix)? {
                        Row::Heading(heading) => list_heading(*heading, theme),
                        // As the navigator's caption, its branches a step in under it: a
                        // click selects it and its chevron folds them away; folded, it names
                        // its top branch.
                        row @ Row::Stack(top, count, folded) => {
                            let selected = is_selected(branches, row);
                            let on = selected && solid;
                            let ink = |color| if on { theme.text_inverted } else { color };
                            let top_branch = branches.list.get(*top)?;
                            let top_ref = top_branch.ref_name.clone();
                            let fold = top_ref.clone();
                            let step = fold_button(
                                ("stack-fold", ix),
                                Some(!*folded),
                                ink(theme.text_3),
                                cx,
                                move |this, _, cx| this.toggle_stack(&fold, cx),
                            );
                            side_row(("stack-caption", ix), selected, solid, theme)
                                .pl(dp(2.))
                                .text_size(dp(12.))
                                .text_color(ink(theme.text_2))
                                .child(step)
                                .child(small_icon(Icon::Stack).text_color(ink(theme.text_3)))
                                .child(div().flex_none().child(format!("Stack of {count}")))
                                .when(*folded, |el| {
                                    el.child(
                                        div()
                                            .min_w_0()
                                            .truncate()
                                            .text_color(ink(theme.text_1))
                                            .child(title(top_branch).into_owned()),
                                    )
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.select_stack(top_ref.clone(), cx)
                                }))
                                .into_any_element()
                        }
                        row @ Row::Commit(branch_ix, commit_ix, stacked) => {
                            let ref_name = branches.list.get(*branch_ix)?.ref_name.clone();
                            let (_, commits) = branches.commits.get(&ref_name)?;
                            let commit = commits.get(*commit_ix)?;
                            let id = commit.id;
                            let label = crate::ui::commit_title(&commit.title);
                            let project = branches.project.clone();
                            // A picked one takes the others picked along.
                            let ids = branches.cherry_picks(id);
                            let draggable = !ids.is_empty();
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
                                .pl(dp(if *stacked { 38. } else { 24. }))
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
                                .child(div().flex_1().min_w_0().truncate().child(label))
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
                                .when(draggable, |el| {
                                    el.on_drag(payload, drag_preview(this.store.clone(), dragged))
                                })
                                .into_any_element()
                        }
                        row @ Row::Branch(branch_ix, stacked) => {
                            let branch = branches.list.get(*branch_ix)?;
                            let selected = is_selected(branches, row);
                            let ref_name = branch.ref_name.clone();
                            // The target has it all: landed, as a merged pull request shows.
                            let (glyph, glyph_color) = match &branch.review {
                                Some(review) => review::state_glyph(review.state, theme),
                                None if branch.ahead == Some(0) => {
                                    (Icon::BranchMerge, theme.purple)
                                }
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
                            let shown = title(branch);
                            let name = div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                // A branch's name is cut in the middle, as many share a start
                                // (origin/hotfix/…) and differ at the end; a title at its end.
                                .when(shown == shown_name(branch), |el| el.text_ellipsis_middle())
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(ink(title_color))
                                .child(shown.into_owned());
                            // The target has it all: said, as the navigator says it of a branch
                            // that landed.
                            let merged = (branch.ahead == Some(0)).then(|| {
                                format!("in {}", target.as_deref().unwrap_or("the target"))
                            });
                            let (project, menu_ref) = (branches.project.clone(), ref_name.clone());
                            side_row(("other-branch", ix), selected, solid, theme)
                                .pl(dp(if *stacked { 16. } else { 2. }))
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
                                // A click selects it; only the chevron, ⇧← and ⇧→ fold its commits,
                                // which are often not what's wanted.
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.select_other_branch(Some(ref_name.clone()), cx)
                                }))
                                .on_mouse_down(
                                    MouseButton::Right,
                                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                        this.select_other_branch(Some(menu_ref.clone()), cx);
                                        this.branch_menu(&project, &menu_ref, event.position, cx)
                                    }),
                                )
                                .into_any_element()
                        }
                    })
                },
            )
            .flex_1()
            .min_h(dp(LIST_MIN_HEIGHT))
            .into_any_element()
        };
        self.list_column(
            Self::step_branches,
            Some(Self::fold_branches),
            Some(Self::apply_selected_branch),
            cx,
        )
        .when(self.selected_branch_commit(project).is_some(), |el| {
            let project = project.clone();
            el.on_action(cx.listener(move |this, _: &CherryPick, window, cx| {
                this.copy_branch_commit(&project, window, cx)
            }))
        })
        .child(filter)
        .children(chips)
        .child(list)
        .child(people)
        .into_any_element()
    }

    /// Whether the filter menu's choices leave some branches out, as they don't at rest.
    fn menu_narrows(&self, branches: &Branches, cx: &App) -> bool {
        let prefs = &self.store.read(cx).prefs;
        branches.who.is_some()
            || branches.recency != Recency::Any
            || prefs.branches_status != BranchStatus::NotMerged
            || prefs.branches_local_only
    }

    /// The filter bar over the list, with a fetch button.
    fn branch_filter_bar(
        &self,
        project: &ProjectId,
        window: &Window,
        cx: &Context<Self>,
    ) -> gpui::Div {
        // Lit while the filters aren't as they are at rest.
        let narrowed = (self.branches_for(project)).is_some_and(|b| self.menu_narrows(b, cx));
        self.filter_bar(
            &self.branch_filter,
            "Branch filters",
            Self::branch_filter_items,
            narrowed,
            window,
            cx,
        )
        .children(self.fetch_button(project, cx))
    }

    /// The filters narrowing the list, a chip each that lets go of it; none at rest.
    fn branch_filter_chips(&self, cx: &Context<Self>) -> Option<gpui::Div> {
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
            .map(|(_, name)| {
                chip("branches-status", (*name).to_owned()).on_click(
                    cx.listener(|this, _, _, cx| this.filter_status(BranchStatus::NotMerged, cx)),
                )
            });
        let local = prefs.branches_local_only.then(|| {
            chip("branches-local", "Only local".to_owned()).on_click(cx.listener(
                |this, _, _, cx| {
                    (this.store).update(cx, |store, cx| {
                        store.set_prefs(|prefs| prefs.branches_local_only = false, cx)
                    });
                    cx.notify();
                },
            ))
        });
        // Not the person picked, whom the People card's Clear lets go of.
        let chips: Vec<_> = [status, local].into_iter().flatten().collect();
        (!chips.is_empty()).then(|| {
            h_flex()
                .flex_none()
                .flex_wrap()
                .gap(dp(4.))
                .px(dp(10.))
                .children(chips)
        })
    }

    /// What the list shows without a branch to list: that every one is applied, or that the
    /// filters hide them.
    fn no_branches(&self, branches: &Branches, theme: &Theme, cx: &Context<Self>) -> AnyElement {
        if branches.list.is_empty() {
            let empty = "Every branch is applied".to_owned();
            return list_empty(Illustration::Cactus, empty, "", theme, cx);
        }
        let query = self.branch_filter.read(cx).text();
        list_filtered_out(query, ("branch", "branches"), theme, cx)
    }

    /// Whose branches the list shows, how recently changed, and which by their pull request,
    /// each choice with how many it would show; and whether only local ones.
    fn branch_filter_items(&self, cx: &mut Context<Self>) -> Vec<MenuItem> {
        let Some(branches) = self.page_branches(cx) else {
            return Vec::new();
        };
        let prefs = &self.store.read(cx).prefs;
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
        // Each says how many picking them lists, not the People card's count of recent work.
        let anyone = self.kept_in(branches, &branches.list, Some(Facet::Who), cx);
        let theirs = |name: &String| {
            (anyone.iter().filter_map(|ix| branches.list.get(*ix)))
                .filter(|branch| branch.people.contains(name))
                .count()
        };
        let mut items = vec![person(None, "Anyone".to_owned(), anyone.len()).under("Person")];
        items.extend(
            offered.map(|(_, (name, ..))| person(Some(name.clone()), name.clone(), theirs(name))),
        );
        let today = upstream::day_of(crate::ui::now());
        let any_time = self.kept_in(branches, &branches.list, Some(Facet::Recency), cx);
        for (ix, recency) in Recency::ALL.into_iter().enumerate() {
            let n = (any_time.iter())
                .filter_map(|ix| branches.list.get(*ix))
                .filter(|branch| recency.keeps(branch.time, today))
                .count();
            let item = MenuItem::new(recency.label(), move |app, _, cx| {
                (app.detail).update(cx, |detail, cx| detail.filter_recency(recency, cx));
            })
            .ticked(branches.recency == recency)
            .detail(n.to_string());
            items.push(match ix {
                0 => item.under("Last activity"),
                _ => item,
            });
        }
        let statuses = self.status_counts(branches, cx).into_iter().zip(STATUSES);
        for (ix, ((status, n), (_, label))) in statuses.enumerate() {
            let item = MenuItem::new(label, move |app, _, cx| {
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
        items
    }

    /// Under the list, as the project's page has its own: whose the branches the filters keep
    /// are, most first, each a filter of its own.
    fn branch_people_card(
        &self,
        branches: &Branches,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let people = (self.branch_people(branches, cx).into_iter())
            .map(|(name, email, n)| (name.clone(), name, email, n))
            .collect();
        let counted = match branches.recency.days() {
            1 => "branches · today".to_owned(),
            days => format!("branches · {days} days"),
        };
        self.people_card(
            people,
            branches.who.as_deref(),
            (counted, "No branches changed then"),
            |this, who, cx| this.filter_person(who, cx),
            window,
            cx,
        )
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
            _ => (Icon::Branch, shown_name(branch).into_owned()),
        };
        let name = match branches.stack {
            true => (stack.iter().map(|b| shown_name(b)))
                .collect::<Vec<_>>()
                .join(" ← "),
            false => shown_name(branch).into_owned(),
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
                    .gap(dp(3.))
                    .child(icon(glyph).size(dp(14.)).text_color(color))
                    .child(format!("{state} #{}", review.number))
            });
        let commits = match (branch.ahead, branches.stack) {
            (Some(0), false) => format!("in {target}"),
            (_, true) => (stack.iter().map(|b| b.own_commits).sum::<Option<usize>>()).map_or_else(
                || "commits uncounted".to_owned(),
                |own| plural(own, "commit"),
            ),
            // The history read ends before where it forked, as in a shallow clone.
            (_, false) => (branch.own_commits).map_or_else(
                || "commits uncounted".to_owned(),
                |own| plural(own, "commit"),
            ),
        };
        let byline = (branch.author.as_ref())
            .zip(branch.time)
            .map(|(author, time)| {
                let email = branch.email.as_deref().unwrap_or_default();
                byline(author, email, branch.agent.as_ref(), time, theme).flex_none()
            });
        let description = (branch.description.clone()).filter(|_| !branches.stack);
        // What right-clicking its row offers, as a page's ⋯ does.
        let (p, r) = (project.clone(), branch.ref_name.clone());
        let menu = icon_button("branch-menu", Icon::Kebab).on_click(cx.listener(
            move |this, event: &ClickEvent, _, cx| this.branch_menu(&p, &r, event.position(), cx),
        ));
        // And the rule along its bottom, which the diff starts below, as a commit's message.
        let height = measured(&self.target_message_height, &self.app, |bounds| {
            f32::from(bounds.size.height) + 1.
        });
        Some(
            v_flex()
                .relative()
                .flex_none()
                .gap(dp(8.))
                .px(dp(16.))
                .py(dp(12.))
                .border_b_1()
                .border_color(theme.border_3)
                .child(
                    h_flex()
                        .justify_between()
                        .gap(dp(8.))
                        .child(page_title(glyph, theme).child(title_text(title.clone()).truncate()))
                        .child(menu),
                )
                .child(
                    meta_row(theme)
                        // Its name, unless the title already is.
                        .when(name != title, |el| {
                            el.child(
                                meta_item(Icon::Branch, theme)
                                    .min_w_0()
                                    .child(div().min_w_0().truncate().child(name)),
                            )
                        })
                        .children(review)
                        .child(meta_item(Icon::Commit, theme).flex_none().child(commits))
                        .children(byline),
                )
                .children(description.map(|description| {
                    div()
                        .text_size(dp(13.))
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
    let selected = |ix: usize| {
        (branches.list.get(ix)).is_some_and(|b| branches.selected.as_ref() == Some(&b.ref_name))
    };
    match *row {
        Row::Heading(_) => false,
        Row::Stack(top, ..) => selected(top) && branches.stack,
        Row::Branch(ix, _) => selected(ix) && !branches.stack && branches.commit.is_none(),
        Row::Commit(ix, commit, _) => {
            let commits = (branches.list.get(ix)).and_then(|b| branches.commits.get(&b.ref_name));
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
    let Some(ref_name) = branches.list.get(ix).map(|b| &b.ref_name) else {
        return false;
    };
    branches.selected.as_ref() == Some(ref_name)
        && (branches.commits.get(ref_name))
            .and_then(|(_, commits)| commits.get(commit))
            .is_some_and(|c| branches.picked.contains(&c.id))
}

/// `name`'s email, from any branch they made the newest commit of, filtered or not, applied
/// or not.
fn email_of(branches: &Branches, name: &str) -> String {
    (branches.list.iter().chain(&branches.applied))
        .find(|b| b.author.as_deref() == Some(name))
        .and_then(|b| b.email.clone())
        .unwrap_or_default()
}

/// What the list calls a branch, as the navigator does: its pull request's title once known,
/// else its name, a remote's as the remote names it (origin/…).
fn title(branch: &OtherBranch) -> Cow<'_, str> {
    (branch.review.as_ref())
        .map(|review| review.title.as_str())
        .filter(|title| !title.is_empty())
        .map_or_else(|| shown_name(branch), Cow::Borrowed)
}

/// A branch's name, a remote's as the remote names it (origin/…).
fn shown_name(branch: &OtherBranch) -> Cow<'_, str> {
    (branch.ref_name.as_bstr().strip_prefix(b"refs/remotes/"))
        .filter(|_| !branch.local)
        .map_or(Cow::Borrowed(branch.name.as_str()), |name| {
            name.to_str_lossy()
        })
}

/// Whether `status` keeps `branch`, by its pull request.
pub(super) fn status_keeps(status: BranchStatus, branch: &OtherBranch) -> bool {
    let state = branch.review.as_ref().map(|review| review.state);
    match status {
        BranchStatus::NotMerged => {
            !matches!(state, Some(ReviewState::Merged | ReviewState::Closed))
        }
        BranchStatus::Open => state == Some(ReviewState::Open),
        BranchStatus::Draft => state == Some(ReviewState::Draft),
        BranchStatus::Closed => state == Some(ReviewState::Closed),
        BranchStatus::NoPullRequest => state.is_none(),
        BranchStatus::Any => true,
    }
}
