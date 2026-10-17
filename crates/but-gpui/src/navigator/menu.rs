//! The navigator's right-click menus.

use gpui::App;

use super::Navigator;
use crate::app::{
    Collapse, CopySelection, CutSelection, DeleteSelection, Expand, Fetch, NewBranchOnTop,
    OpenPullRequest, Push, RenameSelection, StartCommit, UncommitSelection, UpdateFromTarget,
};
use crate::edits::{
    AddEmptyCommitAbove, AddEmptyCommitBelow, CherryPick, CopyChangeId, CopyMessage,
    MoveToNewBranch, NewBranchBelow, NewBranchHere, PullRemote, RewrapMessage, SquashAll,
    SquashPicked, TakeRemote, UnapplyStack, Unstack,
};
use crate::overlays::MenuItem;
use crate::state::Selection;
use crate::ui::plural;

impl Navigator {
    /// What right-clicking `target` offers. Right-clicking selects the row first, so the
    /// items run the same actions as their keys. A branch with commits starts with hiding
    /// or showing them.
    pub(super) fn menu_items(&self, target: &Selection, cx: &App) -> Vec<MenuItem> {
        let mut items = self.row_actions(target, cx);
        if let Selection::Branch(project, ref_name) = target
            && let Some(first) = items.first_mut()
            && (self.store.read(cx).branch_for(target)).is_some_and(|b| !b.commits.is_empty())
        {
            *first = first.clone().separated();
            let key = (project.clone(), ref_name.clone());
            items.insert(
                0,
                match self.closed_branches.contains(&key) {
                    true => MenuItem::action("Show commits", &Expand, cx),
                    false => MenuItem::action("Hide commits", &Collapse, cx),
                },
            );
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
                    action("Collapse", &Collapse)
                } else {
                    action("Expand", &Expand)
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
                items
            }
            Selection::Uncommitted(project) => {
                if store
                    .workspace(project)
                    .is_none_or(|ws| ws.uncommitted.is_empty())
                {
                    return Vec::new();
                }
                vec![
                    action("Commit…", &StartCommit),
                    action("Discard checked changes…", &DeleteSelection).danger(),
                ]
            }
            Selection::Branch(project, ref_name) => {
                let mut items = Vec::new();
                let (Some(branch), Some(workspace)) =
                    (store.branch_for(target), store.workspace(project))
                else {
                    return items;
                };
                if branch.push.can_push() {
                    let label = if workspace.needs_force(ref_name) {
                        "Force push"
                    } else {
                        "Push"
                    };
                    items.push(action(label, &Push));
                }
                if workspace.can_pull(ref_name) {
                    items.extend([
                        MenuItem::action(
                            format!(
                                "Pull {} from the remote",
                                plural(branch.incoming.len(), "commit")
                            ),
                            &PullRemote,
                            cx,
                        ),
                        action("Take the remote's version…", &TakeRemote),
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
                if crate::pr::on_forge(store, target) && !ref_name.is_empty() {
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
                    return items;
                }
                items.push(action("New branch on top…", &NewBranchOnTop).separated());
                if !branch.commits.is_empty() {
                    items.push(action("New branch below…", &NewBranchBelow));
                }
                items.push(action("Add empty commit", &AddEmptyCommitAbove));
                if branch.commits.len() > 1 {
                    items.push(action("Squash all commits", &SquashAll));
                }
                let stacked = workspace
                    .stack_of(ref_name)
                    .is_some_and(|(stack, _)| stack.branches.len() > 1);
                items.push(action("Cut", &CutSelection).separated());
                if stacked {
                    items.push(action("Unstack", &Unstack));
                }
                if let (Some(target), Some(whole)) = (
                    workspace.target.as_ref(),
                    crate::edits::land_scope(workspace, ref_name),
                ) {
                    let label = if whole {
                        format!("Land the stack on {}…", target.name)
                    } else {
                        format!("Land on {}…", target.name)
                    };
                    items.push(MenuItem::action(label, &crate::edits::LandBranch, cx));
                }
                items.push(action(
                    if stacked {
                        "Unapply stack"
                    } else {
                        "Unapply branch"
                    },
                    &UnapplyStack,
                ));
                items.extend([
                    action("Rename…", &RenameSelection).separated(),
                    action("Copy name", &CopySelection),
                    action("Delete branch…", &DeleteSelection).danger(),
                ]);
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
                items
            }
            Selection::Commit(project, id) => {
                let Some((branch, commit)) = store.find_commit(project, *id) else {
                    return Vec::new();
                };
                let mut items = Vec::new();
                if commit.conflicted {
                    items.push(action(
                        "Resolve conflicts…",
                        &crate::resolve::ResolveConflicts,
                    ));
                }
                items.extend([
                    action("Cut", &CutSelection).separated(),
                    action("Cherry-pick", &CherryPick),
                    action("Edit message…", &RenameSelection).separated(),
                ]);
                if crate::edits::needs_rewrap(&commit.message) {
                    items.push(action("Rewrap message", &RewrapMessage));
                }
                items.extend([
                    action("Add empty commit above", &AddEmptyCommitAbove),
                    action("Add empty commit below", &AddEmptyCommitBelow),
                ]);
                if !branch.ref_name.is_empty() && branch.commits.first().map(|c| c.id) != Some(*id)
                {
                    items.push(action("New branch ending here…", &NewBranchHere));
                }
                items.extend([
                    action("Move to a new branch", &MoveToNewBranch).separated(),
                    action("Copy commit ID", &CopySelection).separated(),
                ]);
                if commit.has_change_id() {
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
                let Some(worktree) = (store.workspace(project))
                    .and_then(|ws| ws.worktrees.iter().find(|w| w.name == *name))
                else {
                    return Vec::new();
                };
                let path = worktree.path.clone();
                let copied = path.display().to_string();
                let (project, name) = (project.clone(), name.clone());
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
                ]
            }
            Selection::Branches(_) | Selection::History(_) => Vec::new(),
        }
    }
}
