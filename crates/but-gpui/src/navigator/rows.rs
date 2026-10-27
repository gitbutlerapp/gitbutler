//! The navigator's tree of rows as it's built, before any is drawn: every card's
//! projects, worktrees, stacks, branches and commits, and under the filter, what it finds.

use std::time::Duration;

use gpui::{App, SharedString};

use super::{ClickOpened, Navigator, STEP, upstream_chevrons};
use crate::backend::{Branch, Checkout, FullName, Lane, Workspace, Worktree};
use crate::edits::BranchEdits;
use crate::state::{Cut, NewAt, ProjectEntry, ProjectId, Section, Selection, Store};

/// Commits shown per branch before folding the rest into "N more".
const FOLD_AFTER: usize = 8;

/// How recently a project was last active for "Active in the last 7 days" to keep it.
const ACTIVE_WITHIN: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Rows in display order. Only [`Row::Item`] can be selected.
#[derive(PartialEq)]
pub(super) enum Row {
    /// The space between two cards.
    Gap,
    /// Whatever the [`Selection`] names: a project, a worktree, a stack, a branch, a commit,
    /// one of a project's pages, a section's heading, or one of these on another machine.
    Item(Selection),
    Note(SharedString, bool),
    /// A branch's commits folded away, and how many.
    More(ProjectId, Option<FullName>, usize),
    /// The end of a branch's commits unfolded, which folds them again.
    Fewer(ProjectId, Option<FullName>),
    /// A commit a mutation is making, by its title, and whether it's been long enough to
    /// show it's still on its way.
    Pending(SharedString, bool),
    /// Where a commit of the project being dragged lands in a new branch of its own, or a
    /// stacked branch in a stack of its own.
    NewBranch(ProjectId),
    /// The line after a worktree's section.
    WorktreeDivider,
    /// After the target's new commits listed, how many more its page lists.
    UpstreamMore(ProjectId, usize),
    /// In a linked worktree's section, a branch its lane rests on, by the row it selects, with
    /// the checkout it's in on the first of each lane.
    RestsOn(Selection, Option<Checkout>),
}

/// The rows as they're built, each with how far in it sits: a worktree's section and a
/// stack's branches step in from what holds them. Each also has the guides it draws, as
/// but.dev's sidebar does: a line down from under the chevron of each row it's under.
#[derive(Default)]
pub(super) struct Rows {
    pub(super) rows: Vec<Row>,
    pub(super) insets: Vec<f32>,
    pub(super) guides: Vec<Vec<f32>>,
    /// What the rows pushed next sit in by.
    inset: f32,
    /// Where the guides of the rows pushed next run.
    open: Vec<f32>,
    /// What the rows sat in by before each guide in `open` began, which closing it restores.
    outer: Vec<f32>,
    /// The first item the filter found, which Enter in it selects.
    pub(super) first_hit: Option<Selection>,
    pub(super) drawer: Drawer,
}

/// The projects not watched, in the drawer under the cards: their rows while it's open, how
/// many it lists, and how many there are, filters aside. With no cards it's always open.
#[derive(Default)]
pub(super) struct Drawer {
    pub(super) rows: Vec<Row>,
    pub(super) listed: usize,
    pub(super) total: usize,
}

/// Where an item row's content starts in its card: the row's margin, 3, and its padding, 8,
/// as [`RowStyle::row`](super::RowStyle::row) and [`section_heading`](super::section_heading)
/// set them.
pub(super) const ROW_LEFT: f32 = 3. + 8.;

/// How far in from [`ROW_LEFT`] a commit sits: under its branch's glyph, its title under the
/// branch's name.
pub(super) const COMMIT_INDENT: f32 = 30.;

/// Where a chevron's middle sits in a row set in by nothing: the row's left edge, the indent
/// of a row with a chevron, and half the chevron's step.
pub(super) const GUIDE_AT: f32 = ROW_LEFT + 6. + 7.;

impl Rows {
    pub(super) fn push(&mut self, row: Row) {
        self.rows.push(row);
        self.insets.push(self.inset);
        self.guides.push(self.open.clone());
    }

    /// Set the rows pushed until [`Self::close`] under the row just pushed, a guide running
    /// down beside them from under its chevron.
    pub(super) fn open(&mut self) {
        self.open_at(GUIDE_AT);
    }

    /// As [`Self::open`], for a row whose chevron's middle is `at` instead.
    fn open_at(&mut self, at: f32) {
        let under = self.insets.last().copied().unwrap_or_default();
        self.open.push(at + under);
        self.outer.push(self.inset);
    }

    /// As [`Self::open`], with the rows stepped in `by` further.
    pub(super) fn open_in(&mut self, by: f32) {
        self.open();
        self.inset += by;
    }

    pub(super) fn close(&mut self) {
        self.open.pop();
        self.inset = self.outer.pop().unwrap_or_default();
    }

    /// Rule what follows off from the rows above with a line, unless one already does.
    pub(super) fn rule_off(&mut self) {
        if !matches!(self.rows.last(), Some(Row::WorktreeDivider)) {
            self.push(Row::WorktreeDivider);
        }
    }

    /// Push `item`, the filter's first hit if it's `hit` and none came before it.
    pub(super) fn push_found(&mut self, item: Selection, hit: bool) {
        if hit && self.first_hit.is_none() {
            self.first_hit = Some(item.clone());
        }
        self.push(Row::Item(item));
    }

    /// Take back the rows pushed since there were `len`.
    pub(super) fn truncate(&mut self, len: usize) {
        self.rows.truncate(len);
        self.insets.truncate(len);
        self.guides.truncate(len);
    }
}

impl Extend<Row> for Rows {
    fn extend<I: IntoIterator<Item = Row>>(&mut self, rows: I) {
        rows.into_iter().for_each(|row| self.push(row));
    }
}

impl Navigator {
    /// Rows of every card, with a gap between two cards, and of the drawer. Watched projects
    /// get a card each; the rest wait in the drawer.
    pub(super) fn build_rows(&self, cx: &App) -> Rows {
        let terms = self.filter_terms(cx);
        if !terms.is_empty() {
            return self.build_filtered_rows(&terms, cx);
        }
        let store = self.store.read(cx);
        let mut rows = Rows::default();
        for entry in self.watched(store) {
            if !rows.rows.is_empty() {
                rows.push(Row::Gap);
            }
            let id = &entry.id;
            rows.push(Row::Item(Selection::Project(id.clone())));
            if store.project_folded(id) {
                continue;
            }
            let Some(workspace) = &entry.workspace else {
                rows.push(match &entry.error {
                    Some(err) => Row::Note(err.clone(), true),
                    None => Row::Note("Loading…".into(), false),
                });
                continue;
            };
            if let Some(err) = &entry.error {
                rows.push(Row::Note(err.clone(), true));
            }
            // The target's new commits are a section of their own, as Network worktrees is,
            // ruled off from the worktrees.
            let before = rows.rows.len();
            self.upstream_rows(store, workspace, id, None, &mut rows);
            if rows.rows.len() > before {
                rows.push(Row::WorktreeDivider);
            }
            // Nothing but the edit goes on while a commit is checked out, its files listed
            // where the uncommitted ones are.
            if workspace.editing.is_some() {
                rows.push(Row::Item(Selection::Uncommitted(id.clone())));
                continue;
            }
            // The main worktree's heading folds its branches away, which step in under it; with
            // linked worktrees each is a section too, ruled off from the next.
            let headed = !workspace.worktrees.is_empty();
            let selection = store.selection.as_ref().filter(|s| s.project() == id);
            let main_selected =
                selection.is_some_and(|s| within(s, workspace.branches_in(&Checkout::Main)));
            let main_open = self.worktree_open(id, Checkout::Main, main_selected);
            // Always listed, with nothing uncommitted too, so where to commit is always there.
            rows.push(Row::Item(Selection::Uncommitted(id.clone())));
            rows.open_in(STEP);
            if main_open {
                // A stack being unapplied leaves at once.
                let stacks = workspace.lanes_in(&Checkout::Main).filter(|stack| {
                    !(stack.branches.iter())
                        .any(|b| (b.ref_name.as_ref()).is_some_and(|r| store.hides_branch(id, r)))
                });
                // Whether a branch is checked out on its own or several share the workspace,
                // they're listed alike; with none, as on the default branch, there's nothing to
                // list, and the next commit starts one.
                for stack in stacks {
                    self.lane_rows(store, id, &stack.branches, &mut rows);
                }
            }
            rows.close();
            if headed {
                rows.push(Row::WorktreeDivider);
                let on_branches = |wt: &Worktree| {
                    let checkout = Checkout::Linked(wt.name.clone());
                    selection.is_some_and(|s| within(s, workspace.branches_in(&checkout)))
                };
                // The inactive ones wait under a heading of their own, folded unless opened or
                // the selection's in one.
                let (inactive, active): (Vec<_>, Vec<_>) = (self.worktrees(id, workspace))
                    .into_iter()
                    .filter(|wt| !store.hides_worktree(id, wt.name.as_ref()))
                    .partition(|wt| wt.is_inactive());
                for wt in active {
                    self.worktree_rows(store, id, wt, on_branches(wt), &mut rows);
                }
                if !inactive.is_empty() {
                    rows.push(Row::Item(Selection::Section(id.clone(), Section::Inactive)));
                    if inactive_open(store, id, &inactive) {
                        for wt in inactive {
                            self.worktree_rows(store, id, wt, on_branches(wt), &mut rows);
                        }
                    }
                }
            }
            // Its checkouts on the user's other machines, as worktrees: none while signed out.
            self.mesh_rows(store, id, &mut rows);
            // After the worktrees, so a drag starting in one doesn't push its rows down. Outside
            // the workspace there's nowhere to put a branch of its own.
            if workspace.managed && lands_apart(store.dragging.as_ref(), id, workspace) {
                rows.push(Row::NewBranch(id.clone()));
            }
            // The pages are another kind of row than the worktrees', so a line rules them off,
            // as the last worktree's does when there are linked ones.
            rows.rule_off();
            rows.push(Row::Item(Selection::Branches(id.clone())));
            rows.push(Row::Item(Selection::History(id.clone())));
        }
        let others: Vec<&ProjectEntry> = (store.projects.iter())
            .filter(|entry| !entry.expanded)
            .collect();
        let prefs = &store.prefs;
        let selected = |entry: &ProjectEntry| {
            store.selection.as_ref() == Some(&Selection::Project(entry.id.clone()))
        };
        let mut listed: Vec<(&ProjectEntry, Option<i64>)> = (others.iter())
            .map(|entry| (*entry, last_active(entry)))
            .filter(|(entry, at)| {
                !prefs.active_projects_only || at.is_some_and(active_lately) || selected(entry)
            })
            .collect();
        // Stable, so projects active at the same moment stay by name.
        if prefs.projects_by_activity {
            listed.sort_by_key(|(_, at)| std::cmp::Reverse(*at));
        }
        rows.drawer.listed = listed.len();
        rows.drawer.total = others.len();
        if prefs.projects_folded && !rows.rows.is_empty() {
            return rows;
        }
        rows.drawer.rows = (listed.iter())
            .map(|(entry, _)| Row::Item(Selection::Project(entry.id.clone())))
            .collect();
        if listed.is_empty() && !others.is_empty() {
            (rows.drawer.rows).push(Row::Note("None active in the last 7 days".into(), false));
        }
        rows
    }

    /// The rows while the filter has words in it: every project, worktree, branch and commit
    /// that matches, under what holds it, folded or not. One matches when each word is in
    /// its name or in the name of what holds it, and one at least is in its own, so "shop
    /// cart" finds shop's cart branch, not all of shop. A branch matches by its name or its
    /// pull request's title, a commit by its title, a project not expanded by its name.
    fn build_filtered_rows(&self, terms: &[String], cx: &App) -> Rows {
        let store = self.store.read(cx);
        let hit = |held, own| matches(terms, held, own);
        let mut rows = Rows::default();
        for entry in self.watched(store) {
            let id = &entry.id;
            let start = rows.rows.len();
            if start > 0 {
                rows.push(Row::Gap);
            }
            let own = found(terms, &[&entry.project.name]);
            rows.push_found(Selection::Project(id.clone()), hit(0, own));
            let project_end = rows.rows.len();
            let workspace = (entry.workspace.as_ref()).filter(|ws| ws.editing.is_none());
            if let Some(workspace) = workspace {
                // The main worktree's branches under its heading.
                let main = rows.rows.len();
                rows.push(Row::Item(Selection::Uncommitted(id.clone())));
                rows.open_in(STEP);
                let heading_end = rows.rows.len();
                for stack in workspace.lanes_in(&Checkout::Main) {
                    self.filtered_lane(store, id, &stack.branches, terms, own, &mut rows);
                }
                rows.close();
                if rows.rows.len() == heading_end {
                    rows.truncate(main);
                }
                let worktrees = (self.worktrees(id, workspace).into_iter())
                    .filter(|wt| !store.hides_worktree(id, wt.name.as_ref()));
                for wt in worktrees {
                    let wt_start = rows.rows.len();
                    let wt_own = found(terms, &[wt.name.to_string().as_str()]);
                    let item = Selection::Worktree(id.clone(), wt.name.clone());
                    rows.push_found(item, hit(own, wt_own));
                    rows.open_in(STEP);
                    let lane = workspace.worktree_lane(wt.name.as_ref());
                    let branches = lane.map_or(&[][..], |lane| &lane.branches);
                    self.filtered_lane(store, id, branches, terms, own | wt_own, &mut rows);
                    rows.close();
                    if rows.rows.len() == wt_start + 1 && !hit(own, wt_own) {
                        rows.truncate(wt_start);
                    }
                }
            }
            self.filtered_mesh_rows(store, id, terms, own, &mut rows);
            if rows.rows.len() == project_end && !hit(0, own) {
                rows.truncate(start);
            }
        }
        let others = (store.projects.iter()).filter(|entry| !entry.expanded);
        let total = others.clone().count();
        let listed: Vec<Row> = others
            .filter(|entry| hit(0, found(terms, &[&entry.project.name])))
            .map(|entry| Row::Item(Selection::Project(entry.id.clone())))
            .collect();
        if rows.first_hit.is_none()
            && let Some(Row::Item(first)) = listed.first()
        {
            rows.first_hit = Some(first.clone());
        }
        rows.drawer = Drawer {
            listed: listed.len(),
            total,
            rows: listed,
        };
        if rows.rows.is_empty() && rows.drawer.rows.is_empty() {
            rows.push(Row::Note("Nothing matches".into(), false));
        }
        rows
    }

    /// [`Self::lane_rows`] under the filter, with `held` the words found in what holds the
    /// lane: the branches that match or have commits that do, with those commits, under
    /// their stack's caption when there are two or more.
    fn filtered_lane(
        &self,
        store: &Store,
        id: &ProjectId,
        branches: &[Branch],
        terms: &[String],
        held: u64,
        rows: &mut Rows,
    ) {
        // A stack being unapplied leaves at once.
        if (branches.iter())
            .any(|b| (b.ref_name.as_ref()).is_some_and(|r| store.hides_branch(id, r)))
        {
            return;
        }
        let branches = &*store.after_deletes(id, branches);
        let start = rows.rows.len();
        let stacked = branches.len() > 1;
        if let [_, .., bottom] = branches {
            rows.push(Row::Item(Selection::Stack(
                id.clone(),
                bottom.ref_name.clone(),
            )));
            rows.open_in(STEP);
        }
        let caption_end = rows.rows.len();
        for branch in branches {
            let own = found(
                terms,
                &[store.name_of(id, branch), store.label_of(id, branch)],
            );
            let commits: Vec<_> = (branch.commits.iter())
                .filter(|commit| {
                    !store.hides_commit(id, &commit.key)
                        && matches(terms, held | own, found(terms, &[&commit.title]))
                })
                .collect();
            let branch_hit = matches(terms, held, own);
            if !branch_hit && commits.is_empty() {
                continue;
            }
            rows.push_found(
                Selection::Branch(id.clone(), branch.ref_name.clone()),
                branch_hit,
            );
            rows.open();
            for commit in commits {
                rows.push_found(Selection::Commit(id.clone(), commit.id), true);
            }
            rows.close();
        }
        if stacked {
            rows.close();
        }
        if rows.rows.len() == caption_end {
            rows.truncate(start);
        }
    }

    /// A stack's or a linked worktree's branches: over two or more a caption, which folds
    /// them away unless the selection's in them, and they step in under it; then each branch
    /// with its commits, up to the fold, unless it's closed.
    fn lane_rows(&self, store: &Store, id: &ProjectId, branches: &[Branch], rows: &mut Rows) {
        let branches = &*store.after_deletes(id, branches);
        let stacked = branches.len() > 1;
        if let [_, .., bottom] = branches {
            let key = (id.clone(), bottom.ref_name.clone());
            let selected = (store.selection.as_ref())
                .is_some_and(|s| s.project() == id && branches.iter().any(|b| s.on(b)));
            rows.push(Row::Item(Selection::Stack(key.0.clone(), key.1.clone())));
            if self.folded_stacks.contains(&key) && !selected {
                return;
            }
            rows.open_in(STEP);
        }
        for branch in branches {
            let key = (id.clone(), branch.ref_name.clone());
            // Past the fold, but never past the selected commit, as after moving it down a step.
            let selected_at = match &store.selection {
                Some(Selection::Commit(p, c)) if p == id => {
                    branch.commits.iter().position(|commit| commit.id == *c)
                }
                _ => None,
            };
            let unfolded = self.unfolded(&key);
            let fold_at = FOLD_AFTER.max(selected_at.map_or(0, |at| at + 1));
            let shown = match unfolded {
                true => branch.commits.len(),
                false => fold_at,
            };
            let closed = !branch.commits.is_empty()
                && !(store.selection.as_ref())
                    .is_some_and(|s| s.project() == id && s.under(branch))
                && !self.is_open(&ClickOpened::Here(key.clone()));
            rows.push(Row::Item(Selection::Branch(key.0, key.1)));
            if closed {
                continue;
            }
            rows.open();
            // `None` there is the target's: an unnamed branch has no remote of its own.
            if let (Some(workspace), Some(ref_name)) = (store.workspace(id), &branch.ref_name) {
                self.upstream_rows(store, workspace, id, Some(ref_name), rows);
            }
            let pending = |at: &dyn Fn(&NewAt) -> bool| {
                (store.new_commits(id, at).into_iter())
                    .map(|(title, slow)| Row::Pending(title, slow))
            };
            rows.extend(pending(
                &|at| matches!(at, NewAt::Top(ref_name) if branch.ref_name.as_ref() == Some(ref_name)),
            ));
            // Commits a pending operation takes away go, unless selected and going nowhere
            // else, as a moved one shows twice otherwise; ones it makes show where they go.
            let commits: Vec<_> = (branch.commits.iter().enumerate())
                .filter(|&(at, commit)| {
                    !store.hides_commit(id, &commit.key)
                        || (selected_at == Some(at) && !store.moves_commit(id, &commit.key))
                })
                .collect();
            let count = commits.len();
            rows.extend((commits.into_iter()).take(shown).flat_map(|(_, commit)| {
                let item = Row::Item(Selection::Commit(id.clone(), commit.id));
                (pending(&|at| matches!(at, NewAt::Above(key) if *key == commit.key)))
                    .chain([item])
                    .chain(pending(
                        &|at| matches!(at, NewAt::Below(key) if *key == commit.key),
                    ))
            }));
            // Show fewer goes by the fold, not the selection, so selecting the last row keeps it.
            if count > shown {
                rows.push(Row::More(
                    id.clone(),
                    branch.ref_name.clone(),
                    count - shown,
                ));
            } else if unfolded && count > FOLD_AFTER {
                rows.push(Row::Fewer(id.clone(), branch.ref_name.clone()));
            }
            rows.close();
        }
        if stacked {
            rows.close();
        }
    }

    /// `workspace`'s linked worktrees, in the order they had when the pointer came over the
    /// list, while it's there; a new one goes last.
    fn worktrees<'a>(&self, id: &ProjectId, workspace: &'a Workspace) -> Vec<&'a Worktree> {
        let mut worktrees: Vec<&Worktree> = workspace.worktrees.iter().collect();
        if let Some(held) = self.held_worktrees.get(id) {
            worktrees.sort_by_key(|wt| {
                (held.iter().position(|name| *name == wt.name)).unwrap_or(usize::MAX)
            });
        }
        worktrees
    }

    /// A linked worktree's heading, its uncommitted changes, and, unless it's folded without
    /// the selection in them, its branches; then a line ruling it off from what follows.
    fn worktree_rows(
        &self,
        store: &Store,
        id: &ProjectId,
        worktree: &Worktree,
        selected: bool,
        rows: &mut Rows,
    ) {
        let name = worktree.name.clone();
        rows.push(Row::Item(Selection::Worktree(id.clone(), name.clone())));
        if self.worktree_open(id, Checkout::Linked(name.clone()), selected) {
            rows.open_in(STEP);
            let workspace = store.workspace(id);
            let lane = workspace.and_then(|ws| ws.worktree_lane(name.as_ref()));
            self.lane_rows(store, id, lane.map_or(&[][..], |lane| &lane.branches), rows);
            if let Some((workspace, lane)) = workspace.zip(lane) {
                let listed = |branch: &Branch| {
                    !(branch.ref_name.as_ref())
                        .is_some_and(|r| store.hides_branch(id, r) || store.deletes_branch(id, r))
                };
                rows.extend(
                    rests_on(id, workspace, lane, listed)
                        .map(|(item, checkout)| Row::RestsOn(item, checkout)),
                );
            }
            rows.close();
        }
        rows.push(Row::WorktreeDivider);
    }

    /// The heading over the commits only `ref_name`'s remote has, or with `None` the target's
    /// new ones, and when it's open each of them, newest first, as `but status` lists them;
    /// for the target, then how many more its page lists.
    fn upstream_rows(
        &self,
        store: &Store,
        workspace: &Workspace,
        id: &ProjectId,
        ref_name: Option<&FullName>,
        rows: &mut Rows,
    ) {
        let commits = workspace.upstream(ref_name);
        if commits.is_empty() {
            return;
        }
        let owner = ref_name.cloned();
        rows.push(Row::Item(Selection::Section(
            id.clone(),
            Section::Upstream(owner.clone()),
        )));
        if !self.upstream_open(store, id, ref_name) {
            return;
        }
        // Guides from the heading's chevron and an open merge's, as from a branch's.
        let (heading, merge) = upstream_chevrons(ref_name);
        rows.open_at(heading);
        let selected = match &store.selection {
            Some(Selection::Upstream(p, r, c)) if p == id && *r == owner => Some(*c),
            _ => None,
        };
        for commit in commits {
            rows.push(Row::Item(Selection::Upstream(
                id.clone(),
                owner.clone(),
                commit.id,
            )));
            let Some((merged, _)) = (store.merged.get(&commit.id)).and_then(|m| m.as_ref()) else {
                continue;
            };
            let holds = selected.is_some_and(|c| merged.iter().any(|m| m.id == c));
            if commit.merge && (self.open_merges.contains(&commit.id) || holds) {
                rows.open_at(merge);
                rows.extend(merged.iter().map(|merged| {
                    Row::Item(Selection::Upstream(id.clone(), owner.clone(), merged.id))
                }));
                rows.close();
            }
        }
        let behind = (workspace.target.as_ref()).filter(|_| owner.is_none());
        let more = behind.map_or(0, |target| target.behind.saturating_sub(commits.len()));
        if more > 0 {
            rows.push(Row::UpstreamMore(id.clone(), more));
        }
        rows.close();
    }
}

fn rests_on<'a>(
    id: &'a ProjectId,
    workspace: &'a Workspace,
    lane: &'a Lane,
    listed: impl Fn(&Branch) -> bool + Copy + 'a,
) -> impl Iterator<Item = (Selection, Option<Checkout>)> + 'a {
    workspace.beneath(lane).flat_map(move |link| {
        let mut checkout = Some(link.lane.checkout.clone());
        (link.branches().iter().enumerate())
            .filter(move |(_, branch)| listed(branch))
            .filter_map(move |(at, branch)| {
                let item = match (link.from.filter(|_| at == 0), &branch.ref_name) {
                    (Some(from), _) => Selection::Commit(id.clone(), from),
                    (None, Some(_)) => Selection::Branch(id.clone(), branch.ref_name.clone()),
                    (None, None) => Selection::Commit(id.clone(), branch.commits.first()?.id),
                };
                Some((item, checkout.take()))
            })
    })
}

/// When `entry` was last active: what Git last recorded in it, or what its loaded workspace
/// shows since, as files an agent is writing.
pub(super) fn last_active(entry: &ProjectEntry) -> Option<i64> {
    let loaded = entry.workspace.as_ref().and_then(|ws| ws.last_activity());
    entry.project.last_active.max(loaded)
}

fn active_lately(at: i64) -> bool {
    crate::ui::now() - at <= ACTIVE_WITHIN.as_secs() as i64
}

/// Whether the selection's in one of project `id`'s `inactive` worktrees.
pub(super) fn in_inactive(store: &Store, id: &ProjectId, inactive: &[&Worktree]) -> bool {
    (store.selection.as_ref())
        .filter(|s| s.project() == id)
        .is_some_and(|s| {
            let workspace = store.workspace(id);
            (inactive.iter()).any(|wt| {
                let checkout = Checkout::Linked(wt.name.clone());
                matches!(s, Selection::Worktree(_, name) if *name == wt.name)
                    || within(s, workspace.iter().flat_map(|ws| ws.branches_in(&checkout)))
            })
        })
}

/// Whether project `id`'s `inactive` worktrees are listed under their heading: once it's
/// opened, and while the selection's in one.
pub(super) fn inactive_open(store: &Store, id: &ProjectId, inactive: &[&Worktree]) -> bool {
    in_inactive(store, id, inactive) || store.prefs.shown_inactive.contains(id.as_ref())
}

/// Whether `selection` is one of `branches`, a commit on one, or a stack of them.
pub(super) fn within<'a>(
    selection: &Selection,
    mut branches: impl Iterator<Item = &'a Branch>,
) -> bool {
    branches.any(|branch| {
        selection.on(branch)
            || matches!(selection, Selection::Stack(_, bottom) if *bottom == branch.ref_name)
    })
}

/// Whether what's `dragging` can land apart from the rest of `project`'s `workspace`:
/// commits in a new branch, and a branch that shares its stack in one of its own.
fn lands_apart(dragging: Option<&Cut>, project: &ProjectId, workspace: &Workspace) -> bool {
    match dragging {
        Some(Cut::Commit(from, _)) => from == project,
        Some(Cut::Branch(from, branch)) => {
            from == project && BranchEdits::of(workspace, branch).stacked
        }
        _ => false,
    }
}

/// Which of `terms`, lowercase, any of `texts` holds, as bits by the term's place.
pub(super) fn found(terms: &[String], texts: &[&str]) -> u64 {
    (terms.iter().enumerate())
        .filter(|(_, term)| texts.iter().any(|text| holds(text, term)))
        .fold(0, |bits, (ix, _)| bits | 1 << ix)
}

/// Whether a row whose own text has the terms in `own`, under rows with those in `held`,
/// matches: every term found, and one at least in its own text.
pub(super) fn matches(terms: &[String], held: u64, own: u64) -> bool {
    own != 0 && held | own == u64::MAX >> (64 - terms.len())
}

/// Whether `text` holds `term` (lowercase), in any case; without allocating for ASCII text,
/// as most names and titles are, since the filter looks through every commit on each key.
fn holds(text: &str, term: &str) -> bool {
    match text.is_ascii() {
        true => (text.as_bytes().windows(term.len()))
            .any(|window| window.eq_ignore_ascii_case(term.as_bytes())),
        false => text.to_lowercase().contains(term),
    }
}

#[cfg(test)]
mod filter_tests {
    use super::{found, holds, matches};

    #[test]
    fn a_row_matches_with_what_holds_it() {
        let terms = ["shop".to_owned(), "cart".to_owned()];
        let project = found(&terms, &["shop"]);
        assert!(
            !matches(&terms, 0, project),
            "the project alone lacks \"cart\""
        );
        let branch = found(&terms, &["feature/Cart"]);
        assert!(
            matches(&terms, project, branch),
            "the branch has \"cart\", in any case, and its project \"shop\""
        );
        let commit = found(&terms, &["Keep items in order"]);
        assert!(
            !matches(&terms, project | branch, commit),
            "a commit needs a word of its own, not only its branch's"
        );
        assert!(
            holds("Über die Karte", "über"),
            "non-ASCII text folds case too"
        );
    }
}

#[cfg(test)]
mod within_tests {
    use super::within;
    use crate::backend::model::build::full;
    use crate::backend::{Branch, CommitId, PushState};
    use crate::state::{Section, Selection};

    #[test]
    fn a_commit_only_the_remote_has_is_within_its_branch() {
        let branch = Branch {
            name: "feature".into(),
            ref_name: Some(full("refs/heads/feature")),
            commits: Vec::new(),
            incoming: Vec::new(),
            remote: Some("origin/feature".into()),
            push: PushState::NothingToPush,
            review: None,
        };
        let upstream = |ref_name: Option<&str>| {
            let id = CommitId(gix::ObjectId::from([1; 20]));
            Selection::Upstream("p".into(), ref_name.map(full), id)
        };
        assert!(
            within(&upstream(Some("refs/heads/feature")), [&branch].into_iter()),
            "folding the branch's worktree or stack must keep its selected remote commit"
        );
        assert!(
            !within(&upstream(Some("refs/heads/other")), [&branch].into_iter()),
            "another branch's remote commit is elsewhere"
        );
        assert!(
            !within(&upstream(None), [&branch].into_iter()),
            "the target's new commits sit above every worktree"
        );
        let heading = |ref_name: Option<&str>| {
            Selection::Section("p".into(), Section::Upstream(ref_name.map(full)))
        };
        assert!(
            within(&heading(Some("refs/heads/feature")), [&branch].into_iter()),
            "and so must the heading over its remote's commits"
        );
        assert!(
            !within(&heading(None), [&branch].into_iter()),
            "the target's heading sits above every worktree"
        );
    }
}

#[cfg(test)]
mod lands_apart_tests {
    use super::lands_apart;
    use crate::backend::model::build::{branch, full, workspace, worktree};
    use crate::state::{Cut, ProjectId};

    #[test]
    fn a_stacked_branch_lands_apart_and_a_lone_one_does_not() {
        let project = ProjectId::from("p");
        let ws = workspace(
            vec![
                vec![branch("ui", &[3]), branch("api", &[2, 1])],
                vec![branch("typo", &[4])],
            ],
            vec![worktree(
                "wt",
                None,
                vec![branch("top", &[6]), branch("low", &[5])],
            )],
        );
        let dragged =
            |name: &str| Cut::Branch(project.clone(), full(&format!("refs/heads/{name}")));
        assert!(
            lands_apart(Some(&dragged("ui")), &project, &ws),
            "the top of a stack can leave it"
        );
        assert!(
            lands_apart(Some(&dragged("api")), &project, &ws),
            "and so can the bottom, the one above then resting on the base"
        );
        assert!(
            !lands_apart(Some(&dragged("typo")), &project, &ws),
            "a branch alone in its stack is apart already"
        );
        assert!(
            !lands_apart(Some(&dragged("top")), &project, &ws),
            "a linked worktree's lane can't come apart"
        );
        assert!(
            !lands_apart(Some(&dragged("ui")), &ProjectId::from("other"), &ws),
            "it lands only in its own project"
        );
    }
}

#[cfg(test)]
mod rests_on_tests {
    use super::rests_on;
    use crate::backend::Branch;
    use crate::backend::model::build::{branch, commit_name, workspace, worktree};
    use crate::state::Selection;

    #[test]
    #[expect(
        clippy::disallowed_methods,
        reason = "a test, and snapbox looks for the file its snapshot is in"
    )]
    fn a_worktree_lists_each_branch_beneath_it_once() {
        let unnamed = Branch {
            name: "Unnamed branch".into(),
            ..branch("", &[9])
        };
        let ws = workspace(
            vec![
                vec![
                    branch("top", &[1]),
                    branch("middle", &[2, 3]),
                    branch("bottom", &[4]),
                ],
                vec![unnamed, branch("base", &[10])],
            ],
            vec![
                worktree("wt", Some(3), vec![branch("wt", &[5])]),
                worktree("wt-stacked", Some(5), vec![branch("wt-stacked", &[6])]),
                worktree("on-unnamed", Some(9), vec![]),
                worktree("outside", None, vec![branch("outside", &[7])]),
            ],
        );
        let id = "p".into();
        let rows: String = (ws.lanes.iter().filter(|lane| lane.checkout.is_linked()))
            .map(|lane| {
                let rows = (rests_on(&id, &ws, lane, |b| b.name != "base"))
                    .map(|(row, checkout)| {
                        let row = match row {
                            Selection::Branch(_, ref_name) => {
                                format!(
                                    "Branch({})",
                                    ref_name
                                        .as_ref()
                                        .map(ToString::to_string)
                                        .unwrap_or_default()
                                )
                            }
                            Selection::Commit(_, at) => {
                                format!("Commit({})", commit_name(&ws, at))
                            }
                            other => format!("{other:?}"),
                        };
                        let checkout = checkout.map_or(String::new(), |checkout| {
                            format!(" in {}", checkout.linked().unwrap_or("Main".into()))
                        });
                        format!("{row}{checkout}")
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}: [{rows}]\n", lane.checkout.linked().unwrap_or_default())
            })
            .collect();
        snapbox::assert_data_eq!(
            rows,
            snapbox::str![[r#"
wt: [Commit(middle~1) in Main, Branch(refs/heads/bottom)]
wt-stacked: [Branch(refs/heads/wt) in wt, Commit(middle~1) in Main, Branch(refs/heads/bottom)]
on-unnamed: [Commit(Unnamed branch) in Main]
outside: []

"#]]
        );
    }
}
