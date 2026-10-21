//! The navigator's right-click menus.

use gpui::{Action, App, Context, SharedString, Window};

use super::{ClickOpened, Navigator};
use crate::app::{
    AppView, Collapse, CopySelection, CutSelection, DeleteSelection, Expand, Fetch, NewBranch,
    NewBranchOnTop, OpenPullRequest, Push, RenameSelection, StartCommit, UncommitSelection,
    UpdateFromTarget,
};
use crate::backend::{CommitId, short_ref};
use crate::edits::{
    AddEmptyCommitAbove, AddEmptyCommitBelow, BranchEdits, CherryPick, CommitEdits, CopyChangeId,
    CopyMessage, MoveToNewBranch, NewBranchBelow, NewBranchHere, PullRemote, RewrapMessage,
    SquashAll, SquashPicked, TakeRemote, UnapplyStack, Unstack,
};
use crate::overlays::MenuItem;
use crate::state::{Cut, ProjectId, Seeable, Selection, Store, WatchedOrder};
use crate::ui::{plural, quoted, shortcut};

/// An item that runs `handler` on the row it opened on, showing the keys of `action`, which
/// does the same from the navigator. The ⋯ in a page's header opens these menus with the
/// focus in the detail, where `action` would act on the diff instead, as ⌘C copies the path
/// of the file under its cursor, or do nothing, as → does there.
pub(super) fn keyed(
    label: impl Into<SharedString>,
    action: &dyn Action,
    cx: &App,
    handler: impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
) -> MenuItem {
    MenuItem {
        key: shortcut(action, cx),
        ..MenuItem::new(label, handler)
    }
}

/// An item marking `ids`, commits of `project`, read, or with `read` false unread.
fn mark_read(label: &str, project: &ProjectId, ids: Vec<CommitId>, read: bool) -> MenuItem {
    let project = project.clone();
    MenuItem::new(label.to_owned(), move |this, _, cx| {
        let ids = ids.clone();
        (this.store).update(cx, |store, cx| store.mark_read(&project, ids, read, cx))
    })
}

/// Marking `commits` of `project` read, the ones that aren't, and unread, the ones that
/// are: one item or the other, or both when some are each.
fn read_marks<'a, C: Seeable + 'a>(
    store: &Store,
    project: &ProjectId,
    commits: impl IntoIterator<Item = &'a C>,
) -> Vec<MenuItem> {
    let (unread, read): (Vec<&C>, Vec<&C>) =
        (commits.into_iter()).partition(|commit| store.is_unread(project, *commit));
    let ids = |commits: Vec<&C>| commits.iter().map(|commit| commit.id()).collect();
    let mut items = Vec::new();
    if !unread.is_empty() {
        items.push(mark_read("Mark as read", project, ids(unread), true));
    }
    if !read.is_empty() {
        items.push(mark_read("Mark as unread", project, ids(read), false));
    }
    if let Some(first) = items.first_mut() {
        first.separated = true;
    }
    items
}

/// Starting a branch from a project or its main worktree: a new one beside the others, or on
/// top of the one checked out; or, with linked worktrees on, one in a new linked worktree, as
/// `but worktree new` makes.
fn new_branch_items(store: &Store, project: &ProjectId, cx: &App) -> Vec<MenuItem> {
    let managed = store.workspace(project).is_some_and(|ws| ws.managed);
    let mut items = vec![MenuItem::action("New branch…", &NewBranch, cx)];
    if managed && crate::backend::settings::worktrees_enabled() {
        let project = project.clone();
        items.push(MenuItem::new("New worktree…", move |app, window, cx| {
            let selected = project.clone();
            app.prompt_branch(
                project.clone(),
                "New worktree",
                None,
                window,
                cx,
                |path, _, name| crate::backend::worktrees::create(path, name),
                move |store, name, cx| {
                    store.select(Some(Selection::Worktree(selected, name.into())), cx)
                },
            )
        }));
    }
    items
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
            // On the navigator, as the page header's ⋯ has the detail focused, where the keys
            // don't reach the navigator.
            let key = (project.clone(), ref_name.clone());
            let open = self.branch_open(&key);
            let (label, keys): (_, &dyn Action) = match open {
                true => ("Hide commits", &Collapse),
                false => ("Show commits", &Expand),
            };
            let mut view = vec![keyed(label, keys, cx, move |this, _, cx| {
                let key = ClickOpened::Here(key.clone());
                (this.navigator).update(cx, |nav, cx| nav.set_open(key, !open, cx))
            })];
            let unread: Vec<CommitId> = (branch.commits.iter())
                .filter(|commit| store.is_unread(project, *commit))
                .map(|commit| commit.id)
                .chain(
                    (branch.incoming.iter())
                        .filter(|commit| store.is_unread(project, *commit))
                        .map(|commit| commit.id),
                )
                .collect();
            if !unread.is_empty() {
                view.push(mark_read("Mark all as read", project, unread, true));
            }
            if let Some(first) = items.first_mut() {
                first.separated = true;
            }
            items.splice(0..0, view);
        }
        items
    }

    /// Commit… and discarding the changes the composer has checked, or all of them, for a
    /// checkout with uncommitted changes; a `linked` worktree's discard asks first.
    fn changes_items(&self, linked: bool, cx: &App) -> [MenuItem; 2] {
        // Only the composer checks files; without it, discarding takes them all.
        let detail = self.detail.read(cx);
        let what = match detail.composing() {
            true => "checked changes",
            false => "all changes",
        };
        let count = detail.selected_changes(cx).len();
        [
            MenuItem::action("Commit…", &StartCommit, cx).separated(),
            MenuItem::action(
                crate::edits::discard_label(what, linked, count),
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
                let commit = (ref_name.as_deref()).and_then(|ref_name| {
                    let commits = store.workspace(project)?.upstream(Some(ref_name));
                    commits.iter().find(|commit| commit.id == *id)
                });
                let marks = read_marks(store, project, commit);
                let (project, id) = (project.clone(), *id);
                let mut items = vec![
                    MenuItem::new("Cherry-pick", move |app, _, cx| {
                        app.cut(Cut::CherryPick(project.clone(), [id].into()), cx)
                    }),
                    MenuItem::new("Copy commit ID", move |app, _, cx| {
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
                    keyed("Watch", &Expand, cx, move |this, _, cx| {
                        this.store
                            .update(cx, |store, cx| store.set_expanded(&id, true, cx))
                    })
                }];
                if expanded {
                    // Its card swaps places with the watched one's above or below: it goes
                    // before the one above, or before the one after the one below.
                    let watched = store.expanded_ids();
                    let at = watched.iter().position(|id| id == project).unwrap_or(0);
                    let up = at.checked_sub(1).map(|above| Some(&watched[above]));
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
                if let Some(target) = (store.workspace(project))
                    .and_then(|ws| ws.target.as_ref())
                    .filter(|target| target.needs_update)
                {
                    let update = MenuItem::action(
                        format!("Update from {}", target.name),
                        &UpdateFromTarget,
                        cx,
                    );
                    // With Fetch, else apart from Watch.
                    items.push(if expanded { update } else { update.separated() });
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
                let mut items = new_branch_items(store, project, cx);
                if store
                    .workspace(project)
                    .is_none_or(|ws| ws.uncommitted.is_empty())
                {
                    return items;
                }
                items.extend(self.changes_items(false, cx));
                items
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
                    match (branch.review.as_ref()).map(|r| store.review_state(target.project(), r))
                    {
                        // Creating one pushes first.
                        None if !branch.commits.is_empty()
                            && workspace.push_refused(ref_name).is_none() =>
                        {
                            items.push(action(
                                "Create pull request…",
                                &crate::pr::CreatePullRequest,
                            ))
                        }
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
                items.push(action("New branch on top…", &NewBranchOnTop).separated());
                if edits.new_below {
                    items.push(action("New branch below…", &NewBranchBelow));
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
                let name = short_ref(ref_name).to_owned();
                items.extend([
                    action("Rename…", &RenameSelection).separated(),
                    keyed("Copy name", &CopySelection, cx, move |app, _, cx| {
                        app.copy_text(name.clone(), cx)
                    }),
                ]);
                // A linked worktree keeps the branch it has checked out, its lane's top one.
                let checked_out =
                    linked && workspace.lane_of(ref_name).is_some_and(|(_, at)| at == 0);
                if !checked_out {
                    items.push(action("Delete branch…", &DeleteSelection).danger());
                }
                items
            }
            Selection::Commit(project, id) if store.is_picked(project, *id) => {
                let n = store.picked.len();
                // Commits list newest first. The oldest can be in another stack or worktree
                // than the one clicked, so it's named.
                let Some(oldest) = (store.workspace(project))
                    .and_then(|ws| ws.commits().filter(|c| store.picked.contains(&c.id)).last())
                else {
                    return Vec::new();
                };
                let mut items = vec![MenuItem::action(
                    format!("Squash {n} commits into {}", quoted(&oldest.title)),
                    &SquashPicked,
                    cx,
                )];
                // The other way round: the rest go into the one clicked, in its place.
                if oldest.id != *id {
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
                let picked = (store.workspace(project).into_iter())
                    .flat_map(|ws| ws.commits())
                    .filter(|commit| store.picked.contains(&commit.id));
                items.extend(read_marks(store, project, picked));
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
                let (cut_project, id) = (project.clone(), *id);
                let cut = keyed("Cut", &CutSelection, cx, move |app, _, cx| {
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
                if edits.new_branch_here && !linked {
                    items.push(action("New branch ending here…", &NewBranchHere));
                }
                if store.workspace(project).is_some_and(|ws| ws.managed) {
                    items.push(action("Move to a new branch", &MoveToNewBranch).separated());
                }
                items.push(
                    keyed("Copy commit ID", &CopySelection, cx, move |app, _, cx| {
                        app.copy_text(id.0.to_string(), cx)
                    })
                    .separated(),
                );
                if edits.change_id {
                    items.push(action("Copy change ID", &CopyChangeId));
                }
                items.push(action("Copy message", &CopyMessage));
                items.extend(read_marks(store, project, [commit]));
                items.extend([
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
                // A detached `HEAD` has no branch to keep what was committed there.
                let after = match worktree.branch {
                    Some(_) => "Its branch stays.",
                    None => "It has no branch, so commits made there that no branch has are lost.",
                };
                // As last written, so an agent's edit while the question is up isn't lost.
                let lost: Vec<(String, Option<i64>)> = (worktree.uncommitted.iter())
                    .map(|f| (f.path.clone(), f.modified))
                    .collect();
                let (project, name) = (project.clone(), name.clone());
                let (deleted_project, deleted) = (project.clone(), name.clone());
                let changes = (!lost.is_empty()).then(|| self.changes_items(true, cx));
                let mut items = vec![
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
                        let (project, name, lost) =
                            (deleted_project.clone(), deleted.clone(), lost.clone());
                        let detail = match lost.len() {
                            0 => format!("Its folder is removed from disk. {after}"),
                            n => format!(
                                "Its folder is removed from disk, with {}, which are lost. \
                                 {after}",
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
                                    store.delete_worktree(&project, name, lost, cx)
                                })
                            },
                        )
                    })
                    .danger(),
                ];
                // As Main worktree's, after where it is and before what happens to it.
                items.splice(2..2, changes.into_iter().flatten());
                items
            }
            Selection::Stack(project, bottom) => {
                let key = (project.clone(), bottom.clone());
                let folded = self.folded_stacks.contains(&key);
                let (label, keys): (_, &dyn Action) = match folded {
                    true => ("Show branches", &Expand),
                    false => ("Hide branches", &Collapse),
                };
                let (fold_project, fold_bottom) = key.clone();
                let mut items = vec![keyed(label, keys, cx, move |this, window, cx| {
                    (this.navigator).update(cx, |nav, cx| {
                        nav.toggle_stack(&fold_project, &fold_bottom, !folded, window, cx)
                    })
                })];
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
                items.push(action("New branch on top…", &NewBranchOnTop).separated());
                // A linked worktree's lane stays applied while it has it checked out.
                if store.worktree_of(target).is_none() && workspace.managed {
                    items.push(action("Unapply stack", &UnapplyStack));
                }
                items
            }
            Selection::Mesh(..) => self.mesh_menu(target, cx),
            Selection::Branches(_) | Selection::History(_) => Vec::new(),
        }
    }
}
