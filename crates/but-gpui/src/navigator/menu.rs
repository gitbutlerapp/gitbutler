//! The navigator's right-click menus, and the commands of its rows that aren't edits.

use std::path::Path;

use bstr::BString;
use gpui::{Action, App, Context, Div, Stateful, Window, actions, prelude::*};

use super::{ClickOpened, Navigator};
use crate::app::{
    AppView, BuildAndRestartFromHere, Collapse, CopyBranchName, CopyCommitId, CopyErrorMessage,
    CopyPath, CopyPullRequestLink, CopySelection, CutSelection, Expand, Fetch, MarkAllRead,
    MoveSelection, NewBranch, NewBranchOnTop, NewWorktree, OpenPullRequest, Push, StartCommit,
    ToggleRead, UpdateFromTarget,
};
use crate::backend::rebuild::Rebuild;
use crate::backend::{Checkout, CommitId};
use crate::commands::Command;
use crate::edits::{
    AddEmptyCommitAbove, AddEmptyCommitBelow, CherryPick, CommitEdits, CopyChangeId, CopyMessage,
    DeleteBranch, DiscardChanges, DiscardCommit, MoveToNewBranch, NameBranch, NewBranchBelow,
    NewBranchHere, PullRemote, RenameBranch, Reword, RewrapMessage, SquashAll, SquashIntoSelected,
    SquashPicked, TakeRemote, UnapplyStack, Uncommit, Unstack,
};
use crate::overlays::MenuItem;
use crate::state::{ProjectId, Seeable, Selection, Store, WatchedOrder};
use crate::terminal::NewTerminal;
use crate::ui::plural;

actions!(
    navigator,
    [
        CloseTerminal,
        Watch,
        StopWatching,
        MoveUp,
        MoveDown,
        UnarchiveWorktree,
        Reveal,
        OpenProjectSettings,
        ArchiveWorktree,
        DeleteWorktree,
    ]
);

/// Where Move up or Move down takes the selected watched project or worktree: before the one
/// named, or after the others with `None`.
#[derive(Clone)]
enum Moved {
    Project(ProjectId, Option<ProjectId>),
    Worktree(ProjectId, BString, Option<BString>),
}

impl Moved {
    /// Moving the selection `up` or down, unless it's at that end, or doesn't move by hand.
    fn of(store: &Store, up: bool) -> Option<Self> {
        match store.selection.as_ref()? {
            // Only a watched card, in the manual order.
            Selection::Project(project) => {
                let manual = store.prefs.watched_order == WatchedOrder::Manual;
                if !manual || !store.project(project)?.expanded {
                    return None;
                }
                let watched = store.expanded_ids();
                let at = watched.iter().position(|id| id == project)?;
                let before = beside(&watched, at, up)?.cloned();
                Some(Moved::Project(project.clone(), before))
            }
            // Among those listed with it, the active or the inactive ones.
            Selection::Worktree(project, name) => {
                let ws = store.workspace(project)?;
                let worktree = ws.find_worktree(name.as_ref())?;
                let listed: Vec<&BString> = (store.worktrees_in_order(project, ws).into_iter())
                    .filter(|wt| wt.is_inactive() == worktree.is_inactive())
                    .filter(|wt| !store.hides_worktree(project, wt.name.as_ref()))
                    .map(|wt| &wt.name)
                    .collect();
                let at = listed.iter().position(|other| *other == name)?;
                let before = beside(&listed, at, up)?.map(|b| (*b).clone());
                Some(Moved::Worktree(project.clone(), name.clone(), before))
            }
            _ => None,
        }
    }

    fn apply(&self, store: &mut Store, cx: &mut Context<Store>) {
        match self {
            Moved::Project(id, before) => store.move_watched(id, before.as_ref(), cx),
            Moved::Worktree(project, name, before) => {
                let before = before.as_ref().map(|b| b.as_ref());
                store.move_worktree(project, name.as_ref(), before, cx)
            }
        }
    }
}

/// What the row at `at` of `listed` goes before to swap places with the one above or below
/// it: the one above, or the one after the one below, `None` for after the last. `None` at
/// the end it would move past.
fn beside<T>(listed: &[T], at: usize, up: bool) -> Option<Option<&T>> {
    match up {
        true => at.checked_sub(1).map(|above| listed.get(above)),
        false => (at + 1 < listed.len()).then(|| listed.get(at + 2)),
    }
}

impl AppView {
    /// Register what the selected row of the navigator offers beyond the edits: closing a
    /// terminal, watching a project and moving its card or a worktree, a checkout's folder,
    /// archiving or deleting a worktree, and from anywhere in a project, its archived worktrees.
    pub(crate) fn navigator_actions(&self, el: Stateful<Div>, cx: &Context<Self>) -> Stateful<Div> {
        let store = self.store.read(cx);
        let moves = |el: Stateful<Div>, up: bool| {
            el.when_some(Moved::of(store, up), |el, moved| {
                let run = move |this: &mut Self, cx: &mut Context<Self>| {
                    let moved = moved.clone();
                    (this.store).update(cx, |store, cx| moved.apply(store, cx))
                };
                match up {
                    true => el.on_action(cx.listener(move |this, _: &MoveUp, _, cx| run(this, cx))),
                    false => {
                        el.on_action(cx.listener(move |this, _: &MoveDown, _, cx| run(this, cx)))
                    }
                }
            })
        };
        let el = moves(moves(el, true), false);
        let unarchivable = (store.selection.as_ref())
            .map(|selection| selection.project().clone())
            .filter(|project| {
                (store.workspace(project)).is_some_and(|ws| !ws.archived_worktrees.is_empty())
            });
        let el = el.when_some(unarchivable, |el, project| {
            el.on_action(cx.listener(move |this, _: &UnarchiveWorktree, window, cx| {
                this.unarchive_worktree(&project, window, cx)
            }))
        });
        match store.selection.clone() {
            Some(Selection::Terminal(_, id)) => {
                el.on_action(cx.listener(move |this, _: &CloseTerminal, _, cx| {
                    (this.store).update(cx, |store, cx| store.close_terminal(id, cx))
                }))
            }
            Some(Selection::Project(project)) => {
                let expanded = store.project(&project).is_some_and(|p| p.expanded);
                let path = store.path(&project);
                let (watched, unwatched) = (project.clone(), project.clone());
                let settings = project.clone();
                el.on_action(
                    cx.listener(move |this, _: &OpenProjectSettings, window, cx| {
                        this.open_project_settings(settings.clone(), window, cx)
                    }),
                )
                .when(!expanded, |el| {
                    el.on_action(cx.listener(move |this, _: &Watch, _, cx| {
                        (this.store).update(cx, |store, cx| store.set_expanded(&watched, true, cx))
                    }))
                })
                // At once: Collapse (h) folds an open card before it stops watching it.
                .when(expanded, |el| {
                    el.on_action(cx.listener(move |this, _: &StopWatching, _, cx| {
                        (this.store)
                            .update(cx, |store, cx| store.set_expanded(&unwatched, false, cx))
                    }))
                })
                .when_some(path, |el, path| {
                    let main = (project.clone(), Checkout::Main);
                    checkout_actions(el, path, main, cx)
                })
            }
            Some(Selection::Worktree(project, name)) => {
                let worktree =
                    (store.workspace(&project)).and_then(|ws| ws.find_worktree(name.as_ref()));
                let Some(path) = worktree.map(|w| w.path.clone()) else {
                    return el;
                };
                let (archived, deleted) = ((project.clone(), name.clone()), name.clone());
                let linked = (project.clone(), Checkout::Linked(name.clone()));
                checkout_actions(el, path, linked, cx)
                    .on_action(cx.listener(move |this, _: &ArchiveWorktree, _, cx| {
                        let (project, name) = archived.clone();
                        (this.store).update(cx, |store, cx| {
                            store.set_worktree_archived(&project, name, true, cx)
                        })
                    }))
                    .on_action(cx.listener(move |this, _: &DeleteWorktree, window, cx| {
                        this.delete_worktree(&project, &deleted, window, cx)
                    }))
            }
            _ => el,
        }
    }

    /// Offer the worktrees archived of `project`, to unarchive one.
    fn unarchive_worktree(
        &mut self,
        project: &ProjectId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        let archived: Vec<BString> = (store.workspace(project)).map_or_else(Vec::new, |ws| {
            (ws.archived_worktrees.iter())
                .map(|(name, _)| name.clone())
                .collect()
        });
        let choices = (archived.into_iter())
            .map(|name| {
                let project = project.clone();
                crate::palette::Choice::new(name.to_string(), move |this, _, cx| {
                    (this.store).update(cx, |store, cx| {
                        store.set_worktree_archived(&project, name, false, cx)
                    })
                })
            })
            .collect();
        let place = store.place(&Selection::Project(project.clone()));
        self.open_picker(
            Some(place),
            "Unarchive a worktree",
            "Unarchive",
            choices,
            window,
            cx,
        )
    }

    /// Delete `project`'s linked worktree `name`, its folder and what's uncommitted in it,
    /// after asking.
    fn delete_worktree(
        &mut self,
        project: &ProjectId,
        name: &BString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        let Some(worktree) =
            (store.workspace(project)).and_then(|ws| ws.find_worktree(name.as_ref()))
        else {
            return;
        };
        // A detached `HEAD` has no branch to keep what was committed there.
        let after = match worktree.branch {
            Some(_) => "Its branch stays.",
            None => "It has no branch, so commits made there that no branch has are lost.",
        };
        // As last written, so an agent's edit while the question is up isn't lost.
        let lost: Vec<(String, Option<i64>, Option<u64>)> = (worktree.uncommitted.iter())
            .map(|f| (f.path.clone(), f.modified, f.size))
            .collect();
        let detail = match lost.len() {
            0 => format!("Its folder is removed from disk. {after}"),
            n => format!(
                "Its folder is removed from disk, and {} with it. {after}",
                plural(n, "uncommitted file")
            ),
        };
        let message = format!("Delete worktree {name}?");
        let (project, name) = (project.clone(), name.clone());
        let at = Some(Selection::Project(project.clone()));
        self.confirm_then(
            at,
            message,
            &detail,
            "Delete",
            window,
            cx,
            move |this, cx| {
                (this.store).update(cx, |store, cx| {
                    store.delete_worktree(&project, name, lost, cx)
                })
            },
        )
    }
}

/// Register opening `checkout` of a project, in `path`, in the editor or the file manager.
fn checkout_actions(
    el: Stateful<Div>,
    path: std::path::PathBuf,
    (project, checkout): (ProjectId, Checkout),
    cx: &Context<AppView>,
) -> Stateful<Div> {
    el.on_action(cx.listener(move |_, _: &Reveal, _, cx| cx.reveal_path(&path)))
        .on_action(
            cx.listener(move |this, _: &crate::open_in::OpenInEditor, window, cx| {
                let (main, position) = (checkout.clone(), window.mouse_position());
                this.open_in_editor(&project, main, None, None, position, cx)
            }),
        )
}

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
fn new_branch_items(cx: &App) -> Vec<MenuItem> {
    vec![
        MenuItem::command(&NewBranch, cx),
        MenuItem::command(&NewWorktree, cx),
    ]
}

/// Build and restart from checkout `dir`, the row right-clicked and so selected, in a client
/// `cargo run` started, when `dir` has the client to build.
fn build_item(dir: &Path, cx: &App) -> Option<MenuItem> {
    let builds = cx.try_global::<Rebuild>().is_some_and(|r| r.builds_in(dir));
    let item = MenuItem::command(&BuildAndRestartFromHere, cx);
    builds.then(|| item.separated())
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

    fn row_actions(&self, target: &Selection, cx: &App) -> Vec<MenuItem> {
        let store = self.store.read(cx);
        let command = |command: &'static dyn Command| MenuItem::command(command, cx);
        match target {
            Selection::Terminal(..) => vec![command(&Reword), command(&CloseTerminal)],
            // A remote's commit can be copied into a branch here, or its ID copied, and a
            // branch's marked read or unread.
            Selection::Upstream(project, ref_name, id) => {
                let commit = (ref_name.as_ref()).and_then(|ref_name| {
                    let commits = store.workspace(project)?.upstream(Some(ref_name));
                    commits.iter().find(|commit| commit.id == *id)
                });
                let marks = read_marks(store, project, commit, cx);
                let mut items = vec![
                    command(&CherryPick),
                    command(&CopyCommitId).keys(&CopySelection, cx),
                ];
                items.extend(marks);
                items
            }
            Selection::Project(project) => {
                let expanded = store.project(project).is_some_and(|p| p.expanded);
                let mut items = vec![command(&Watch).keys(&Expand, cx), command(&StopWatching)];
                if store.read_marks_of(target).is_some() {
                    items.push(MenuItem::action("Mark all as read", &MarkAllRead, cx));
                }
                items.extend([command(&MoveUp), command(&MoveDown)]);
                if expanded {
                    items.push(command(&MoveSelection));
                    let mut branch = new_branch_items(cx);
                    if let Some(first) = branch.first_mut() {
                        first.separated = true;
                    }
                    items.extend(branch);
                    items.push(command(&Fetch).separated());
                }
                // An unwatched project's too, once its page has read it. With Fetch, else apart
                // from Watch.
                let update = command(&UpdateFromTarget);
                items.push(if expanded { update } else { update.separated() });
                items.extend([
                    command(&UnarchiveWorktree),
                    command(&crate::open_in::OpenInEditor).separated(),
                    command(&crate::open_in::OpenInTerminal),
                    command(&Reveal),
                    // The note saying why the project can't be read may name a command to run.
                    command(&CopyErrorMessage),
                    command(&OpenProjectSettings).separated(),
                ]);
                items
            }
            Selection::Uncommitted(project) => {
                let mut items = new_branch_items(cx);
                items.push(command(&NewTerminal).separated());
                items.extend([command(&Fetch).separated(), command(&UpdateFromTarget)]);
                if store
                    .workspace(project)
                    .is_some_and(|ws| !ws.uncommitted.is_empty())
                {
                    items.extend(changes_items(cx));
                }
                items.extend(store.path(project).and_then(|path| build_item(&path, cx)));
                items
            }
            Selection::Branch(_, ref_name) => {
                let mut items = Vec::new();
                if store.branch_for(target).is_none() {
                    return items;
                }
                items.extend([
                    command(&Push),
                    command(&PullRemote),
                    command(&TakeRemote),
                    command(&OpenPullRequest),
                    command(&CopyPullRequestLink),
                    command(&crate::pr::CreatePullRequest),
                    command(&crate::pr::MergePullRequest),
                    command(&crate::pr::EnableAutoMerge),
                    command(&crate::pr::DisableAutoMerge),
                ]);
                if ref_name.is_none() {
                    items.push(command(&NameBranch).separated());
                    return items;
                }
                items.extend([
                    command(&NewBranchOnTop).separated(),
                    command(&NewBranchBelow),
                    command(&AddEmptyCommitAbove),
                    command(&SquashAll),
                    command(&CutSelection).separated(),
                    command(&MoveSelection),
                    command(&Unstack),
                    command(&crate::edits::MergeBranch),
                    command(&UnapplyStack),
                ]);
                items.extend([
                    command(&RenameBranch).separated(),
                    command(&CopyBranchName).keys(&CopySelection, cx),
                    command(&DeleteBranch).danger(),
                ]);
                items
            }
            Selection::Commit(project, id) if store.is_picked(project, *id) => {
                let ids = store.with_picks(project, *id);
                let picked: Vec<_> = (ids.iter())
                    .filter_map(|id| store.find_commit(project, *id).map(|(_, commit)| commit))
                    .collect();
                let mut items = vec![
                    command(&CutSelection),
                    command(&MoveSelection),
                    command(&CherryPick),
                    command(&SquashPicked).separated(),
                    command(&SquashIntoSelected),
                    command(&MoveToNewBranch).separated(),
                ];
                // A run picks commits without opening them, as clicking one does.
                items.extend(read_marks(store, project, picked, cx));
                items.extend([
                    command(&Uncommit).separated(),
                    command(&DiscardCommit).danger(),
                ]);
                items
            }
            Selection::Commit(project, id) => {
                let Some((branch, commit)) = store.find_commit(project, *id) else {
                    return Vec::new();
                };
                let mut items = vec![
                    command(&crate::detail::ResolveConflicts),
                    command(&CutSelection).separated(),
                    command(&MoveSelection),
                    command(&CherryPick),
                    command(&Reword).separated(),
                    command(&RewrapMessage),
                    command(&AddEmptyCommitAbove),
                    command(&AddEmptyCommitBelow),
                    command(&NewBranchHere),
                    command(&MoveToNewBranch).separated(),
                ];
                // ⌘C copies the change ID where there is one, else the commit ID.
                let (copy_id, change_id) = (command(&CopyCommitId), command(&CopyChangeId));
                items.extend(match CommitEdits::of(branch, commit).change_id {
                    true => [copy_id.separated(), change_id.keys(&CopySelection, cx)],
                    false => [copy_id.keys(&CopySelection, cx).separated(), change_id],
                });
                items.push(command(&CopyMessage));
                items.extend(read_marks(store, project, [commit], cx));
                items.extend([
                    command(&Uncommit).separated(),
                    command(&DiscardCommit).danger(),
                ]);
                items
            }
            Selection::Worktree(project, name) => {
                let Some(worktree) =
                    (store.workspace(project)).and_then(|ws| ws.find_worktree(name.as_ref()))
                else {
                    return Vec::new();
                };
                let mut items = vec![
                    command(&Reveal),
                    command(&CopyPath).keys(&CopySelection, cx),
                    command(&NewTerminal).separated(),
                    // Not Update: it rebases the workspace's stacks, not this worktree's.
                    command(&Fetch),
                ];
                // As Main worktree's, after Fetch and before what happens to it.
                if !worktree.uncommitted.is_empty() {
                    items.extend(changes_items(cx));
                }
                items.extend([
                    command(&MoveUp).separated(),
                    command(&MoveDown),
                    command(&ArchiveWorktree).separated(),
                    command(&DeleteWorktree).danger(),
                ]);
                items.extend(build_item(&worktree.path, cx));
                items
            }
            Selection::Mesh(..) => self.mesh_menu(target, cx),
            // The rows list a stack's branches, not the stack.
            Selection::Stack(..)
            | Selection::Branches(_)
            | Selection::History(_)
            | Selection::Graph(_)
            | Selection::Section(..) => Vec::new(),
        }
    }
}

/// Commit and discarding the changes of the selected checkout, which has some.
fn changes_items(cx: &App) -> [MenuItem; 2] {
    [
        MenuItem::command(&StartCommit, cx).separated(),
        MenuItem::command(&DiscardChanges, cx).danger(),
    ]
}
