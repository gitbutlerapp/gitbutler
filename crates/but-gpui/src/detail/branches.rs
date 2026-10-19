//! The Branches page: branches outside the workspace, local ones first, then the ones only a
//! remote has, each with what it would bring in, ready to apply or delete.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui::{
    AnyElement, ClickEvent, Context, Focusable as _, MouseButton, MouseDownEvent, Pixels, Point,
    SharedString, Window, div, prelude::*, px,
};

use super::Detail;
use super::panes::{Crumb, side_list, side_row};
use crate::app::{Collapse, Expand, SelectNext, SelectPrevious};
use crate::backend::branches::{self, ApplyOutcome, BranchCommit, OtherBranch};
use crate::backend::{self, CommitId, DiffSource};
use crate::overlays::MenuItem;
use crate::state::{Cut, Overlay, Prefs, ProjectId, Selection};
use crate::theme::Theme;
use crate::transfer::drag_preview;
use crate::ui::illustration::Illustration;
use crate::ui::person::person_avatar;
use crate::ui::{
    ButtonStyle, Icon, ROW_HEIGHT, Tooltip, age_column, button, empty_state, faint, field, h_flex,
    icon_button, muted, plural, relative_time, review, small_icon, v_flex,
};

pub(super) struct Branches {
    project: ProjectId,
    /// The project's branches when the list loaded; a reload that changes none of them
    /// leaves the list as it is. `None` until it has loaded.
    seen: Option<u64>,
    list: Vec<OtherBranch>,
    /// The selected branch, by ref.
    selected: Option<String>,
    /// The branches whose commits show under them, by ref: folded until opened, as the
    /// navigator's are.
    open: HashSet<String>,
    /// The stacks whose branches are folded away under their caption, by their top branch.
    folded: HashSet<String>,
    /// Open branches' commits, by ref and what they were loaded for.
    commits: HashMap<String, (Loaded, Vec<BranchCommit>)>,
    /// One of the selected branch's commits, to see on its own and to copy into the workspace.
    commit: Option<CommitId>,
}

impl Branches {
    fn selected_branch(&self) -> Option<&OtherBranch> {
        let selected = self.selected.as_ref()?;
        self.list.iter().find(|b| b.ref_name == *selected)
    }
}

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
        if !force
            && self
                .branches_for(project)
                .is_some_and(|b| b.seen == Some(seen))
        {
            return;
        }
        let project = project.clone();
        self._branches_load = cx.spawn(async move |this, cx| {
            let result = backend::run(move || branches::other_branches(&path)).await;
            this.update(cx, |this, cx| {
                let list = match result {
                    Ok(list) => list,
                    Err(err) => {
                        this.store.update(cx, |store, cx| {
                            store.error("Couldn't list the branches", format!("{err:#}"), cx)
                        });
                        Vec::new()
                    }
                };
                let previous = this.branches.take().filter(|b| b.project == project);
                // The newest, which the list shows first.
                let first = list.first();
                let selected = (previous.as_ref().and_then(|b| b.selected.clone()))
                    .filter(|ref_name| list.iter().any(|b| b.ref_name == *ref_name))
                    .or_else(|| first.map(|b| b.ref_name.clone()));
                // Which branches were open and their commits, of those still listed, and the
                // selected commit while its branch stays selected.
                let listed = |ref_name: &String| list.iter().any(|b| b.ref_name == *ref_name);
                let (open, folded, commits, commit) = match previous {
                    Some(b) => (
                        b.open.into_iter().filter(|r| listed(r)).collect(),
                        b.folded.into_iter().filter(|r| listed(r)).collect(),
                        b.commits.into_iter().filter(|(r, _)| listed(r)).collect(),
                        b.commit.filter(|_| b.selected == selected),
                    ),
                    None => Default::default(),
                };
                this.branches = Some(Branches {
                    project,
                    seen: Some(seen),
                    list,
                    selected,
                    open,
                    folded,
                    commits,
                    commit,
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

    pub(super) fn selected_other_branch(&self, project: &ProjectId) -> Option<&OtherBranch> {
        self.branches_for(project)?.selected_branch()
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
                    // Unless the branch moved meanwhile; one that failed is tried again when
                    // it's next opened.
                    let current =
                        (branches.list.iter()).any(|b| b.ref_name == r && (b.tip, b.ahead) == key);
                    match commits {
                        Ok(commits) if current => {
                            branches.commits.insert(r, (key, commits));
                        }
                        Ok(_) => {}
                        Err(err) => tracing::warn!("couldn't list the branch's commits: {err:#}"),
                    }
                }
                let gone = (branches.commit.zip(branches.selected.as_ref()))
                    .and_then(|(id, r)| Some((id, &branches.commits.get(r)?.1)))
                    .is_some_and(|(id, commits)| commits.iter().all(|c| c.id != id));
                if gone {
                    branches.commit = None;
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
        if branches.open.insert(ref_name.to_owned()) {
            self.load_open_commits(cx);
        } else {
            branches.open.remove(ref_name);
            if branches.selected.as_deref() == Some(ref_name) && branches.commit.take().is_some() {
                self.show_picked(cx);
            }
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
        // when asked for.
        let keeps = |branch: &OtherBranch| {
            !store.hides_branch(&branches.project, &branch.ref_name)
                && (prefs.branches_with_empty || branch.own_commits != Some(0))
                && (branch.local || !prefs.branches_local_only)
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
    pub(crate) fn select_shown_branch(&mut self, cx: &mut Context<Self>) {
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
        if let Some(branches) = &mut self.branches {
            branches.selected = ref_name;
            branches.commit = None;
        }
        self.show_picked(cx);
    }

    /// Select commit `id` of the branch `ref_name`, which it's listed under.
    fn select_branch_commit(&mut self, ref_name: &str, id: CommitId, cx: &mut Context<Self>) {
        if let Some(branches) = &mut self.branches {
            branches.selected = Some(ref_name.to_owned());
            branches.commit = Some(id);
        }
        self.show_picked(cx);
    }

    /// ↓ or ↑ (`delta`) in the list: the next or previous branch or commit shown, kept in view.
    fn step_branches(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(branches) = &self.branches else {
            return;
        };
        let rows = self.branch_rows(branches, cx);
        let at = rows.iter().position(|row| is_selected(branches, row));
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
        let Some(branch) = self.selected_other_branch(project) else {
            return;
        };
        let mut items = Vec::new();
        let worktree = self.checked_out_in(project, branch, cx);
        let p = project.clone();
        items.push(match worktree.clone() {
            Some(name) => MenuItem::new("Open worktree", move |app, _, cx| {
                let shown = Selection::Worktree(p.clone(), name.clone());
                app.store
                    .update(cx, |store, cx| store.select(Some(shown), cx))
            }),
            None => MenuItem::new("Apply to workspace", move |app, _, cx| {
                app.detail
                    .update(cx, |detail, cx| detail.apply_branch(&p, cx))
            }),
        });
        if worktree.is_none() {
            let p = project.clone();
            items.push(MenuItem::new("Switch to branch", move |app, _, cx| {
                app.detail
                    .update(cx, |detail, cx| detail.switch_to_branch(&p, cx))
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
        if branch.local && worktree.is_none() {
            let p = project.clone();
            items.push(
                // Once the menu's done with the app, which asking to confirm updates too.
                MenuItem::new("Delete branch…", move |app, window, cx| {
                    let (detail, p) = (app.detail.clone(), p.clone());
                    window.defer(cx, move |window, cx| {
                        detail.update(cx, |detail, cx| detail.delete_other_branch(&p, window, cx))
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

    /// Select commit `id` of `ref_name` and offer to copy it into the workspace, or its ID.
    fn branch_commit_menu(
        &mut self,
        project: &ProjectId,
        ref_name: &str,
        id: CommitId,
        at: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.select_branch_commit(ref_name, id, cx);
        let project = project.clone();
        let items = vec![
            MenuItem::new("Cherry-pick", move |app, _, cx| {
                app.cut(Cut::CherryPick(project.clone(), id), cx)
            }),
            MenuItem::new("Copy commit ID", move |app, _, cx| {
                app.copy_text(id.0.to_string(), cx)
            }),
        ];
        self.app
            .update(cx, |app, cx| app.open_menu(at, items, cx))
            .ok();
    }

    /// Pick up the selected commit, to put a copy of it where ⌘V lands in the workspace.
    fn copy_branch_commit(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        let Some(id) = self.selected_branch_commit(project) else {
            return;
        };
        let cut = Cut::CherryPick(project.clone(), id);
        self.app.update(cx, |app, cx| app.cut(cut, cx)).ok();
    }

    fn apply_branch(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        let Some(branch) = self.selected_other_branch(project) else {
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

    /// Leave the workspace for just the selected branch, checked out on its own.
    fn switch_to_branch(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        let Some(branch) = self.selected_other_branch(project) else {
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(branch) = self.selected_other_branch(project).filter(|b| b.local) else {
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
                            let gone = ref_name.clone();
                            store.run(
                                &project,
                                "Delete branch",
                                overlay,
                                move |path, _| backend::remove_branch(path, &ref_name),
                                // Out of the list now: reading the branches again, as their
                                // refs changed, lands after the overlay is gone.
                                move |store, (), cx| {
                                    (detail.update(cx, |detail, _| {
                                        if let Some(branches) = detail.branches.as_mut() {
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

    pub(super) fn render_branches_side(
        &self,
        project: &ProjectId,
        theme: &Theme,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let filter_focused = self.branch_filter.focus_handle(cx).is_focused(window);
        let prefs = &self.store.read(cx).prefs;
        // Lit while the filters aren't as they are at rest, as Lite counts them.
        let changed = prefs.branches_with_empty || prefs.branches_local_only;
        let filter = h_flex()
            .flex_none()
            .m(px(10.))
            .gap(px(4.))
            .child(field(&self.branch_filter, filter_focused, theme).flex_1())
            .child(
                icon_button("branch-filters", Icon::Filter)
                    .small()
                    .selected(changed)
                    .tooltip("Branch filters")
                    .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                        this.branch_filters_menu(event.position(), cx)
                    })),
            );
        let Some(branches) = self.branches_for(project).filter(|b| b.seen.is_some()) else {
            return empty_state(None, "Loading branches…", "", theme, cx);
        };
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
            div()
                .flex_1()
                .min_h(px(0.))
                .child(empty_state(Some(drawing), empty, "", theme, cx))
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
                            let payload = Cut::CherryPick(project.clone(), id);
                            let selected = is_selected(branches, row);
                            let on = selected && solid;
                            let menu_ref = ref_name.clone();
                            side_row(("other-commit", ix), selected, solid, theme)
                                // Its glyph under its branch's, its title under the name.
                                .pl(px(if *stacked { 38. } else { 24. }))
                                .aria_label(label.clone())
                                .child(small_icon(Icon::Commit).text_color(match on {
                                    true => theme.text_inverted,
                                    false => theme.text_3,
                                }))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .truncate()
                                        .text_size(px(12.))
                                        .child(label.clone()),
                                )
                                .child(quiet(faint(relative_time(commit.time), theme), on, theme))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    window.focus(&this.side_focus, cx);
                                    this.select_branch_commit(&ref_name, id, cx)
                                }))
                                .on_mouse_down(
                                    MouseButton::Right,
                                    cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                        window.focus(&this.side_focus, cx);
                                        let at = event.position;
                                        this.branch_commit_menu(&project, &menu_ref, id, at, cx)
                                    }),
                                )
                                .on_drag(payload, drag_preview(this.store.clone(), label))
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
                                .truncate();
                            let name = match &branch.review {
                                Some(review) if !review.title.is_empty() => name
                                    .text_color(ink(review::title_color(review.state, theme)))
                                    .child(SharedString::from(review.title.clone())),
                                // A branch only a remote has, by the remote's name for it.
                                _ => name
                                    .font_family(theme.mono_font.clone())
                                    .text_size(px(12.))
                                    .text_color(ink(if merged.is_some() {
                                        theme.text_2
                                    } else {
                                        theme.blue
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
                                            .child(person_avatar(author, 14., theme))
                                            .tooltip(Tooltip::text(format!(
                                                "Last commit by {author}"
                                            ))),
                                    )
                                })
                                // Its slot kept for an author without a time, so the
                                // avatars line up.
                                .when(branch.time.is_some() || branch.author.is_some(), |el| {
                                    el.child(quiet(age_column(branch.time, theme), on, theme))
                                })
                                // Double-clicking folds it or opens it, as with a folder.
                                .on_click(cx.listener(
                                    move |this, event: &ClickEvent, window, cx| {
                                        window.focus(&this.side_focus, cx);
                                        match event.click_count() {
                                            2 => this.toggle_branch(&ref_name, cx),
                                            _ => {
                                                this.select_other_branch(Some(ref_name.clone()), cx)
                                            }
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
        };
        v_flex()
            .size_full()
            .track_focus(&self.side_focus)
            .key_context("SideList")
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.step_branches(1, cx)))
            .on_action(cx.listener(|this, _: &SelectPrevious, _, cx| this.step_branches(-1, cx)))
            .on_action(cx.listener(|this, _: &Expand, _, cx| this.fold_branches(true, cx)))
            .on_action(cx.listener(|this, _: &Collapse, _, cx| this.fold_branches(false, cx)))
            .child(filter)
            .child(list)
            .into_any_element()
    }

    /// Lite's branch filters: branches with no commits of their own, left out at rest, and
    /// the ones only a remote has.
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
        ];
        (self.app)
            .update(cx, |app, cx| app.open_menu(position, items, cx))
            .ok();
    }

    /// The header's crumbs for the selected branch and commit, and what can be done with
    /// them.
    pub(super) fn branches_header(
        &self,
        project: &ProjectId,
        crumbs: &mut Vec<Crumb>,
        actions: &mut Vec<AnyElement>,
        theme: &Theme,
        cx: &Context<Self>,
    ) {
        let Some(branch) = self.selected_other_branch(project) else {
            return;
        };
        crumbs.push((branch.name.clone().into(), None));
        let local = branch.local;
        // Who made it and how far ahead it is.
        let target = (self.store.read(cx).workspace(project))
            .and_then(|ws| ws.target.as_ref())
            .map_or("the target".to_owned(), |t| t.name.clone());
        let ahead = match branch.ahead {
            Some(0) => format!("already in {target}"),
            Some(n) => plural(n, "commit"),
            None => String::new(),
        };
        let summary = [branch.author.clone().unwrap_or_default(), ahead];
        let summary: Vec<String> = summary.into_iter().filter(|p| !p.is_empty()).collect();
        actions.push(muted(summary.join(" · "), theme).into_any_element());
        if let Some(title) = self.selected_branch_commit_title(project) {
            crumbs.push((title.into(), None));
            let project = project.clone();
            actions.push(
                button("copy-commit", "Cherry-pick")
                    .style(ButtonStyle::Ghost)
                    .tooltip("Then press ⌘V on a branch or commit, or drag it there")
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.copy_branch_commit(&project, cx)
                    }))
                    .into_any_element(),
            );
        }
        if let Some(name) = self.checked_out_in(project, branch, cx) {
            let shown = Selection::Worktree(project.clone(), name);
            let store = self.store.clone();
            actions.push(
                button("open-worktree", "Open worktree")
                    .style(ButtonStyle::Pop)
                    .tooltip("A linked worktree has it checked out")
                    .on_click(move |_, _, cx| {
                        store.update(cx, |store, cx| store.select(Some(shown.clone()), cx))
                    })
                    .into_any_element(),
            );
        } else {
            if local {
                let project = project.clone();
                actions.push(
                    button("delete-other", "Delete…")
                        .style(ButtonStyle::Ghost)
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.delete_other_branch(&project, window, cx)
                        }))
                        .into_any_element(),
                );
            }
            let switched = project.clone();
            actions.push(
                button("switch-to-branch", "Switch to branch")
                    .style(ButtonStyle::Ghost)
                    .tooltip(
                        "Leave the workspace and work on just this branch. Switch to the \
                         workspace, on the project's page, brings the others back.",
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.switch_to_branch(&switched, cx)
                    }))
                    .into_any_element(),
            );
            let project = project.clone();
            actions.push(
                button("apply-branch", "Apply to workspace")
                    .style(ButtonStyle::Pop)
                    .tooltip("Add it to the workspace, beside your other branches")
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.apply_branch(&project, cx)
                    }))
                    .into_any_element(),
            );
        }
    }
}

/// Faint meta text, in the selection's ink on a solid one.
fn quiet(el: gpui::Div, on: bool, theme: &Theme) -> gpui::Div {
    el.flex_none()
        .when(on, |el| el.text_color(theme.text_inverted.opacity(0.7)))
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

/// What the list calls a branch: its pull request's title once known, else its name.
fn label(branch: &OtherBranch) -> &str {
    (branch.review.as_ref())
        .map(|review| review.title.as_str())
        .filter(|title| !title.is_empty())
        .unwrap_or(&branch.name)
}
