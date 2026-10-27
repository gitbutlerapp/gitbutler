//! The navigator's right-click menus.

use gpui::{Action, App};

use super::{ClickOpened, Navigator};
use crate::app::{
    Collapse, CopySelection, CutSelection, DeleteSelection, Expand, Fetch, MarkAllRead, NewBranch,
    NewBranchOnTop, OpenPullRequest, Push, RenameSelection, StartCommit, ToggleRead,
    UncommitSelection, UpdateFromTarget,
};
use crate::backend::{Checkout, CommitId, short_ref};
use crate::edits::{
    AddEmptyCommitAbove, AddEmptyCommitBelow, BranchEdits, CherryPick, CommitEdits, CopyChangeId,
    CopyMessage, MoveToNewBranch, NewBranchBelow, NewBranchHere, PullRemote, RewrapMessage,
    SquashAll, SquashPicked, TakeRemote, UnapplyStack, Unstack,
};
use crate::overlays::MenuItem;
use crate::state::{Cut, ProjectId, Seeable, Selection, Store, WatchedOrder};
use crate::ui::{commit_title, plural, quoted};

/// Marking `commits` of `project` read, the ones that aren't, and unread, the ones that
/// are: one item or the other, or both when some are each. The first is what u does.
fn read_marks<'a, C: Seeable + 'a>(
    store: &Store,
    project: &ProjectId,
    commits: impl IntoIterator<Item = &'a C>,
    cx: &App,
) -> Vec<MenuItem> {
    let (unread, read): (Vec<&C>, Vec<&C>) =
        (commits.into_iter()).partition(|commit| store.is_unread(project, *commit));
    let mut items = Vec::new();
    if !unread.is_empty() {
        items.push(MenuItem::action("Mark as read", &ToggleRead, cx));
    }
    if !read.is_empty() && unread.is_empty() {
        items.push(MenuItem::action("Mark as unread", &ToggleRead, cx));
    } else if !read.is_empty() {
        let ids: Vec<CommitId> = read.iter().map(|commit| commit.id()).collect();
        let project = project.clone();
        items.push(MenuItem::new("Mark as unread", move |this, _, cx| {
            let ids = ids.clone();
            (this.store).update(cx, |store, cx| store.mark_read(&project, ids, false, cx))
        }));
    }
    if let Some(first) = items.first_mut() {
        first.separated = true;
    }
    items
}

/// Starting a branch from a project or its main worktree: a new one beside the others, or on
/// top of the default branch when that's checked out on its own; or one in a new linked
/// worktree, as `but worktree new` makes.
fn new_branch_items(store: &Store, project: &ProjectId, cx: &App) -> Vec<MenuItem> {
    let managed = store.workspace(project).is_some_and(|ws| ws.managed);
    let mut items = vec![MenuItem::action("New branch", &NewBranch, cx)];
    if managed {
        let project = project.clone();
        items.push(MenuItem::new("New worktree…", move |app, window, cx| {
            let selected = project.clone();
            app.prompt_branch(
                Selection::Project(project.clone()),
                "New worktree",
                window,
                cx,
                |path, _, name| crate::backend::worktrees::create(path, name),
                move |store, name, cx| store.select(Some(Selection::Worktree(selected, name)), cx),
            )
        }));
    }
    items
}

/// Updating `project` from its target, when that would change anything.
fn update_item(store: &Store, project: &ProjectId, cx: &App) -> Option<MenuItem> {
    let target = (store.workspace(project)?.target.as_ref()).filter(|t| t.needs_update)?;
    let label = format!("Update from {}", target.name);
    Some(MenuItem::action(label, &UpdateFromTarget, cx))
}

impl Navigator {
    /// What right-clicking `target` offers. Right-clicking selects the row first, so the
    /// items run the same actions as their keys. A branch with commits starts with hiding
    /// or showing them, and one with unread commits, its own or its remote's, with marking
    /// them read.
    pub(crate) fn menu_items(&self, target: &Selection, cx: &App) -> Vec<MenuItem> {
        let mut items = self.row_actions(target, cx);
        let store = self.store.read(cx);
        if let Selection::Branch(project, ref_name) = target
            && let Some(branch) = store.branch_for(target)
        {
            let mut view = Vec::new();
            // On the navigator, as the page header's ⋯ has the detail focused, where the keys
            // don't reach the navigator.
            if !branch.commits.is_empty() {
                let key = ClickOpened::Here((project.clone(), ref_name.clone()));
                let open = self.is_open(&key);
                let (label, keys): (_, &dyn Action) = match open {
                    true => ("Hide commits", &Collapse),
                    false => ("Show commits", &Expand),
                };
                view.push(MenuItem::keyed(label, keys, cx, move |this, _, cx| {
                    let key = key.clone();
                    (this.navigator).update(cx, |nav, cx| nav.set_open(key, !open, cx))
                }));
            }
            if store.read_marks_of(target).is_some() {
                view.push(MenuItem::action("Mark all as read", &ToggleRead, cx));
            }
            if let Some(first) = items.first_mut().filter(|_| !view.is_empty()) {
                first.separated = true;
            }
            items.splice(0..0, view);
        }
        items
    }

    /// Commit… and discarding the changes the composer has checked, or all of them, for
    /// `project`'s `checkout` with uncommitted changes; a linked worktree's discard asks first.
    fn changes_items(&self, project: &ProjectId, checkout: Checkout, cx: &App) -> [MenuItem; 2] {
        // Only the composer checks files, and only its own checkout's: without it, or for
        // another checkout right-clicked while it's open, discarding takes them all.
        // The composer's own checkout, not the selection's, which a right-click has just moved.
        let what = match self.detail.read(cx).composing_in(project, &checkout) {
            true => "checked changes",
            false => "all changes",
        };
        [
            MenuItem::action("Commit…", &StartCommit, cx).separated(),
            MenuItem::action(
                crate::edits::discard_label(what, checkout.is_linked()),
                &DeleteSelection,
                cx,
            )
            .danger(),
        ]
    }

    fn row_actions(&self, target: &Selection, cx: &App) -> Vec<MenuItem> {
        let store = self.store.read(cx);
        let action =
            |label: &'static str, action: &dyn gpui::Action| MenuItem::action(label, action, cx);
        match target {
            // A remote's commit can be copied into a branch here, or its ID copied, and a
            // branch's marked read or unread.
            Selection::Upstream(project, ref_name, id) => {
                let commit = (ref_name.as_ref()).and_then(|ref_name| {
                    let commits = store.workspace(project)?.upstream(Some(ref_name));
                    commits.iter().find(|commit| commit.id == *id)
                });
                let marks = read_marks(store, project, commit, cx);
                let (project, id) = (project.clone(), *id);
                let mut items = vec![
                    MenuItem::new("Cherry-pick", move |app, _, cx| {
                        app.cut(Cut::CherryPick(project.clone(), [id].into()), cx)
                    }),
                    MenuItem::keyed("Copy commit ID", &CopySelection, cx, move |app, _, cx| {
                        app.copy_text(id.0.to_string(), cx)
                    }),
                ];
                items.extend(marks);
                items
            }
            Selection::Project(project) => {
                let expanded = store.project(project).is_some_and(|p| p.expanded);
                let id = project.clone();
                let mut items = vec![if expanded {
                    // At once: Collapse (h) folds an open card before it stops watching it.
                    MenuItem::new("Stop watching", move |this, _, cx| {
                        this.store
                            .update(cx, |store, cx| store.set_expanded(&id, false, cx))
                    })
                } else {
                    MenuItem::keyed("Watch", &Expand, cx, move |this, _, cx| {
                        this.store
                            .update(cx, |store, cx| store.set_expanded(&id, true, cx))
                    })
                }];
                if store.read_marks_of(target).is_some() {
                    items.push(MenuItem::action("Mark all as read", &MarkAllRead, cx));
                }
                if expanded {
                    // Its card swaps places with the watched one's above or below: it goes
                    // before the one above, or before the one after the one below.
                    let watched = store.expanded_ids();
                    let at = watched.iter().position(|id| id == project).unwrap_or(0);
                    let up = at.checked_sub(1).map(|above| watched.get(above));
                    let down = (at + 1 < watched.len()).then(|| watched.get(at + 2));
                    // Only the manual order moves by hand.
                    if store.prefs.watched_order == WatchedOrder::Manual {
                        for (label, before) in [("Move up", up), ("Move down", down)] {
                            if let Some(before) = before {
                                let (id, before) = (project.clone(), before.cloned());
                                items.push(MenuItem::new(label, move |this, _, cx| {
                                    (this.store).update(cx, |store, cx| {
                                        store.move_watched(&id, before.as_ref(), cx)
                                    })
                                }));
                            }
                        }
                    }
                    let mut branch = new_branch_items(store, project, cx);
                    if let Some(first) = branch.first_mut() {
                        first.separated = true;
                    }
                    items.extend(branch);
                    items.push(action("Fetch", &Fetch).separated());
                }
                // An unwatched project's too, once its page has read it.
                if let Some(update) = update_item(store, project, cx) {
                    // With Fetch, else apart from Watch.
                    items.push(if expanded { update } else { update.separated() });
                }
                let archived = (store.workspace(project)).map_or_else(Vec::new, |ws| {
                    (ws.archived_worktrees.iter())
                        .map(|(name, _)| name.clone())
                        .collect()
                });
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
                            let place = this
                                .store
                                .read(cx)
                                .place(&Selection::Project(project.clone()));
                            this.open_picker(
                                Some(place),
                                "Show a worktree again",
                                "Show",
                                choices,
                                window,
                                cx,
                            )
                        },
                    ));
                }
                if let Some(path) = store.path(project) {
                    let label = crate::open_in::editor_label(store);
                    let (editor_project, terminal_project) = (project.clone(), project.clone());
                    items.extend([
                        MenuItem::new(label, move |this, window, cx| {
                            let position = window.mouse_position();
                            let main = Checkout::Main;
                            this.open_in_editor(&editor_project, main, None, None, position, cx)
                        })
                        .separated(),
                        MenuItem::keyed(
                            "Open in terminal",
                            &crate::open_in::OpenInTerminal,
                            cx,
                            move |this, _, cx| this.open_in_terminal(&terminal_project, cx),
                        ),
                        MenuItem::new(crate::ui::REVEAL, move |_, _, cx| cx.reveal_path(&path)),
                    ]);
                }
                // The note saying why the project can't be read may name a command to run.
                if let Some(error) = store.project(project).and_then(|p| p.error.clone()) {
                    items.push(MenuItem::new("Copy error message", move |_, _, cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(error.to_string()))
                    }));
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
                let mut items = new_branch_items(store, project, cx);
                items.push(action("Fetch", &Fetch).separated());
                items.extend(update_item(store, project, cx));
                if store
                    .workspace(project)
                    .is_none_or(|ws| ws.uncommitted.is_empty())
                {
                    return items;
                }
                items.extend(self.changes_items(project, Checkout::Main, cx));
                items
            }
            Selection::Branch(project, ref_name) => {
                let mut items = Vec::new();
                let (Some(branch), Some(workspace)) =
                    (store.branch_for(target), store.workspace(project))
                else {
                    return items;
                };
                let named = ref_name.as_ref();
                if let Some(ref_name) = named
                    && workspace.push_blocked(Some(ref_name)).is_none()
                {
                    items.push(action(workspace.push_label(ref_name), &Push));
                }
                let linked = store.in_linked(target);
                if named.is_some_and(|ref_name| workspace.can_pull(ref_name)) {
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
                if crate::pr::create_offer(store, target) == Some(Ok(())) {
                    items.push(action(
                        "Create pull request…",
                        &crate::pr::CreatePullRequest,
                    ))
                }
                if crate::pr::on_forge(store, target) && named.is_some() {
                    let project = target.project();
                    let open = (branch.review.as_ref()).filter(|r| {
                        store.review_state(project, r) == crate::backend::ReviewState::Open
                    });
                    if let Some(review) = open {
                        let offer = crate::pr::mergeability(store, project, review).offer();
                        let method = crate::pr::merge_method(store, project);
                        items.extend(offer.map(|offer| action(offer.label(method), offer.action())))
                    }
                }
                let Some(ref_name) = named else {
                    if !branch.commits.is_empty() {
                        items.push(action("Name branch…", &RenameSelection).separated());
                    }
                    return items;
                };
                let edits = BranchEdits::of(workspace, ref_name);
                items.push(action("New branch on top", &NewBranchOnTop).separated());
                if edits.new_below {
                    items.push(action("New branch below", &NewBranchBelow));
                }
                items.push(action("Add empty commit", &AddEmptyCommitAbove));
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
                if workspace.managed && workspace.stack_of(Some(ref_name)).is_some() {
                    items.push(action(
                        if edits.stacked {
                            "Unapply stack"
                        } else {
                            "Unapply branch"
                        },
                        &UnapplyStack,
                    ));
                }
                let name = short_ref(ref_name).into_owned();
                items.extend([
                    action("Rename…", &RenameSelection).separated(),
                    MenuItem::keyed("Copy name", &CopySelection, cx, move |app, _, cx| {
                        app.copy_text(name.clone(), cx)
                    }),
                ]);
                // A linked worktree keeps the branch it has checked out, its lane's top one.
                let checked_out = linked
                    && workspace
                        .lane_of(Some(ref_name))
                        .is_some_and(|(_, at)| at == 0);
                if !checked_out {
                    items.push(action("Delete branch", &DeleteSelection).danger());
                }
                items
            }
            Selection::Commit(project, id) if store.is_picked(project, *id) => {
                let n = store.picked.len();
                let ids = store.with_picks(project, *id);
                let picked: Vec<_> = (ids.iter())
                    .filter_map(|id| store.find_commit(project, *id).map(|(_, commit)| commit))
                    .collect();
                // Commits list newest first. The oldest can be in another branch of the stack
                // than the one clicked, so it's named.
                let Some(&oldest) = picked.last() else {
                    return Vec::new();
                };
                let cut = (project.clone(), ids);
                let mut items = vec![
                    MenuItem::keyed(
                        format!("Cut {n} commits"),
                        &CutSelection,
                        cx,
                        move |app, _, cx| app.cut(Cut::Commit(cut.0.clone(), cut.1.clone()), cx),
                    ),
                    MenuItem::action(format!("Cherry-pick {n} commits"), &CherryPick, cx),
                    MenuItem::action(
                        format!(
                            "Squash {n} commits into {}",
                            quoted(&commit_title(&oldest.title))
                        ),
                        &SquashPicked,
                        cx,
                    )
                    .separated(),
                ];
                // The other way round: the rest go into the one clicked, in its place.
                if oldest.id != *id {
                    let into = *id;
                    items.push(MenuItem::new(
                        format!("Squash {n} commits into this one"),
                        move |this, _, cx| this.squash_picks(Some(into), cx),
                    ));
                }
                if store.workspace(project).is_some_and(|ws| ws.managed) {
                    let label = format!("Move {n} commits to a new branch");
                    items.push(MenuItem::action(label, &MoveToNewBranch, cx).separated());
                }
                let conflicted = picked.iter().any(|commit| commit.conflicted);
                // A run picks commits without opening them, as clicking one does.
                items.extend(read_marks(store, project, picked, cx));
                let discard =
                    MenuItem::action(format!("Discard {n} commits"), &DeleteSelection, cx).danger();
                items.extend(match conflicted {
                    true => vec![discard.separated()],
                    false => vec![
                        MenuItem::action(format!("Uncommit {n} commits"), &UncommitSelection, cx)
                            .separated(),
                        discard,
                    ],
                });
                items
            }
            Selection::Commit(project, id) => {
                let Some((branch, commit)) = store.find_commit(project, *id) else {
                    return Vec::new();
                };
                let edits = CommitEdits::of(branch, commit);
                // A linked worktree's commits stay in it, and their conflicts resolve there.
                let linked = store.in_linked(target);
                let mut items = Vec::new();
                if commit.conflicted && !linked {
                    items.push(action(
                        "Resolve conflicts…",
                        &crate::detail::ResolveConflicts,
                    ));
                }
                let (cut_project, id) = (project.clone(), *id);
                let cut = MenuItem::keyed("Cut", &CutSelection, cx, move |app, _, cx| {
                    let ids = app.store.read(cx).with_picks(&cut_project, id);
                    app.cut(Cut::Commit(cut_project.clone(), ids), cx)
                });
                items.extend([
                    cut.separated(),
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
                if edits.new_branch_here {
                    items.push(action("New branch ending here", &NewBranchHere));
                }
                if store.workspace(project).is_some_and(|ws| ws.managed) {
                    items.push(action("Move to a new branch", &MoveToNewBranch).separated());
                }
                let mut copy_id =
                    MenuItem::keyed("Copy commit ID", &CopySelection, cx, move |app, _, cx| {
                        app.copy_text(id.0.to_string(), cx)
                    })
                    .separated();
                if edits.change_id {
                    let mut change_id = action("Copy change ID", &CopyChangeId);
                    change_id.key = copy_id.key.take().or(change_id.key);
                    items.extend([copy_id, change_id]);
                } else {
                    items.push(copy_id);
                }
                items.push(action("Copy message", &CopyMessage));
                items.extend(read_marks(store, project, [commit], cx));
                let discard = action("Discard commit", &DeleteSelection).danger();
                items.extend(match commit.conflicted {
                    // Resolved, not uncommitted, as its files are one side's.
                    true => vec![discard.separated()],
                    false => vec![action("Uncommit", &UncommitSelection).separated(), discard],
                });
                items
            }
            Selection::Worktree(project, name) => {
                let Some(worktree) =
                    (store.workspace(project)).and_then(|ws| ws.find_worktree(name.as_ref()))
                else {
                    return Vec::new();
                };
                let path = worktree.path.clone();
                let copied = path.display().to_string();
                // A detached `HEAD` has no branch to keep what was committed there.
                let after = match worktree.branch {
                    Some(_) => "Its branch stays.",
                    None => "It has no branch, so commits made there that no branch has are lost.",
                };
                // As last written, so an agent's edit while the question is up isn't lost.
                let lost: Vec<(String, Option<i64>, Option<u64>)> = (worktree.uncommitted.iter())
                    .map(|f| (f.path.clone(), f.modified, f.size))
                    .collect();
                let (project, name) = (project.clone(), name.clone());
                let (deleted_project, deleted) = (project.clone(), name.clone());
                let linked = Checkout::Linked(name.clone());
                let changes = (!lost.is_empty()).then(|| self.changes_items(&project, linked, cx));
                let mut items = vec![
                    MenuItem::new(crate::ui::REVEAL, move |_, _, cx| cx.reveal_path(&path)),
                    MenuItem::keyed("Copy path", &CopySelection, cx, move |this, _, cx| {
                        this.copy_text(copied.clone(), cx)
                    }),
                    // Not Update: it rebases the workspace's stacks, not this worktree's.
                    action("Fetch", &Fetch).separated(),
                    MenuItem::new("Archive", move |this, _, cx| {
                        let (project, name) = (project.clone(), name.clone());
                        this.store.update(cx, |store, cx| {
                            store.set_worktree_archived(&project, name, true, cx)
                        })
                    })
                    .separated(),
                    MenuItem::new("Delete worktree…", move |this, window, cx| {
                        let (project, name, lost) =
                            (deleted_project.clone(), deleted.clone(), lost.clone());
                        let detail = match lost.len() {
                            0 => format!("Its folder is removed from disk. {after}"),
                            n => format!(
                                "Its folder is removed from disk, and {} with it. {after}",
                                plural(n, "uncommitted file")
                            ),
                        };
                        let message = format!("Delete worktree {name}?");
                        this.confirm_then(
                            Some(Selection::Project(project.clone())),
                            message,
                            &detail,
                            "Delete",
                            window,
                            cx,
                            move |this, cx| {
                                this.store.update(cx, |store, cx| {
                                    store.delete_worktree(&project, name, lost, cx)
                                })
                            },
                        )
                    })
                    .danger(),
                ];
                // As Main worktree's, after Fetch and before what happens to it.
                items.splice(3..3, changes.into_iter().flatten());
                items
            }
            Selection::Stack(project, bottom) => {
                let key = (project.clone(), bottom.clone());
                let folded = self.folded_stacks.contains(&key);
                let (label, keys): (_, &dyn Action) = match folded {
                    true => ("Show branches", &Expand),
                    false => ("Hide branches", &Collapse),
                };
                let (fold_project, fold_bottom) = key;
                let mut items = vec![MenuItem::keyed(label, keys, cx, move |this, window, cx| {
                    (this.navigator).update(cx, |nav, cx| {
                        nav.toggle_stack(&fold_project, fold_bottom.as_ref(), !folded, window, cx)
                    })
                })];
                let Some(workspace) = store.workspace(project) else {
                    return items;
                };
                // Through its top branch, which takes the ones below it along.
                let Some(top) = workspace
                    .lane(bottom.as_ref())
                    .and_then(|lane| lane.branches.first())
                else {
                    return items;
                };
                let top = top.ref_name.as_ref();
                if let Some(top) = top
                    && workspace.push_blocked(Some(top)).is_none()
                {
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
                let named = top.is_some();
                if let Some(target) = workspace.target.as_ref()
                    && let Some(top) = top
                    && BranchEdits::of(workspace, top).land == Some(true)
                {
                    items.push(MenuItem::action(
                        format!("Land the stack on {}…", target.name),
                        &crate::edits::LandBranch,
                        cx,
                    ));
                }
                items.push(action("New branch on top", &NewBranchOnTop).separated());
                // A linked worktree's lane stays applied while it has it checked out.
                if workspace.managed && named && workspace.stack_of(bottom.as_ref()).is_some() {
                    items.push(action("Unapply stack", &UnapplyStack));
                }
                items
            }
            Selection::Mesh(..) => self.mesh_menu(target, cx),
            Selection::Branches(_) | Selection::History(_) | Selection::Section(..) => Vec::new(),
        }
    }
}
