//! The navigator's right-click menus.

use gpui::App;

use super::Navigator;
use crate::app::{
    Collapse, CopySelection, CutSelection, DeleteSelection, Expand, Fetch, NewBranchOnTop,
    OpenPullRequest, Push, RenameSelection, StartCommit, UncommitSelection, UpdateFromTarget,
};
use crate::backend::CommitId;
use crate::edits::{
    AddEmptyCommitAbove, AddEmptyCommitBelow, BranchEdits, CherryPick, CommitEdits, CopyChangeId,
    CopyMessage, MoveToNewBranch, NewBranchBelow, NewBranchHere, PullRemote, RewrapMessage,
    SquashAll, SquashPicked, TakeRemote, UnapplyStack, Unstack,
};
use crate::overlays::MenuItem;
use crate::state::{ProjectId, Selection};
use crate::ui::plural;

/// An item marking `unread`, commits of `project`, read.
fn mark_read(label: &str, project: &ProjectId, unread: Vec<CommitId>) -> MenuItem {
    let project = project.clone();
    MenuItem::new(label.to_owned(), move |this, _, cx| {
        let unread = unread.clone();
        (this.store).update(cx, |store, cx| store.mark_read(&project, unread, cx))
    })
}

impl Navigator {
    /// What right-clicking `target` offers. Right-clicking selects the row first, so the
    /// items run the same actions as their keys. A branch with commits starts with hiding
    /// or showing them, and marking its unread ones read.
    pub(crate) fn menu_items(&self, target: &Selection, cx: &App) -> Vec<MenuItem> {
        let mut items = self.row_actions(target, cx);
        let store = self.store.read(cx);
        if let Selection::Branch(project, ref_name) = target
            && let Some(branch) = store.branch_for(target).filter(|b| !b.commits.is_empty())
        {
            let key = (project.clone(), ref_name.clone());
            let mut view = vec![match self.closed_branches.contains(&key) {
                true => MenuItem::action("Show commits", &Expand, cx),
                false => MenuItem::action("Hide commits", &Collapse, cx),
            }];
            let unread: Vec<CommitId> = (branch.commits.iter())
                .filter(|commit| store.is_unread(project, commit))
                .map(|commit| commit.id)
                .collect();
            if !unread.is_empty() {
                view.push(mark_read("Mark all as read", project, unread));
            }
            if let Some(first) = items.first_mut() {
                first.separated = true;
            }
            items.splice(0..0, view);
        }
        items
    }

    fn row_actions(&self, target: &Selection, cx: &App) -> Vec<MenuItem> {
        let store = self.store.read(cx);
        let action =
            |label: &'static str, action: &dyn gpui::Action| MenuItem::action(label, action, cx);
        match target {
            Selection::Project(project) => {
                let expanded = store.project(project).is_some_and(|p| p.expanded);
                let mut items = vec![if expanded {
                    action("Stop watching", &Collapse)
                } else {
                    action("Watch", &Expand)
                }];
                if expanded {
                    items.push(action("Fetch", &Fetch));
                    if let Some(target) = (store.workspace(project))
                        .and_then(|ws| ws.target.as_ref())
                        .filter(|target| target.needs_update)
                    {
                        items.push(MenuItem::action(
                            format!("Update from {}", target.name),
                            &UpdateFromTarget,
                            cx,
                        ));
                    }
                }
                let archived = (store.workspace(project))
                    .map_or_else(Vec::new, |ws| ws.archived_worktrees.clone());
                if !archived.is_empty() {
                    let project = project.clone();
                    items.push(MenuItem::new(
                        "Show an archived worktree…",
                        move |this, window, cx| {
                            let choices = (archived.iter())
                                .map(|name| {
                                    let (project, name) = (project.clone(), name.clone());
                                    crate::palette::Choice::new(
                                        name.to_string(),
                                        move |this, _, cx| {
                                            this.store.update(cx, |store, cx| {
                                                store.set_worktree_archived(
                                                    &project, name, false, cx,
                                                )
                                            })
                                        },
                                    )
                                })
                                .collect();
                            this.open_picker("Show a worktree again", "Show", choices, window, cx)
                        },
                    ));
                }
                if let Some(path) = store.path(project) {
                    let label = crate::open_in::editor_label(store);
                    let (editor_project, terminal_project) = (project.clone(), project.clone());
                    items.extend([
                        MenuItem::new(label, move |this, window, cx| {
                            let position = window.mouse_position();
                            this.open_in_editor(&editor_project, None, None, None, position, cx)
                        })
                        .separated(),
                        MenuItem::new("Open in terminal", move |this, _, cx| {
                            this.open_in_terminal(&terminal_project, cx)
                        }),
                        MenuItem::new("Reveal in Finder", move |_, _, cx| cx.reveal_path(&path)),
                    ]);
                }
                let settings_project = project.clone();
                items.push(
                    MenuItem::new("Project settings…", move |this, window, cx| {
                        this.open_project_settings(settings_project.clone(), window, cx)
                    })
                    .separated(),
                );
                items
            }
            Selection::Uncommitted(project) => {
                if store
                    .workspace(project)
                    .is_none_or(|ws| ws.uncommitted.is_empty())
                {
                    return Vec::new();
                }
                // Only the composer checks files; without it, discarding takes them all.
                let discard = match self.detail.read(cx).composing() {
                    true => "Discard checked changes…",
                    false => "Discard all changes…",
                };
                vec![
                    action("Commit…", &StartCommit),
                    action(discard, &DeleteSelection).danger(),
                ]
            }
            Selection::Branch(project, ref_name) => {
                let mut items = Vec::new();
                let (Some(branch), Some(workspace)) =
                    (store.branch_for(target), store.workspace(project))
                else {
                    return items;
                };
                if workspace.can_push(ref_name) && workspace.push_refused(ref_name).is_none() {
                    let label = if workspace.needs_force(ref_name) {
                        "Force push"
                    } else {
                        "Push"
                    };
                    items.push(action(label, &Push));
                }
                // What reaches past a linked worktree, like its remote or a pull request, is
                // done in it.
                let linked = store.worktree_of(target).is_some();
                if workspace.can_pull(ref_name) && !linked {
                    items.extend([
                        MenuItem::action(
                            format!(
                                "Pull {} from the remote",
                                plural(branch.incoming.len(), "commit")
                            ),
                            &PullRemote,
                            cx,
                        ),
                        action("Replace with the remote's version…", &TakeRemote),
                    ]);
                }
                if let Some(url) = branch.review.as_ref().and_then(|r| r.url.clone()) {
                    items.extend([
                        action("Open pull request", &OpenPullRequest),
                        MenuItem::new("Copy pull request link", move |this, _, cx| {
                            this.copy_text(url.clone(), cx)
                        }),
                    ]);
                }
                if crate::pr::on_forge(store, target) && !ref_name.is_empty() && !linked {
                    use crate::backend::ReviewState as S;
                    match branch.review.as_ref().map(|r| r.state) {
                        None if !branch.commits.is_empty() => items.push(action(
                            "Create pull request…",
                            &crate::pr::CreatePullRequest,
                        )),
                        Some(S::Open) => {
                            items.push(action("Merge pull request…", &crate::pr::MergePullRequest))
                        }
                        _ => {}
                    }
                }
                if ref_name.is_empty() {
                    if !branch.commits.is_empty() && !linked {
                        items.push(action("Name branch…", &RenameSelection).separated());
                    }
                    return items;
                }
                let edits = BranchEdits::of(workspace, ref_name);
                // A linked worktree's branches can't be ordered yet.
                if !linked {
                    items.push(action("New branch on top…", &NewBranchOnTop).separated());
                }
                if edits.new_below {
                    items.push(action("New branch below…", &NewBranchBelow));
                }
                let empty = action("Add empty commit", &AddEmptyCommitAbove);
                items.push(if linked { empty.separated() } else { empty });
                if edits.squash_all {
                    items.push(action("Squash all commits", &SquashAll));
                }
                items.push(action("Cut", &CutSelection).separated());
                if edits.stacked {
                    items.push(action("Unstack", &Unstack));
                }
                if let (Some(target), Some(whole)) = (workspace.target.as_ref(), edits.land) {
                    let label = if whole {
                        format!("Land the stack on {}…", target.name)
                    } else {
                        format!("Land on {}…", target.name)
                    };
                    items.push(MenuItem::action(label, &crate::edits::LandBranch, cx));
                }
                // Only the workspace has stacks to take out of it.
                if workspace.managed && !linked {
                    items.push(action(
                        if edits.stacked {
                            "Unapply stack"
                        } else {
                            "Unapply branch"
                        },
                        &UnapplyStack,
                    ));
                }
                items.extend([
                    action("Rename…", &RenameSelection).separated(),
                    action("Copy name", &CopySelection),
                ]);
                if !linked {
                    items.push(action("Delete branch…", &DeleteSelection).danger());
                }
                items
            }
            Selection::Commit(project, id) if store.picked.contains(id) => {
                let n = store.picked.len();
                // Commits list newest first.
                let oldest = (store.workspace(project))
                    .and_then(|ws| ws.commits().filter(|c| store.picked.contains(&c.id)).last())
                    .map(|c| c.id);
                let mut items = vec![MenuItem::action(
                    format!("Squash {n} commits"),
                    &SquashPicked,
                    cx,
                )];
                // The other way round: the rest go into the one clicked, in its place.
                if oldest != Some(*id) {
                    let into = *id;
                    items.push(MenuItem::new(
                        format!("Squash {n} commits into this one"),
                        move |this, _, cx| this.squash_picks(Some(into), cx),
                    ));
                }
                items.push(MenuItem::action(
                    format!("Uncommit {n} commits"),
                    &UncommitSelection,
                    cx,
                ));
                // A run picks commits without opening them, as clicking one does.
                let unread: Vec<CommitId> = (store.workspace(project).into_iter())
                    .flat_map(|ws| ws.commits())
                    .filter(|c| store.picked.contains(&c.id) && store.is_unread(project, c))
                    .map(|c| c.id)
                    .collect();
                if !unread.is_empty() {
                    items.push(mark_read("Mark as read", project, unread).separated());
                }
                items
            }
            Selection::Commit(project, id) => {
                let Some((branch, commit)) = store.find_commit(project, *id) else {
                    return Vec::new();
                };
                let edits = CommitEdits::of(branch, commit);
                // A linked worktree's commits stay in it, and their conflicts resolve there.
                let linked = store.worktree_of(target).is_some();
                let mut items = Vec::new();
                if commit.conflicted && !linked {
                    items.push(action(
                        "Resolve conflicts…",
                        &crate::detail::ResolveConflicts,
                    ));
                }
                items.extend([
                    action("Cut", &CutSelection).separated(),
                    action("Cherry-pick", &CherryPick),
                    action("Edit message…", &RenameSelection).separated(),
                ]);
                if edits.rewrap {
                    items.push(action("Rewrap message", &RewrapMessage));
                }
                items.extend([
                    action("Add empty commit above", &AddEmptyCommitAbove),
                    action("Add empty commit below", &AddEmptyCommitBelow),
                ]);
                if edits.new_branch_here && !linked {
                    items.push(action("New branch ending here…", &NewBranchHere));
                }
                items.extend([
                    action("Move to a new branch", &MoveToNewBranch).separated(),
                    action("Copy commit ID", &CopySelection).separated(),
                ]);
                if edits.change_id {
                    items.push(action("Copy change ID", &CopyChangeId));
                }
                items.extend([
                    action("Copy message", &CopyMessage),
                    action("Uncommit", &UncommitSelection).separated(),
                    action("Discard commit…", &DeleteSelection).danger(),
                ]);
                items
            }
            Selection::Worktree(project, name) => {
                let Some(worktree) =
                    (store.workspace(project)).and_then(|ws| ws.find_worktree(name))
                else {
                    return Vec::new();
                };
                let path = worktree.path.clone();
                let copied = path.display().to_string();
                let dirty = worktree.uncommitted.len();
                let (project, name) = (project.clone(), name.clone());
                let (deleted_project, deleted) = (project.clone(), name.clone());
                vec![
                    MenuItem::new("Reveal in Finder", move |_, _, cx| cx.reveal_path(&path)),
                    MenuItem::new("Copy path", move |this, _, cx| {
                        this.copy_text(copied.clone(), cx)
                    }),
                    MenuItem::new("Archive", move |this, _, cx| {
                        let (project, name) = (project.clone(), name.clone());
                        this.store.update(cx, |store, cx| {
                            store.set_worktree_archived(&project, name, true, cx)
                        })
                    })
                    .separated(),
                    MenuItem::new("Delete worktree…", move |this, window, cx| {
                        let (project, name) = (deleted_project.clone(), deleted.clone());
                        let detail = match dirty {
                            0 => "Its folder is removed from disk. Its branch stays.".to_owned(),
                            n => format!(
                                "Its folder is removed from disk, with {}, which are lost. \
                                 Its branch stays.",
                                plural(n, "uncommitted file")
                            ),
                        };
                        let message = format!("Delete worktree {name}?");
                        this.confirm_then(
                            message,
                            &detail,
                            "Delete",
                            window,
                            cx,
                            move |this, cx| {
                                this.store.update(cx, |store, cx| {
                                    store.delete_worktree(&project, name, dirty > 0, cx)
                                })
                            },
                        )
                    })
                    .danger(),
                ]
            }
            Selection::Stack(project, bottom) => {
                let key = (project.clone(), bottom.clone());
                let mut items = vec![match self.folded_stacks.contains(&key) {
                    true => action("Show branches", &Expand),
                    false => action("Hide branches", &Collapse),
                }];
                let Some(workspace) = store.workspace(project) else {
                    return items;
                };
                // Through its top branch, which takes the ones below it along.
                let Some(top) = workspace.lane(bottom).and_then(<[_]>::first) else {
                    return items;
                };
                let top = &top.ref_name;
                if workspace.can_push(top) && workspace.push_refused(top).is_none() {
                    items.push(
                        action(
                            match workspace.needs_force(top) {
                                true => "Force push stack",
                                false => "Push stack",
                            },
                            &Push,
                        )
                        .separated(),
                    );
                }
                if let Some(target) = workspace.target.as_ref()
                    && BranchEdits::of(workspace, top).land == Some(true)
                {
                    items.push(MenuItem::action(
                        format!("Land the stack on {}…", target.name),
                        &crate::edits::LandBranch,
                        cx,
                    ));
                }
                if store.worktree_of(target).is_none() {
                    items.push(action("New branch on top…", &NewBranchOnTop).separated());
                    if workspace.managed {
                        items.push(action("Unapply stack", &UnapplyStack));
                    }
                }
                items
            }
            Selection::Branches(_) | Selection::History(_) => Vec::new(),
        }
    }
}
