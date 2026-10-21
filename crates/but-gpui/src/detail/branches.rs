//! The Branches page: branches outside the workspace, local ones first, then the ones only a
//! remote has, each with what it would bring in, ready to apply or delete. It opens on a table
//! of every branch, one line each, and opening one shows the list beside its diff.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui::{
    AnyElement, App, ClickEvent, Context, MouseButton, MouseDownEvent, Pixels, Point, SharedString,
    Window, div, prelude::*, px, relative,
};

use super::Detail;
use super::panes::{Crumb, list_empty, quiet, side_list, side_row};
use super::pull_request::{TABS_HEIGHT, tab};
use crate::app::{Collapse, Confirm, Expand, SelectNext, SelectPrevious};
use crate::backend::branches::{self, ApplyOutcome, OtherBranch};
use crate::backend::upstream::TargetCommit;
use crate::backend::{self, CommitId, DiffSource, ReviewState};
use crate::edits::CherryPick;
use crate::overlays::MenuItem;
use crate::state::{Cut, Overlay, Picks, Prefs, ProjectId, Selection};
use crate::theme::Theme;
use crate::transfer::drag_preview;
use crate::ui::illustration::Illustration;
use crate::ui::person::byline_avatar;
use crate::ui::text_input::TextInput;
use crate::ui::{
    Icon, ROW_HEIGHT, Tooltip, age_column, empty_state, faint, h_flex, icon_button, relative_time,
    review, section_label, small_icon, v_flex,
};

pub(super) struct Branches {
    project: ProjectId,
    /// The project's branches when the list loaded; a reload that changes none of them
    /// leaves the list as it is. `None` until it has loaded.
    seen: u64,
    list: Vec<OtherBranch>,
    /// The selected branch, by ref.
    selected: Option<String>,
    /// The branches whose commits show under them, by ref: folded until opened, as the
    /// navigator's are.
    open: HashSet<String>,
    /// The branch a click on its row opened, which a click on another's folds again, as the
    /// navigator does; one opened or folded by hand since stays as it's left.
    click_opened: Option<String>,
    /// The stacks whose branches are folded away under their caption, by their top branch.
    folded: HashSet<String>,
    /// Open branches' commits, by ref and what they were loaded for.
    commits: HashMap<String, (Loaded, Vec<TargetCommit>)>,
    /// One of the selected branch's commits, to see on its own and to copy into the workspace.
    commit: Option<CommitId>,
    /// Commits of the selected branch picked along with the selected one (⌘- or ⇧-click), to
    /// cherry-pick together.
    picked: Picks<CommitId>,
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

/// A line of the table: a heading over the branches changed in the last 30 days, or over the
/// older ones, with how many it heads; or a branch, by index into the list.
enum TableRow {
    Heading(&'static str, usize),
    Branch(usize),
}

/// How long ago a stack last changed for its branches to be listed under "Older".
const RECENT: i64 = 30 * 24 * 60 * 60;

/// The hover group of a table row, which shows its edit button.
const TABLE_ROW: &str = "branch-table-row";

/// A branch's tip and how many commits it has that the target doesn't: a fetch can land
/// some of its commits on the target without moving its tip.
type Loaded = (CommitId, Option<usize>);

/// A row of the list: a stack's caption, by its top branch's index, with how many of its
/// branches show and whether they're folded away; a branch by index into the list; or a
/// commit of an open one, by the branch's index and its own. A branch in a stack shown with
/// others, and its commits, sit a step in (`stacked`).
enum Row {
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
                // The newest, which the list shows first.
                let first = list.first();
                let was = previous.as_ref().and_then(|b| b.selected.clone());
                let selected = (was.clone())
                    .filter(|ref_name| list.iter().any(|b| b.ref_name == *ref_name))
                    .or_else(|| first.map(|b| b.ref_name.clone()));
                // The branch opened went, as an agent renamed or deleted it: back to the table,
                // rather than open another in its place.
                if was.is_some_and(|was| !list.iter().any(|b| b.ref_name == was)) {
                    this.branches_table = true;
                }
                // Which branches were open and their commits, of those still listed, and the
                // selected commit while its branch stays selected.
                let listed = |ref_name: &String| list.iter().any(|b| b.ref_name == *ref_name);
                let (open, click_opened, folded, commits, commit, picked) = match previous {
                    Some(b) => {
                        let same = b.selected == selected;
                        (
                            b.open.into_iter().filter(|r| listed(r)).collect(),
                            b.click_opened.filter(|r| listed(r)),
                            b.folded.into_iter().filter(|r| listed(r)).collect(),
                            b.commits.into_iter().filter(|(r, _)| listed(r)).collect(),
                            b.commit.filter(|_| same),
                            if same { b.picked } else { Picks::default() },
                        )
                    }
                    None => Default::default(),
                };
                this.branches = Some(Branches {
                    project,
                    seen,
                    list,
                    selected,
                    open,
                    click_opened,
                    folded,
                    commits,
                    commit,
                    picked,
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

    /// The branch whose diff shows; none while the table does.
    pub(super) fn selected_other_branch(&self, project: &ProjectId) -> Option<&OtherBranch> {
        if self.branches_table {
            return None;
        }
        self.branches_for(project)?.selected_branch()
    }

    /// Whether the Branches page shows its table of every branch rather than one's diff.
    pub(crate) fn showing_branches_table(&self, cx: &App) -> bool {
        self.branches_table && matches!(self.store.read(cx).selection, Some(Selection::Branches(_)))
    }

    /// Go back to the table of every branch, letting go of a selected commit.
    pub(super) fn show_branches_table(&mut self, cx: &mut Context<Self>) {
        self.close_branch(cx);
        // The diff beside the list goes with it.
        self.app.update(cx, |_, cx| cx.notify()).ok();
    }

    fn close_branch(&mut self, cx: &mut Context<Self>) {
        self.branches_table = true;
        if let Some(branches) = &mut self.branches {
            branches.commit = None;
            branches.picked.clear();
        }
        self.show_picked(cx);
    }

    /// Esc on the Branches page with a branch opened: back to the table, whether it did. The
    /// app, which Esc is handled in, redraws for the diff that goes.
    pub fn close_opened_branch(&mut self, cx: &mut Context<Self>) -> bool {
        let opened = !self.branches_table
            && matches!(self.store.read(cx).selection, Some(Selection::Branches(_)));
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

    /// Open branch `ref_name` from the table: the list, it selected there, beside its diff.
    fn open_from_table(&mut self, ref_name: String, window: &mut Window, cx: &mut Context<Self>) {
        self.branches_table = false;
        // It opens on its commits, as a merge does, which come before the pull request tab
        // the branch pages keep picked.
        self.commits_tab = true;
        window.focus(&self.side_focus, cx);
        self.select_other_branch(Some(ref_name), cx);
        self.app.update(cx, |_, cx| cx.notify()).ok();
    }

    /// Branch `ref_name` of `project`'s list, while it's still listed: what a menu opened on
    /// it acts on, even once a reload has selected another.
    fn listed_branch(&self, project: &ProjectId, ref_name: &str) -> Option<&OtherBranch> {
        self.branches_for(project)?.branch(ref_name)
    }

    pub(super) fn branches_source(&self, project: &ProjectId) -> Option<DiffSource> {
        if let Some(id) = self.selected_branch_commit(project) {
            return Some(DiffSource::Commit(id));
        }
        let branch = self.selected_other_branch(project)?;
        Some(DiffSource::Branch {
            tip: branch.tip,
            ahead: branch.ahead,
        })
    }

    fn selected_branch_commit(&self, project: &ProjectId) -> Option<CommitId> {
        self.branches_for(project)?.commit
    }

    fn selected_branch_commit_title(&self, project: &ProjectId) -> Option<String> {
        let branches = self.branches_for(project)?;
        let (_, commits) = branches.commits.get(branches.selected.as_ref()?)?;
        let commit = commits.iter().find(|c| Some(c.id) == branches.commit)?;
        Some(commit.title.clone())
    }

    /// Load the open branches' commits that aren't loaded for them as they are now. A selected
    /// commit its branch no longer has is let go.
    fn load_open_commits(&mut self, cx: &mut Context<Self>) {
        let table = self.branches_table;
        let Some(branches) = &self.branches else {
            return;
        };
        // And the opened branch's, which it lists beside the list; not the table's selection,
        // which nothing shows the commits of, as the keys step through it.
        let wanted: Vec<(String, Loaded)> = (branches.list.iter())
            .filter(|b| {
                branches.open.contains(&b.ref_name)
                    || (!table && branches.selected.as_ref() == Some(&b.ref_name))
            })
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
    fn toggle_stack(&mut self, top: &str, cx: &mut Context<Self>) {
        if let Some(branches) = &mut self.branches
            && !branches.folded.remove(top)
        {
            branches.folded.insert(top.to_owned());
        }
        cx.notify();
    }

    /// Show a branch's commits under it, or fold them away, as a navigator branch does.
    /// Folding it over its selected commit selects the branch.
    fn toggle_branch(&mut self, ref_name: &str, cx: &mut Context<Self>) {
        let Some(branches) = &mut self.branches else {
            return;
        };
        // By hand, it stays as it's left.
        if branches.click_opened.as_deref() == Some(ref_name) {
            branches.click_opened = None;
        }
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

    /// Open branch `ref_name`'s commits for a click on its row, and fold the ones the last
    /// click opened, as the navigator does: a click looks inside a branch, without the list
    /// growing by a branch each time.
    fn open_on_click(&mut self, ref_name: &str, cx: &mut Context<Self>) {
        let Some(branches) = &mut self.branches else {
            return;
        };
        if branches
            .branch(ref_name)
            .is_some_and(|b| b.own_commits == Some(0))
        {
            return;
        }
        if let Some(previous) = branches
            .click_opened
            .take_if(|previous| previous != ref_name)
        {
            branches.open.remove(&previous);
        }
        if branches.open.insert(ref_name.to_owned()) {
            branches.click_opened = Some(ref_name.to_owned());
            self.load_open_commits(cx);
        }
        cx.notify();
    }

    /// The branches the filter keeps, local ones first, under headings.
    fn branch_rows(&self, branches: &Branches, cx: &Context<Self>) -> Vec<Row> {
        let shown = self.shown_branches(branches, cx);
        let mut rows = Vec::new();
        // In the listing's order, as Lite has them: newest first, local and remote alike, a
        // stack's branches together under a caption while more than one of them shows.
        for stack in shown.chunk_by(|a, b| branches.list[*a].stack == branches.list[*b].stack) {
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

    /// The branches the filter keeps, by index into the list, folded away or not.
    fn shown_branches(&self, branches: &Branches, cx: &Context<Self>) -> Vec<usize> {
        let query = self.branch_filter.read(cx).text().trim().to_lowercase();
        let store = self.store.read(cx);
        let prefs = &store.prefs;
        // One being deleted leaves at once. As in Lite, one with no commits of its own only
        // when asked for, and a merged one too.
        let keeps = |branch: &OtherBranch| {
            !store.hides_branch(&branches.project, &branch.ref_name)
                && (prefs.branches_with_empty || branch.own_commits != Some(0))
                && (branch.local || !prefs.branches_local_only)
                && (prefs.branches_with_merged || !is_merged(branch))
                && (query.is_empty()
                    || branch.name.to_lowercase().contains(&query)
                    || (branch.review.as_ref())
                        .is_some_and(|review| review.title.to_lowercase().contains(&query))
                    || branch
                        .author
                        .as_ref()
                        .is_some_and(|author| author.to_lowercase().contains(&query)))
        };
        (0..branches.list.len())
            .filter(|ix| keeps(&branches.list[*ix]))
            .collect()
    }

    /// Select the first branch the filter shows when it hides the selected one, so the diff
    /// is of a branch in the list.
    pub(super) fn select_shown_branch(&mut self, cx: &mut Context<Self>) {
        let Some(branches) = &self.branches else {
            return;
        };
        // One folded away in its stack still counts.
        let selected =
            |ix: &usize| Some(&branches.list[*ix].ref_name) == branches.selected.as_ref();
        if self.shown_branches(branches, cx).iter().any(selected) {
            return;
        }
        let rows = self.branch_rows(branches, cx);
        // With none shown, none is selected, and nothing shows beside the list either.
        let first = rows.iter().find_map(|row| match row {
            Row::Branch(ix, _) => Some(branches.list[*ix].ref_name.clone()),
            _ => None,
        });
        if first != branches.selected {
            self.select_other_branch(first, cx);
        }
    }

    fn select_other_branch(&mut self, ref_name: Option<String>, cx: &mut Context<Self>) {
        self.message_open = false;
        if let Some(branches) = &mut self.branches {
            branches.selected = ref_name;
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

    /// ↓ or ↑ (`delta`) in the list: the next or previous branch or commit shown, kept in view.
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
        let next = match at {
            Some(at) => (0..rows.len() as isize)
                .map(|step| at as isize + delta * (step + 1))
                .take_while(|ix| (0..rows.len() as isize).contains(ix))
                .map(|ix| ix as usize)
                .find(|ix| !matches!(rows[*ix], Row::Stack(..))),
            None => rows.iter().position(|row| !matches!(row, Row::Stack(..))),
        };
        if let Some(next) = next {
            self.select_branch_row(&rows[next], cx);
            (self.branches_scroll).scroll_to_item(next, gpui::ScrollStrategy::Nearest);
        }
    }

    /// → in the list: open the selected branch, or step into its commits when it's open.
    /// ← folds it, or steps from one of its commits back up to it.
    fn fold_branches(&mut self, open: bool, cx: &mut Context<Self>) {
        let Some(branches) = &self.branches else {
            return;
        };
        let Some(branch) = branches.selected_branch() else {
            return;
        };
        let ref_name = branch.ref_name.clone();
        let is_open = branches.open.contains(&ref_name);
        match (open, branches.commit.is_some(), is_open) {
            (true, false, false) if branch.own_commits != Some(0) => {
                self.toggle_branch(&ref_name, cx)
            }
            (true, false, true) => self.step_branches(1, cx),
            (false, true, _) => self.select_other_branch(Some(ref_name), cx),
            (false, false, true) => self.toggle_branch(&ref_name, cx),
            _ => {}
        }
    }

    fn select_branch_row(&mut self, row: &Row, cx: &mut Context<Self>) {
        let Some(branches) = &self.branches else {
            return;
        };
        match *row {
            Row::Stack(..) => {}
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
        // By its name, as the table has none opened.
        let Some(branch) = self.listed_branch(project, ref_name) else {
            return;
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
            None => MenuItem::new("Apply to workspace", move |app, _, cx| {
                app.detail
                    .update(cx, |detail, cx| detail.apply_branch(&p, &r, cx))
            }),
        });
        if worktree.is_none() {
            let (p, r) = (project.clone(), ref_name.to_owned());
            items.push(MenuItem::new("Switch to branch", move |app, _, cx| {
                app.detail
                    .update(cx, |detail, cx| detail.switch_to_branch(&p, &r, cx))
            }));
        }
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
        self.app
            .update(cx, |app, cx| app.open_menu(at, items, cx))
            .ok();
    }

    /// The linked worktree that has `branch` checked out, if one does: applying or deleting it
    /// would pull it out from under that worktree.
    fn checked_out_in(
        &self,
        project: &ProjectId,
        branch: &OtherBranch,
        cx: &Context<Self>,
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

    /// Leave the workspace for just branch `ref_name`, checked out on its own.
    fn switch_to_branch(&mut self, project: &ProjectId, ref_name: &str, cx: &mut Context<Self>) {
        let Some(branch) = self.listed_branch(project, ref_name) else {
            return;
        };
        let (ref_name, name) = (branch.ref_name.clone(), branch.name.clone());
        let selected = project.clone();
        self.store.update(cx, |store, cx| {
            store.run(
                project,
                format!("Switch to {name}"),
                Overlay::default(),
                move |path, _| backend::switch_to_branch(path, &ref_name),
                move |store, local, cx| {
                    store.select(Some(Selection::Branch(selected, local)), cx);
                    store.snack_undoable(format!("Switched to {name}"), cx)
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

    /// The opened branch's own commits, newest first, once read for its tip.
    pub(super) fn opened_branch_commits(&self, project: &ProjectId) -> Option<&[TargetCommit]> {
        let branch = self.selected_other_branch(project)?;
        let branches = self.branches_for(project)?;
        let ((tip, _), _) = branches.commits.get(&branch.ref_name)?;
        (*tip == branch.tip).then(|| branches.listed_commits(&branch.ref_name))
    }

    /// Beside the list, the opened branch as a merge shows on the project's page: tabs for its
    /// commits, its changes and its pull request, and under them its commits and files, the
    /// window drawing the diff over the rest.
    pub(super) fn render_branch_beside(
        &self,
        project: &ProjectId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(branch) = self.selected_other_branch(project) else {
            return div().flex_1().into_any_element();
        };
        let review = branch.review.as_ref();
        let commits = self.showing_commits(cx);
        let reviewing = self.review_tab && review.is_some() && !commits;
        let below = match reviewing {
            true => self.render_review_tab(project, theme, cx),
            false => self.render_commits_and_files(
                self.render_branch_commit_list(project, theme, cx),
                theme,
                cx,
            ),
        };
        let changed = (self.shown_diff().map(|diff| diff.files.len())).filter(|n| *n > 0);
        let count = (self.opened_branch_commits(project).map(<[_]>::len)).or(branch.own_commits);
        let tabs = h_flex()
            .flex_none()
            .h(px(TABS_HEIGHT))
            .px(px(16.))
            .items_end()
            .border_b_1()
            .border_color(theme.border_3)
            .child(
                h_flex()
                    .flex_grow(1.)
                    .ml(px(-8.))
                    .child(
                        tab("tab-commits", "Commits", commits, theme)
                            .children(count.map(|n| faint(n.to_string(), theme)))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.set_list_tab(false, true, window, cx)
                            })),
                    )
                    .child(
                        tab("tab-changes", "Changes", !reviewing && !commits, theme)
                            .children(changed.map(|n| faint(n.to_string(), theme)))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.set_list_tab(false, false, window, cx)
                            })),
                    )
                    .when(review.is_some(), |el| {
                        el.child(
                            tab("tab-review", "Pull request", reviewing, theme).on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.set_list_tab(true, false, window, cx)
                                }),
                            ),
                        )
                    }),
            );
        // A commit picked shows its message over the tabs, as a merge's does.
        let message = (self.picked_branch_commit(project))
            .map(|commit| self.render_target_message(commit, theme, cx));
        v_flex()
            .relative()
            .flex_1()
            .h_full()
            .min_w(px(0.))
            .children(message)
            .child(tabs)
            .child(below)
            .child(self.measure_beside_width())
            .into_any_element()
    }

    /// The opened branch's own commits, under a row for all of them when there's more than
    /// one; a click shows one in the diff.
    fn render_branch_commit_list(
        &self,
        project: &ProjectId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(commits) = self.opened_branch_commits(project) else {
            return empty_state(None, "Loading…", "", theme, cx);
        };
        if commits.is_empty() {
            return empty_state(None, "No commits to list", "", theme, cx);
        }
        let project = project.clone();
        side_list(
            "branch-commits",
            // One commit is all there is: no row for all of them.
            commits.len() + usize::from(commits.len() > 1),
            &self.merged_scroll,
            "branch-commits-scrollbar",
            cx,
            move |this, ix, theme, _, cx| {
                let commits = this.opened_branch_commits(&project)?;
                let branches = this.branches_for(&project)?;
                let ref_name = branches.selected.clone()?;
                let pick = match (ix, commits.len()) {
                    (_, 1) => Some(commits.first()?),
                    (0, _) => None,
                    (ix, _) => Some(commits.get(ix - 1)?),
                };
                let id = pick.map(|c| c.id);
                let selected = branches.commit == id || commits.len() == 1;
                let row = side_row(("branch-commit", ix), selected, false, theme).on_click(
                    cx.listener(move |this, _, _, cx| match id {
                        Some(id) => this.select_branch_commit(&ref_name, id, cx),
                        None => this.select_other_branch(Some(ref_name.clone()), cx),
                    }),
                );
                Some(match pick {
                    None => row
                        .child(small_icon(Icon::Branch).text_color(theme.text_3))
                        .child(
                            div()
                                .flex_1()
                                .child(format!("All {} commits", commits.len())),
                        )
                        .into_any_element(),
                    Some(commit) => row
                        .child(small_icon(Icon::Commit).text_color(theme.text_3))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .truncate()
                                .child(SharedString::from(commit.title.clone())),
                        )
                        .child(quiet(age_column(Some(commit.time), theme), false, theme))
                        .into_any_element(),
                })
            },
        )
        .flex_1()
        .min_h(px(0.))
        .into_any_element()
    }

    /// The table's lines: the branches the filter keeps, in the listing's order, the stacks
    /// changed in the last 30 days first. A stack goes by its newest branch, as the listing
    /// orders them, so its branches stay together.
    fn table_rows(&self, branches: &Branches, cx: &Context<Self>) -> Vec<TableRow> {
        let mut newest: HashMap<usize, i64> = HashMap::new();
        for branch in &branches.list {
            if let Some(time) = branch.time {
                let at = newest.entry(branch.stack).or_insert(time);
                *at = (*at).max(time);
            }
        }
        let now = crate::ui::now();
        let (recent, older): (Vec<usize>, Vec<usize>) = (self.shown_branches(branches, cx))
            .into_iter()
            .partition(|ix| {
                (newest.get(&branches.list[*ix].stack)).is_some_and(|time| now - time < RECENT)
            });
        let mut rows = Vec::new();
        for (title, group) in [("Last 30 days", recent), ("Older", older)] {
            if !group.is_empty() {
                rows.push(TableRow::Heading(title, group.len()));
                rows.extend(group.into_iter().map(TableRow::Branch));
            }
        }
        rows
    }

    /// ↑ and ↓ in the table: the branch above or below the selected one.
    fn step_table(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(branches) = &self.branches else {
            return;
        };
        let rows = self.table_rows(branches, cx);
        let selected = |row: &TableRow| {
            matches!(*row, TableRow::Branch(ix)
                if branches.selected.as_ref() == Some(&branches.list[ix].ref_name))
        };
        let at = rows.iter().position(selected);
        let next = match at {
            Some(at) => (0..rows.len() as isize)
                .map(|step| at as isize + delta * (step + 1))
                .take_while(|ix| (0..rows.len() as isize).contains(ix))
                .map(|ix| ix as usize)
                .find(|ix| matches!(rows[*ix], TableRow::Branch(_))),
            None => rows
                .iter()
                .position(|row| matches!(row, TableRow::Branch(_))),
        };
        if let Some(next) = next
            && let TableRow::Branch(ix) = rows[next]
        {
            let ref_name = branches.list[ix].ref_name.clone();
            self.select_other_branch(Some(ref_name), cx);
            (self.branches_table_scroll).scroll_to_item(next, gpui::ScrollStrategy::Nearest);
        }
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
                app.open_prompt(
                    &title,
                    Some(
                        "Git keeps it as the branch's description, where git branch \
                         --edit-description does. Leave it empty to remove it."
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

    /// The table of every branch the filter keeps, one line each, as the page opens: what it's
    /// for, its pull request, its commits, who last worked on it and when. Opening one shows
    /// the list beside its diff.
    pub(super) fn render_branches_table(
        &self,
        project: &ProjectId,
        theme: &Theme,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(branches) = self.branches_for(project) else {
            return empty_state(None, "Loading branches…", "", theme, cx);
        };
        let prefs = &self.store.read(cx).prefs;
        let changed =
            prefs.branches_with_empty || prefs.branches_local_only || prefs.branches_with_merged;
        let filter = self
            .filter_bar(
                &self.branch_filter,
                "Branch filters",
                Self::branch_filters_menu,
                changed,
                window,
                cx,
            )
            .children(self.fetch_button(project, cx));
        let rows = self.table_rows(branches, cx);
        let body = if rows.is_empty() {
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
        } else {
            side_list(
                "branch-table",
                rows.len(),
                &self.branches_table_scroll,
                "branch-table-scrollbar",
                cx,
                move |this, ix, theme, window, cx| {
                    let branches = this.branches.as_ref()?;
                    Some(match rows.get(ix)? {
                        TableRow::Heading(title, count) => h_flex()
                            .h(px(ROW_HEIGHT))
                            .px(px(14.))
                            .gap(px(6.))
                            .items_end()
                            .pb(px(4.))
                            .child(section_label(*title, theme))
                            .child(faint(count.to_string(), theme))
                            .into_any_element(),
                        TableRow::Branch(branch_ix) => {
                            this.render_table_row(branches, ix, *branch_ix, theme, window, cx)
                        }
                    })
                },
            )
            .into_any_element()
        };
        v_flex()
            .size_full()
            .track_focus(&self.side_focus)
            .key_context("SideList")
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.step_table(1, cx)))
            .on_action(cx.listener(|this, _: &SelectPrevious, _, cx| this.step_table(-1, cx)))
            .on_action(cx.listener(|this, _: &Confirm, window, cx| {
                let selected = (this.branches.as_ref()).and_then(|b| b.selected.clone());
                if let Some(ref_name) = selected {
                    this.open_from_table(ref_name, window, cx);
                }
            }))
            .child(filter)
            .child(table_columns(
                // Inset as the rows are, so the headings sit over their columns.
                h_flex()
                    .h(px(28.))
                    .mx(px(6.))
                    .px(px(8.))
                    .border_b_1()
                    .border_color(theme.border_3)
                    .text_size(px(12.))
                    .text_color(theme.text_2),
                [
                    "Branch",
                    "Description",
                    "Pull request",
                    "Commits",
                    "Ahead",
                    "Last author",
                    "Updated",
                ]
                .map(|label| div().child(label).into_any_element()),
            ))
            .child(body)
            .into_any_element()
    }

    /// Branch `branch_ix` as a line of the table, line `ix`.
    fn render_table_row(
        &self,
        branches: &Branches,
        ix: usize,
        branch_ix: usize,
        theme: &Theme,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let branch = &branches.list[branch_ix];
        let selected = branches.selected.as_ref() == Some(&branch.ref_name);
        let solid = self.side_focus.is_focused(window);
        let on = selected && solid;
        let ink = |color| if on { theme.text_inverted } else { color };
        let ref_name = branch.ref_name.clone();
        let project = branches.project.clone();
        let shown_name = (branch.ref_name.strip_prefix("refs/remotes/"))
            .filter(|_| !branch.local)
            .unwrap_or(&branch.name);
        // Stacked on the branch listed after it in its stack.
        let below = (branches.list.get(branch_ix + 1)).filter(|b| b.stack == branch.stack);
        let name = h_flex()
            .gap(px(6.))
            .child(
                div()
                    .min_w(px(0.))
                    .truncate()
                    // Semibold UI type, as the navigator names branches.
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(ink(theme.text_1))
                    .child(SharedString::from(shown_name.to_owned())),
            )
            .when_some(below, |el, below| {
                el.child(
                    div()
                        .id(("branch-table-stacked", ix))
                        .flex_none()
                        .child(small_icon(Icon::Stack).text_color(ink(theme.text_3)))
                        .tooltip(Tooltip::text(format!("Stacked on {}", below.name))),
                )
            });
        // Its own description, else its pull request's title, else its newest commit's.
        let own = branch.description.as_ref();
        let text = (own.or(branch
            .review
            .as_ref()
            .map(|r| &r.title)
            .filter(|t| !t.is_empty())))
        .or(branch.subject.as_ref())
        .map(|text| text.lines().next().unwrap_or_default().to_owned());
        let edit = branch.local.then(|| {
            let (project, ref_name) = (project.clone(), ref_name.clone());
            div()
                .flex_none()
                .opacity(0.)
                .group_hover(TABLE_ROW, |style| style.opacity(1.))
                // Editing it doesn't open the branch.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    icon_button(("branch-table-describe", ix), Icon::Edit)
                        .small()
                        .inverted(on)
                        .tooltip("Edit description")
                        .on_click(window.listener_for(
                            &cx.entity(),
                            move |this, _: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                this.edit_branch_description(&project, &ref_name, window, cx)
                            },
                        )),
                )
        });
        // The button at the column's end, where it is on every line.
        let description = h_flex()
            .gap(px(4.))
            .justify_between()
            .child(
                div()
                    .min_w(px(0.))
                    .truncate()
                    .text_color(ink(match own {
                        Some(_) => theme.text_1,
                        None => theme.text_2,
                    }))
                    .child(text.unwrap_or_default()),
            )
            .children(edit);
        let review = match &branch.review {
            Some(review) => {
                let (glyph, color) = review::state_glyph(review.state, theme);
                h_flex()
                    .gap(px(4.))
                    .child(small_icon(glyph).text_color(ink(color)))
                    .child(review::state_label(review.state))
                    .child(
                        div()
                            .text_color(ink(theme.text_2))
                            .child(format!("#{}", review.number)),
                    )
            }
            None => h_flex()
                .text_color(ink(theme.text_2))
                .child(match branch.local {
                    true => "No pull request",
                    false => "Remote only",
                }),
        };
        let count = |n: Option<usize>| {
            div()
                .text_color(ink(theme.text_2))
                .child(n.map_or("—".to_owned(), |n| n.to_string()))
        };
        let author = h_flex()
            .gap(px(6.))
            .when_some(branch.author.as_ref(), |el, author| {
                el.child(byline_avatar(
                    author,
                    branch.email.as_deref().unwrap_or_default(),
                    branch.agent.as_ref(),
                    14.,
                    theme,
                ))
                .child(div().min_w(px(0.)).truncate().child(author.clone()))
            });
        let updated = div()
            .text_color(ink(theme.text_2))
            .child(branch.time.map(relative_time).unwrap_or_default());
        let open_ref = ref_name.clone();
        table_columns(
            side_row(("branch-table-row", ix), selected, solid, theme).group(TABLE_ROW),
            [
                name.into_any_element(),
                description.into_any_element(),
                review.into_any_element(),
                count(branch.own_commits).into_any_element(),
                count(branch.ahead).into_any_element(),
                author.into_any_element(),
                updated.into_any_element(),
            ],
        )
        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
            this.open_from_table(open_ref.clone(), window, cx)
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                window.focus(&this.side_focus, cx);
                this.branch_menu(&project, &ref_name, event.position, cx)
            }),
        )
        .into_any_element()
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
        let prefs = &self.store.read(cx).prefs;
        // Lit while the filters aren't as they are at rest, as Lite counts them.
        let changed =
            prefs.branches_with_empty || prefs.branches_local_only || prefs.branches_with_merged;
        let filter = self
            .filter_bar(
                &self.branch_filter,
                "Branch filters",
                Self::branch_filters_menu,
                changed,
                window,
                cx,
            )
            .children(self.fetch_button(project, cx));
        let rows = self.branch_rows(branches, cx);
        let list = if rows.is_empty() {
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
                        // As the navigator's caption, its branches a step in under it. A
                        // click folds them away; folded, it names its top branch.
                        Row::Stack(top, count, folded) => {
                            let top_ref = branches.list[*top].ref_name.clone();
                            let chevron = match folded {
                                true => Icon::ChevronRight,
                                false => Icon::ChevronDown,
                            };
                            h_flex()
                                .id(("stack-caption", ix))
                                .h(px(ROW_HEIGHT))
                                .mx(px(6.))
                                .pl(px(2.))
                                .pr(px(8.))
                                .gap(px(8.))
                                .rounded(px(4.))
                                .hover(|el| el.bg(theme.hover()))
                                .text_size(px(12.))
                                .text_color(theme.text_2)
                                .child(
                                    div()
                                        .flex_none()
                                        .w(px(14.))
                                        .flex()
                                        .justify_center()
                                        .child(small_icon(chevron).text_color(theme.text_3)),
                                )
                                .child(small_icon(Icon::Stack).text_color(theme.text_3))
                                .child(div().flex_none().child(format!("Stack of {count}")))
                                .when(*folded, |el| {
                                    el.child(
                                        div()
                                            .min_w(px(0.))
                                            .truncate()
                                            .text_color(theme.text_1)
                                            .child(label(&branches.list[*top]).to_owned()),
                                    )
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.toggle_stack(&top_ref, cx)
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
                                .aria_label(label.clone())
                                .child(small_icon(Icon::Commit).text_color(match on {
                                    true => theme.text_inverted,
                                    false => theme.text_3,
                                }))
                                .child(div().flex_1().min_w(px(0.)).truncate().child(label.clone()))
                                // Who wrote it and when, in the branch rows' columns, so the
                                // two line up.
                                .child(
                                    div()
                                        .id(("other-commit-author", ix))
                                        .flex_none()
                                        .child(byline_avatar(
                                            &commit.author,
                                            &commit.email,
                                            commit.agent.as_ref(),
                                            14.,
                                            theme,
                                        ))
                                        .tooltip(Tooltip::text(match &commit.agent {
                                            Some(agent) => format!(
                                                "Committed by {}, written by {}",
                                                commit.author, agent.name
                                            ),
                                            None => format!("Authored by {}", commit.author),
                                        })),
                                )
                                .child(quiet(age_column(Some(commit.time), theme), on, theme))
                                .on_click(cx.listener(
                                    move |this, event: &ClickEvent, window, cx| {
                                        window.focus(&this.side_focus, cx);
                                        let held = event.modifiers();
                                        match held.secondary() || held.shift {
                                            true => this
                                                .pick_branch_commit(&ref_name, id, held.shift, cx),
                                            false => this.select_branch_commit(&ref_name, id, cx),
                                        }
                                    },
                                ))
                                .on_mouse_down(
                                    MouseButton::Right,
                                    cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                        window.focus(&this.side_focus, cx);
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
                            let merged = (branch.ahead == Some(0)).then(|| {
                                let ws = this.store.read(cx).workspace(&branches.project);
                                let target = ws.and_then(|ws| ws.target.as_ref());
                                format!("in {}", target.map_or("the target", |t| &t.name))
                            });
                            // As the navigator names it: a pull request by its title, the
                            // branch's name and number on hover.
                            let (glyph, glyph_color) = match &branch.review {
                                Some(review) => review::state_glyph(review.state, theme),
                                None => (Icon::Branch, theme.text_2),
                            };
                            let on = selected && solid;
                            let ink = |color| if on { theme.text_inverted } else { color };
                            // As in the navigator: a chevron folds its commits away, and a
                            // branch with none to show has a dot there instead.
                            let step = div()
                                .id(("other-branch-fold", ix))
                                .flex_none()
                                .w(px(14.))
                                .h(px(16.))
                                .flex()
                                .items_center()
                                .justify_center();
                            let step = match branch.own_commits == Some(0) {
                                true => step.child(
                                    small_icon(Icon::BulletDisc).text_color(ink(theme.text_3)),
                                ),
                                false => {
                                    let open = branches.open.contains(&ref_name);
                                    let fold = ref_name.clone();
                                    step.rounded(px(3.))
                                        .hover(|el| el.bg(theme.hover()))
                                        .child(
                                            small_icon(match open {
                                                true => Icon::ChevronDown,
                                                false => Icon::ChevronRight,
                                            })
                                            .text_color(ink(theme.text_3)),
                                        )
                                        // Folding the branch doesn't select it.
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation()
                                        })
                                        .on_click(cx.listener(
                                            move |this, _: &ClickEvent, _, cx| {
                                                cx.stop_propagation();
                                                this.toggle_branch(&fold, cx)
                                            },
                                        ))
                                }
                            };
                            let shown_name = (branch.ref_name.strip_prefix("refs/remotes/"))
                                .filter(|_| !branch.local)
                                .unwrap_or(&branch.name);
                            let name = div()
                                .id(("other-branch-name", ix))
                                .flex_1()
                                .min_w(px(0.))
                                .truncate()
                                .font_weight(gpui::FontWeight::SEMIBOLD);
                            let name = match &branch.review {
                                Some(review) if !review.title.is_empty() => name
                                    .text_color(ink(review::title_color(review.state, theme)))
                                    .child(SharedString::from(review.title.clone())),
                                // A branch only a remote has, by the remote's name for it.
                                _ => name
                                    .text_color(ink(if merged.is_some() {
                                        theme.text_2
                                    } else {
                                        theme.text_1
                                    }))
                                    .child(SharedString::from(shown_name.to_owned())),
                            };
                            let tip = (branch.review.as_ref())
                                .map(|review| format!("{shown_name} · #{}", review.number));
                            let (project, menu_ref) = (branches.project.clone(), ref_name.clone());
                            side_row(("other-branch", ix), selected, solid, theme)
                                .pl(px(if *stacked { 16. } else { 2. }))
                                .child(step)
                                .child(small_icon(glyph).text_color(ink(glyph_color)))
                                .child(
                                    name.when_some(tip, |el, tip| el.tooltip(Tooltip::text(tip))),
                                )
                                .when_some(merged, |el, merged| {
                                    el.child(quiet(faint(merged, theme), on, theme))
                                })
                                // Who made its newest commit, to pick out a teammate's at a
                                // glance; their name on hover.
                                .when_some(branch.author.as_ref(), |el, author| {
                                    el.child(
                                        div()
                                            .id(("other-branch-author", ix))
                                            .flex_none()
                                            .child(byline_avatar(
                                                author,
                                                branch.email.as_deref().unwrap_or_default(),
                                                branch.agent.as_ref(),
                                                14.,
                                                theme,
                                            ))
                                            .tooltip(Tooltip::text(match &branch.agent {
                                                Some(agent) => format!(
                                                    "Last commit by {author}, written by {}",
                                                    agent.name
                                                ),
                                                None => format!("Last commit by {author}"),
                                            })),
                                    )
                                })
                                // Its slot kept for an author without a time, so the
                                // avatars line up.
                                .when(branch.time.is_some() || branch.author.is_some(), |el| {
                                    el.child(quiet(age_column(branch.time, theme), on, theme))
                                })
                                // A click selects it and looks inside, opening its commits; the
                                // chevron, ← and → fold them.
                                .on_click(cx.listener(
                                    move |this, event: &ClickEvent, window, cx| {
                                        window.focus(&this.side_focus, cx);
                                        if event.click_count() == 1 {
                                            this.select_other_branch(Some(ref_name.clone()), cx);
                                            this.open_on_click(&ref_name, cx);
                                        }
                                    },
                                ))
                                .on_mouse_down(
                                    MouseButton::Right,
                                    cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                        window.focus(&this.side_focus, cx);
                                        this.branch_menu(&project, &menu_ref, event.position, cx)
                                    }),
                                )
                                .into_any_element()
                        }
                    })
                },
            )
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
            .child(list)
            .into_any_element()
    }

    /// Lite's branch filters: branches with no commits of their own, left out at rest, and
    /// the ones only a remote has; and merged ones, left out at rest too.
    fn branch_filters_menu(&self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let prefs = &self.store.read(cx).prefs;
        let toggle = |label: &str, ticked: bool, update: fn(&mut Prefs)| {
            MenuItem::new(label.to_owned(), move |app, _, cx| {
                app.store
                    .update(cx, |store, cx| store.set_prefs(update, cx));
                // The selected branch may have just been filtered out.
                app.detail
                    .update(cx, |detail, cx| detail.select_shown_branch(cx));
            })
            .ticked(ticked)
        };
        let items = vec![
            toggle(
                "Include empty branches",
                prefs.branches_with_empty,
                |prefs| prefs.branches_with_empty = !prefs.branches_with_empty,
            ),
            toggle("Only local branches", prefs.branches_local_only, |prefs| {
                prefs.branches_local_only = !prefs.branches_local_only
            }),
            toggle(
                "Show merged branches",
                prefs.branches_with_merged,
                |prefs| prefs.branches_with_merged = !prefs.branches_with_merged,
            ),
        ];
        (self.app)
            .update(cx, |app, cx| app.open_menu(position, items, cx))
            .ok();
    }

    /// The page's crumbs after "Branches": the selected branch, and the commit of it selected.
    pub(super) fn branches_crumbs(&self, project: &ProjectId, crumbs: &mut Vec<Crumb>) {
        let Some(branch) = self.selected_other_branch(project) else {
            return;
        };
        crumbs.push((branch.name.clone().into(), None));
        if let Some(title) = self.selected_branch_commit_title(project) {
            crumbs.push((title.into(), None));
        }
    }
}

/// `row` with `cells` laid out as the table's columns: the branch and its description share
/// what the others leave, the description taking more.
fn table_columns<E: ParentElement + Styled>(row: E, cells: [AnyElement; 7]) -> E {
    let widths = [
        None,
        None,
        Some(140.),
        Some(64.),
        Some(56.),
        Some(160.),
        Some(72.),
    ];
    let row = row.gap(px(12.)).items_center();
    cells
        .into_iter()
        .zip(widths)
        .enumerate()
        .fold(row, |row, (ix, (cell, width))| {
            let column = div().min_w(px(0.)).overflow_hidden().child(cell);
            row.child(match (ix, width) {
                (0, _) => column.flex_none().w(relative(0.22)),
                (_, None) => column.flex_grow(1.).flex_shrink(1.).flex_basis(px(0.)),
                (_, Some(width)) => column.flex_none().w(px(width)),
            })
        })
}

/// Whether `row` is what's selected: the selected commit, or else the selected branch.
fn is_selected(branches: &Branches, row: &Row) -> bool {
    let selected = |ix: usize| branches.selected.as_ref() == Some(&branches.list[ix].ref_name);
    match *row {
        Row::Stack(..) => false,
        Row::Branch(ix, _) => selected(ix) && branches.commit.is_none(),
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

/// What the list calls a branch: its pull request's title once known, else its name.
fn label(branch: &OtherBranch) -> &str {
    (branch.review.as_ref())
        .map(|review| review.title.as_str())
        .filter(|title| !title.is_empty())
        .unwrap_or(&branch.name)
}

/// Whether `branch`'s pull request was merged. One the target already has, as merged without
/// a forge, counts no commits of its own and goes with the empty ones.
fn is_merged(branch: &OtherBranch) -> bool {
    (branch.review.as_ref()).is_some_and(|review| review.state == ReviewState::Merged)
}
