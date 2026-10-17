//! The actions on the selected commits, branches and changes, besides cut and paste: push,
//! rename, delete and discard, move, squash and split commits, new branches, land, pull,
//! unstack and unapply. `edit_actions` registers them around the navigator; Push is on the
//! window instead, so the branch page's button reaches it.

use std::path::Path;
use std::sync::Arc;

use gpui::{App, Context, Div, Window, actions, prelude::*};

use crate::app::{
    AppView, DeleteSelection, NewBranchOnTop, Push, RenameSelection, UncommitSelection,
};
use crate::backend::{self, Branch, Commit, CommitId, Destination, Selected, Workspace, short_ref};
use crate::state::{Cut, Overlay, ProjectId, Remap, Selection, Store};

actions!(
    edits,
    [
        AddEmptyCommitAbove,
        AddEmptyCommitBelow,
        CherryPick,
        SquashAll,
        SquashPicked,
        NewBranchBelow,
        NewBranchHere,
        Unstack,
        UnapplyStack,
        RewrapMessage,
        CopyChangeId,
        CopyMessage,
        LandBranch,
        PullRemote,
        TakeRemote,
        MoveToNewBranch,
    ]
);

/// Body lines longer than this get rewrapped, as `git` and `but reword` expect.
const WRAP_AT: usize = 72;

/// Whether `message` has body lines too long to read in `git log`.
fn needs_rewrap(message: &str) -> bool {
    message
        .lines()
        .skip(1)
        .any(|line| line.chars().count() > WRAP_AT)
}

/// Whether `ref_name` can land, and if so whether with the branches below it: the bottom
/// branch of a stack lands alone, the top one of a taller stack lands the whole stack.
pub fn land_scope(workspace: &Workspace, ref_name: &str) -> Option<bool> {
    let (stack, at) = workspace.stack_of(ref_name)?;
    let branch = &stack.branches[at];
    if branch.commits.is_empty() || branch.commits.iter().any(|c| c.conflicted) {
        return None;
    }
    let bottom = at + 1 == stack.branches.len();
    match (at, bottom) {
        (_, true) => Some(false),
        (0, false) => Some(true),
        _ => None,
    }
}

/// Which edits of a named branch have something to do, so its menu and the palette offer
/// the same ones.
#[derive(Default)]
pub struct BranchEdits {
    /// It has commits for a new branch below to keep.
    pub new_below: bool,
    /// It has more than one commit to squash into one.
    pub squash_all: bool,
    /// Other branches share its stack, so it can be unstacked.
    pub stacked: bool,
    /// It can land on the target, and whether with the branches below it.
    pub land: Option<bool>,
}

impl BranchEdits {
    pub fn of(workspace: &Workspace, ref_name: &str) -> Self {
        let Some((stack, at)) = workspace.stack_of(ref_name) else {
            return Self::default();
        };
        let commits = stack.branches[at].commits.len();
        Self {
            new_below: commits > 0,
            squash_all: commits > 1,
            stacked: stack.branches.len() > 1,
            land: land_scope(workspace, ref_name).filter(|_| workspace.target.is_some()),
        }
    }
}

/// Which edits of a commit have something to do, so its menu and the palette offer the
/// same ones.
#[derive(Default)]
pub struct CommitEdits {
    /// It's below the top of a named branch, so a new branch can end at it.
    pub new_branch_here: bool,
    /// Its message has body lines to rewrap.
    pub rewrap: bool,
    /// It has a change ID to copy.
    pub change_id: bool,
}

impl CommitEdits {
    pub fn of(branch: &Branch, commit: &Commit) -> Self {
        Self {
            new_branch_here: !branch.ref_name.is_empty()
                && branch.commits.first().map(|c| c.id) != Some(commit.id),
            rewrap: needs_rewrap(&commit.message),
            change_id: commit.has_change_id(),
        }
    }
}

impl AppView {
    /// Register the edits on the element around the navigator and the detail. Delete, rename, new branch
    /// on top and uncommit are always there; the rest only where the selection lets them do
    /// something, which is also what the palette offers.
    pub fn edit_actions(&self, el: Div, cx: &mut Context<Self>) -> Div {
        let el = el
            .on_action(cx.listener(Self::delete_selection))
            .on_action(cx.listener(Self::rename_selection))
            .on_action(cx.listener(Self::new_branch_on_top))
            .on_action(cx.listener(Self::uncommit_selection));
        let store = self.store.read(cx);
        let picked = store.picked.len() > 1;
        match &store.selection {
            Some(Selection::Commit(project, id)) => {
                let edits = (store.find_commit(project, *id))
                    .map_or_else(CommitEdits::default, |(branch, commit)| {
                        CommitEdits::of(branch, commit)
                    });
                el.on_action(cx.listener(|this, _: &AddEmptyCommitAbove, _, cx| {
                    this.add_empty_commit(true, cx)
                }))
                .on_action(cx.listener(|this, _: &AddEmptyCommitBelow, _, cx| {
                    this.add_empty_commit(false, cx)
                }))
                .on_action(cx.listener(Self::cherry_pick))
                .on_action(cx.listener(|this, _: &MoveToNewBranch, _, cx| {
                    let Some((project, commit)) = this.selected_commit(cx) else {
                        return;
                    };
                    let id = commit.id;
                    this.store.update(cx, |store, cx| {
                        // A branch of its own sits beside the others, which only the
                        // workspace has.
                        if store.workspace(&project).is_some_and(|ws| !ws.managed) {
                            return store.snack(
                                "Moving a commit to a new branch needs the GitButler \
                                 workspace. Go back to it from the project's page.",
                                cx,
                            );
                        }
                        store.move_to_new_branch(&project, id, cx)
                    });
                }))
                .when(picked, |el| el.on_action(cx.listener(Self::squash_picked)))
                .when(edits.new_branch_here, |el| {
                    el.on_action(cx.listener(Self::new_branch_here))
                })
                .when(edits.rewrap, |el| {
                    el.on_action(cx.listener(Self::rewrap_message))
                })
                .when(edits.change_id, |el| {
                    el.on_action(
                        cx.listener(|this, _: &CopyChangeId, _, cx| this.copy_commit(true, cx)),
                    )
                })
                .on_action(cx.listener(|this, _: &CopyMessage, _, cx| this.copy_commit(false, cx)))
            }
            Some(Selection::Branch(project, ref_name)) => {
                let Some(workspace) = store.workspace(project) else {
                    return el;
                };
                let el = el.when(workspace.can_pull(ref_name), |el| {
                    el.on_action(cx.listener(|this, _: &PullRemote, window, cx| {
                        this.pull_remote(false, window, cx)
                    }))
                    .on_action(cx.listener(
                        |this, _: &TakeRemote, window, cx| this.pull_remote(true, window, cx),
                    ))
                });
                // The rest work on a named branch only, as its menu says.
                if ref_name.is_empty() {
                    return el;
                }
                let edits = BranchEdits::of(workspace, ref_name);
                el.on_action(cx.listener(|this, _: &AddEmptyCommitAbove, _, cx| {
                    this.add_empty_commit(true, cx)
                }))
                .when(edits.squash_all, |el| {
                    el.on_action(cx.listener(Self::squash_all))
                })
                .when(edits.new_below, |el| {
                    el.on_action(cx.listener(Self::new_branch_below))
                })
                .when(edits.stacked, |el| el.on_action(cx.listener(Self::unstack)))
                // Outside the workspace there are no stacks to take out of it.
                .when(workspace.managed, |el| {
                    el.on_action(cx.listener(Self::unapply_stack))
                })
                .when(edits.land.is_some(), |el| {
                    el.on_action(cx.listener(Self::land_branch))
                })
            }
            _ => el,
        }
    }

    /// Bring the commits only the selected branch's remote has into it, the local ones
    /// rebased on top; with `theirs`, after asking, swap the branch for the remote's.
    pub fn pull_remote(&mut self, theirs: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some((project, ref_name)) = self.selected_named_branch(cx) else {
            return;
        };
        if !(self.store.read(cx).workspace(&project)).is_some_and(|ws| ws.can_pull(&ref_name)) {
            return;
        }
        if !theirs {
            return self.store.update(cx, |store, cx| {
                store.pull_remote(&project, ref_name, false, cx)
            });
        }
        let name = short_ref(&ref_name).to_owned();
        self.confirm_then(
            format!("Make {name} the remote's version?"),
            "Its commits the remote doesn't have are dropped. You can undo this with ⌘Z.",
            "Take the remote's",
            window,
            cx,
            move |this, cx| {
                this.store.update(cx, |store, cx| {
                    store.run(
                        &project,
                        "Take the remote's version",
                        Overlay::default(),
                        move |path, _| backend::pull_remote(path, &ref_name, true),
                        move |store, (), cx| {
                            store.snack_undoable(format!("{name} is the remote's now"), cx)
                        },
                        cx,
                    )
                })
            },
        );
    }

    /// The selected commit, and the project it's in.
    pub(crate) fn selected_commit<'a>(&self, cx: &'a App) -> Option<(ProjectId, &'a Commit)> {
        let store = self.store.read(cx);
        let Some(Selection::Commit(project, id)) = &store.selection else {
            return None;
        };
        let (_, commit) = store.find_commit(project, *id)?;
        Some((project.clone(), commit))
    }

    /// The selected commit, for an action on one commit; with several picked, which looks
    /// like several selected, it says so instead.
    pub(crate) fn single_commit(&self, cx: &mut Context<Self>) -> Option<(ProjectId, CommitId)> {
        if self.store.read(cx).picked.len() > 1 {
            self.store.update(cx, |store, cx| {
                store.snack("That works on one commit: click one to pick just it", cx)
            });
            return None;
        }
        let (project, commit) = self.selected_commit(cx)?;
        Some((project, commit.id))
    }

    /// The selected branch, when it has a name.
    fn selected_named_branch(&self, cx: &App) -> Option<(ProjectId, String)> {
        match self.store.read(cx).selection.clone()? {
            Selection::Branch(project, ref_name) if !ref_name.is_empty() => {
                Some((project, ref_name))
            }
            _ => None,
        }
    }

    /// The selected branch, or the branch holding the selected commit, named or not.
    fn branch_of_selection(&self, cx: &App) -> Option<(ProjectId, String)> {
        let selection = self.selection(cx)?;
        let branch = self.store.read(cx).branch_for(&selection)?;
        Some((selection.project().clone(), branch.ref_name.clone()))
    }

    /// Push the selected branch, or from the uncommitted changes the branch they commit to.
    pub fn push_selected(&mut self, _: &Push, _: &mut Window, cx: &mut Context<Self>) {
        let branch = match self.selection(cx) {
            Some(Selection::Uncommitted(project)) => {
                let target = self.detail.read(cx).effective_target(&project, cx);
                let Some(target) = target else {
                    return self
                        .store
                        .update(cx, |store, cx| store.snack("Select a branch to push", cx));
                };
                Some((project, target))
            }
            _ => self.branch_of_selection(cx),
        };
        if let Some((project, ref_name)) = branch {
            self.store
                .update(cx, |store, cx| store.push(&project, ref_name, true, cx));
        }
    }

    fn delete_selection(
        &mut self,
        _: &DeleteSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.selection(cx) {
            Some(Selection::Commit(..)) => {
                if let Some((project, id)) = self.single_commit(cx) {
                    self.discard_commit(project, id, window, cx)
                }
            }
            Some(Selection::Branch(project, ref_name)) => {
                self.remove_branch(project, ref_name, window, cx)
            }
            Some(Selection::Uncommitted(project)) => {
                let changes = self.detail.read(cx).selected_changes(cx);
                self.discard(project, None, changes, window, cx);
            }
            _ => {}
        }
    }

    fn rename_selection(
        &mut self,
        _: &RenameSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.selection(cx) {
            Some(Selection::Branch(project, ref_name)) => {
                let current = short_ref(&ref_name).to_owned();
                // An unnamed branch has no ref to rename, so it gets one at its newest commit.
                let unnamed_top = (self.store.read(cx).workspace(&project))
                    .filter(|_| ref_name.is_empty())
                    .and_then(|ws| Some(ws.find_branch("")?.commits.first()?.id));
                let title = match unnamed_top {
                    Some(_) => "Name branch",
                    None => "Rename branch",
                };
                let (selected, detail, from) =
                    (project.clone(), self.detail.downgrade(), ref_name.clone());
                self.prompt_branch(
                    project,
                    title,
                    &current,
                    window,
                    cx,
                    move |path, remap, name| match unnamed_top {
                        Some(top) => backend::name_branch(path, remap.get(top)?, name),
                        None => backend::rename_branch(path, &ref_name, name),
                    },
                    move |store, to, cx| {
                        (detail.update(cx, |detail, cx| detail.branch_renamed(&from, &to, cx)))
                            .ok();
                        store.select(Some(Selection::Branch(selected, to)), cx)
                    },
                )
            }
            Some(Selection::Commit(..)) => self
                .detail
                .update(cx, |detail, cx| detail.start_reword(window, cx)),
            _ => {}
        }
    }

    fn new_branch_on_top(
        &mut self,
        _: &NewBranchOnTop,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some((project, below)) = self
            .branch_of_selection(cx)
            .filter(|(_, ref_name)| !ref_name.is_empty())
        {
            self.prompt_new_branch(
                project,
                "New branch on top",
                window,
                cx,
                move |path, _, name| backend::create_dependent_branch(path, &below, name),
            );
        }
    }

    /// Ask for a branch name and make the branch with `op`, then select it. With the composer
    /// open, it's what the next commit goes to.
    pub(crate) fn prompt_new_branch(
        &mut self,
        project: ProjectId,
        title: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
        op: impl FnOnce(&Path, &Remap, &str) -> anyhow::Result<String> + Send + 'static,
    ) {
        let (selected, detail) = (project.clone(), self.detail.downgrade());
        let done = move |store: &mut Store, branch: String, cx: &mut Context<Store>| {
            (detail.update(cx, |detail, cx| {
                detail.branch_created(&selected, branch.clone(), cx)
            }))
            .ok();
            store.select(Some(Selection::Branch(selected, branch)), cx)
        };
        self.prompt_branch(project, title, "", window, cx, op, done);
    }

    fn uncommit_selection(
        &mut self,
        _: &UncommitSelection,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(Selection::Commit(project, id)) = self.selection(cx) {
            self.store.update(cx, |store, cx| {
                let ids = match store.picked.is_empty() {
                    true => vec![id],
                    false => std::mem::take(&mut store.picked),
                };
                store.uncommit(&project, ids, cx)
            });
        }
    }

    fn remove_branch(
        &mut self,
        project: ProjectId,
        ref_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = short_ref(&ref_name).to_owned();
        let Some((stack, at)) =
            (self.store.read(cx).workspace(&project)).and_then(|ws| ws.stack_of(&ref_name))
        else {
            return;
        };
        // Only the ref goes. Its commits stay in the stack, which needs another named branch
        // to hold them: the one above, or below when there's none above.
        let named = |b: &&Branch| !b.ref_name.is_empty();
        if stack.branches.iter().filter(named).count() < 2
            && stack.branches.iter().any(|b| !b.commits.is_empty())
        {
            let message = format!(
                "To delete {name} and its commits, unapply it, then delete it from Branches"
            );
            return self.store.update(cx, |store, cx| store.snack(message, cx));
        }
        let above = at.checked_sub(1).map(|ix| &stack.branches[ix]);
        let detail = match above.filter(named).or(stack.branches.get(at + 1)) {
            Some(next) if !stack.branches[at].commits.is_empty() => format!(
                "Its commits stay, as part of {}. You can undo this with ⌘Z.",
                next.name
            ),
            _ => "You can undo this with ⌘Z.".to_owned(),
        };
        self.confirm_then(
            format!("Delete branch {name}?"),
            &detail,
            "Delete",
            window,
            cx,
            move |this, cx| {
                let project = project.clone();
                this.store.update(cx, |store, cx| {
                    store.run(
                        &project,
                        "Delete branch",
                        Overlay::default(),
                        move |path, _| backend::remove_branch(path, &ref_name),
                        |store, (), cx| store.snack_undoable("Branch deleted", cx),
                        cx,
                    )
                });
            },
        );
    }

    fn discard_commit(
        &mut self,
        project: ProjectId,
        id: CommitId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.confirm_then(
            format!("Discard commit {}?", id.short()),
            "Its changes are dropped. You can undo this with ⌘Z.",
            "Discard",
            window,
            cx,
            move |this, cx| {
                let project = project.clone();
                this.store.update(cx, |store, cx| {
                    store.run(
                        &project,
                        "Discard commit",
                        Overlay::commits(store.key_of(&project, id)),
                        move |path, remap| backend::discard_commit(path, remap.get(id)?),
                        |store, (), cx| store.snack_undoable("Commit discarded", cx),
                        cx,
                    )
                });
            },
        );
    }

    /// Throw away uncommitted changes of the main checkout, or of the linked `worktree`.
    /// One file or hunk of the main checkout goes at once, with Undo on the snackbar as in
    /// Zed's review; several files ask first. A linked worktree's always ask, as the oplog
    /// can't bring them back.
    pub fn discard(
        &mut self,
        project: ProjectId,
        worktree: Option<Arc<str>>,
        mut changes: Vec<Selected>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Not what a pending discard or commit already takes, as after a quick second press.
        // Those only take the main checkout's changes.
        if worktree.is_none() {
            let store = self.store.read(cx);
            changes.retain(|change| {
                let path = &change.file.path;
                match &change.hunks {
                    None => !store.hides_file(&project, path),
                    Some(hunks) => !hunks.iter().all(|h| store.hides_hunk(&project, path, *h)),
                }
            });
        }
        let ask = match (&worktree, &changes[..]) {
            (_, []) => return,
            (None, [_]) => None,
            (None, _) => Some((
                format!("Discard changes to {} files?", changes.len()),
                "You can undo this with ⌘Z.",
            )),
            (Some(name), picked) => {
                let what = match picked {
                    [one] if one.hunks.is_some() => format!("a hunk of {}", one.file.path),
                    [one] => format!("changes to {}", one.file.path),
                    _ => format!("changes to {} files", picked.len()),
                };
                Some((
                    format!("Discard {what} in {name}?"),
                    "Changes discarded in a linked worktree can't be undone.",
                ))
            }
        };
        let run = move |this: &mut AppView, changes: Vec<Selected>, cx: &mut Context<AppView>| {
            // An overlay hides the main checkout's files, and only a full reload reads a
            // linked worktree's again.
            let overlay = match worktree {
                None => Overlay {
                    files_only: true,
                    ..Overlay::files(&changes)
                },
                Some(_) => Overlay::default(),
            };
            let undoable = worktree.is_none();
            this.store.update(cx, |store, cx| {
                store.run(
                    &project,
                    "Discard",
                    overlay,
                    move |path, _| backend::discard(path, worktree.as_deref(), &changes),
                    move |store, (), cx| match undoable {
                        true => store.snack_undoable("Changes discarded", cx),
                        false => store.snack("Changes discarded", cx),
                    },
                    cx,
                )
            });
        };
        match ask {
            None => run(self, changes, cx),
            Some((message, detail)) => {
                self.confirm_then(message, detail, "Discard", window, cx, move |this, cx| {
                    run(this, changes, cx)
                })
            }
        }
    }

    /// An empty commit above or below the selected commit, or on top of the selected branch.
    fn add_empty_commit(&mut self, above: bool, cx: &mut Context<Self>) {
        let at = match self.store.read(cx).selection.clone() {
            Some(Selection::Commit(project, id)) if above => {
                (project, Destination::AboveCommit(id))
            }
            Some(Selection::Commit(project, id)) => (project, Destination::BelowCommit(id)),
            Some(Selection::Branch(project, ref_name)) if !ref_name.is_empty() => {
                (project, Destination::Branch(ref_name))
            }
            _ => return,
        };
        let (project, to) = at;
        let selected = project.clone();
        self.store.update(cx, |store, cx| {
            store.run(
                &project,
                "Add empty commit",
                Overlay::default(),
                move |path, remap| backend::insert_empty_commit(path, &remap.destination(to)?),
                move |store, id, cx| {
                    store.select(Some(Selection::Commit(selected, id)), cx);
                    store.snack_undoable("Empty commit added", cx)
                },
                cx,
            )
        });
    }

    /// Pick up the selected commit to put a copy of it where ⌘V lands.
    fn cherry_pick(&mut self, _: &CherryPick, _: &mut Window, cx: &mut Context<Self>) {
        if let Some((project, id)) = self.single_commit(cx) {
            self.cut(Cut::CherryPick(project, id), cx);
        }
    }

    /// Squash every commit of the selected branch into its oldest, keeping all messages.
    fn squash_all(&mut self, _: &SquashAll, _: &mut Window, cx: &mut Context<Self>) {
        let Some((project, ref_name)) = self.selected_named_branch(cx) else {
            return;
        };
        let store = self.store.read(cx);
        let Some(branch) = store
            .workspace(&project)
            .and_then(|ws| ws.find_branch(&ref_name))
        else {
            return;
        };
        let Some((oldest, rest)) = branch.commits.split_last() else {
            return;
        };
        if rest.is_empty() {
            return;
        }
        let overlay = Overlay::commits(rest.iter().map(|c| c.key.clone()));
        // Oldest first, so their messages follow in the order they were written.
        let rest = rest.iter().rev().map(|c| c.id).collect();
        self.squash(project, rest, oldest.id, overlay, "Squash all".into(), cx);
    }

    /// Squash the picked commits into the oldest of them.
    fn squash_picked(&mut self, _: &SquashPicked, _: &mut Window, cx: &mut Context<Self>) {
        self.squash_picks(None, cx)
    }

    /// Squash the picked commits into one, in the place of `into` when it's one of them, or
    /// of the oldest. Their messages are kept, one after another.
    pub fn squash_picks(&mut self, into: Option<CommitId>, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let Some(Selection::Commit(project, _)) = store.selection.clone() else {
            return;
        };
        let Some(workspace) = store.workspace(&project) else {
            return;
        };
        // The workspace lists commits newest first.
        let picked: Vec<_> = workspace
            .commits()
            .filter(|c| store.picked.contains(&c.id))
            .collect();
        let oldest = picked.last().map(|c| c.id);
        let Some(into) = into.filter(|id| store.picked.contains(id)).or(oldest) else {
            return;
        };
        let rest: Vec<_> = picked.into_iter().filter(|c| c.id != into).collect();
        if rest.is_empty() {
            return;
        }
        let overlay = Overlay::commits(rest.iter().map(|c| c.key.clone()));
        let label = format!("Squash {} commits", rest.len() + 1);
        // Oldest first, so their messages follow `into`'s in the order they were written.
        let rest = rest.iter().rev().map(|c| c.id).collect();
        self.store.update(cx, |store, _| store.picked.clear());
        self.squash(project, rest, into, overlay, label, cx);
    }

    fn squash(
        &mut self,
        project: ProjectId,
        rest: Vec<CommitId>,
        into: CommitId,
        overlay: Overlay,
        label: String,
        cx: &mut Context<Self>,
    ) {
        self.store.update(cx, |store, cx| {
            store.run(
                &project,
                label,
                overlay,
                move |path, remap| {
                    let rest = rest
                        .iter()
                        .map(|id| remap.get(*id))
                        .collect::<anyhow::Result<Vec<_>>>()?;
                    backend::squash(path, &rest, remap.get(into)?)
                },
                |store, rewritten, cx| {
                    store.follow(&rewritten, cx);
                    store.snack_undoable("Squashed into one commit", cx)
                },
                cx,
            )
        });
    }

    /// A new branch under the selected one, which keeps its commits.
    fn new_branch_below(
        &mut self,
        _: &NewBranchBelow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((project, ref_name)) = self.selected_named_branch(cx) else {
            return;
        };
        let oldest = self
            .store
            .read(cx)
            .workspace(&project)
            .and_then(|ws| ws.find_branch(&ref_name))
            .and_then(|branch| branch.commits.last())
            .map(|commit| commit.id);
        let Some(oldest) = oldest else {
            return;
        };
        let title = format!("New branch below {}", short_ref(&ref_name));
        self.prompt_new_branch(project, &title, window, cx, move |path, remap, name| {
            backend::create_branch_at_commit(path, remap.get(oldest)?, true, name)
        });
    }

    /// Split the selected commit's branch: a new branch ending at it takes it and the
    /// commits below it.
    fn new_branch_here(&mut self, _: &NewBranchHere, window: &mut Window, cx: &mut Context<Self>) {
        let Some((project, commit)) = self.selected_commit(cx) else {
            return;
        };
        let id = commit.id;
        let title = format!("New branch ending at {}", id.short());
        self.prompt_new_branch(project, &title, window, cx, move |path, remap, name| {
            backend::create_branch_at_commit(path, remap.get(id)?, false, name)
        });
    }

    fn unstack(&mut self, _: &Unstack, _: &mut Window, cx: &mut Context<Self>) {
        let Some((project, ref_name)) = self.selected_named_branch(cx) else {
            return;
        };
        self.store.update(cx, |store, cx| {
            store.run(
                &project,
                "Unstack",
                Overlay::default(),
                move |path, _| backend::tear_off_branch(path, &ref_name),
                |store, (), cx| store.snack_undoable("Moved to its own stack", cx),
                cx,
            )
        });
    }

    fn unapply_stack(&mut self, _: &UnapplyStack, _: &mut Window, cx: &mut Context<Self>) {
        let Some((project, ref_name)) = self.selected_named_branch(cx) else {
            return;
        };
        let name = short_ref(&ref_name).to_owned();
        self.store.update(cx, |store, cx| {
            store.run(
                &project,
                "Unapply",
                Overlay::default(),
                move |path, _| backend::unapply_stack(path, &ref_name),
                move |store, (), cx| store.snack_undoable(format!("Unapplied {name}"), cx),
                cx,
            )
        });
    }

    /// Put the selected branch onto the target and push it there, after asking: the push
    /// can't be undone. The bottom branch of a stack lands alone; the top one with the
    /// branches below it.
    fn land_branch(&mut self, _: &LandBranch, window: &mut Window, cx: &mut Context<Self>) {
        let Some((project, ref_name)) = self.selected_named_branch(cx) else {
            return;
        };
        let store = self.store.read(cx);
        let Some(workspace) = store.workspace(&project) else {
            return;
        };
        let Some(target) = workspace.target.as_ref().map(|t| t.name.clone()) else {
            return;
        };
        let Some(whole_stack) = land_scope(workspace, &ref_name) else {
            return;
        };
        let name = short_ref(&ref_name).to_owned();
        let what = if whole_stack {
            format!("{name} and the branches below it")
        } else {
            name.clone()
        };
        self.confirm_then(
            format!("Land {what} on {target}?"),
            &format!(
                "Their commits go straight onto {target} and get pushed, skipping review. \
                 A push can't be undone."
            ),
            "Land",
            window,
            cx,
            move |this, cx| {
                let label = format!("Land {name}");
                this.store.update(cx, |store, cx| {
                    store.run(
                        &project,
                        label,
                        Overlay::default(),
                        move |path, _| backend::land(path, &name, whole_stack),
                        move |store, update_needed, cx| {
                            let mut message = format!("Landed on {target}");
                            if update_needed {
                                message.push_str(". Update to bring the other branches along");
                            }
                            store.snack(message, cx)
                        },
                        cx,
                    )
                });
            },
        );
    }

    fn rewrap_message(&mut self, _: &RewrapMessage, _: &mut Window, cx: &mut Context<Self>) {
        let Some((project, commit)) = self.selected_commit(cx) else {
            return;
        };
        let (id, message) = (commit.id, backend::wrapped_message(&commit.message));
        self.store.update(cx, |store, cx| {
            store.run(
                &project,
                "Rewrap message",
                Overlay::default(),
                move |path, remap| backend::reword(path, remap.get(id)?, &message),
                |store, rewritten, cx| {
                    store.follow(&rewritten, cx);
                    store.snack_undoable("Message rewrapped", cx)
                },
                cx,
            )
        });
    }

    /// Copy the selected commit's change ID, or its whole message.
    fn copy_commit(&mut self, change_id: bool, cx: &mut Context<Self>) {
        let Some((_, commit)) = self.selected_commit(cx) else {
            return;
        };
        let text = if change_id {
            commit.key.clone()
        } else {
            commit.message.clone()
        };
        self.copy_text(text, cx);
    }

    /// Move `files` out of `commit` into a new commit right above it, and select that one.
    pub fn split_off(
        &mut self,
        project: ProjectId,
        commit: CommitId,
        files: std::sync::Arc<[Selected]>,
        cx: &mut Context<Self>,
    ) {
        let selected = project.clone();
        self.store.update(cx, |store, cx| {
            store.run(
                &project,
                "Split commit",
                Overlay::commit_files(store.key_of(&project, commit), files.iter()),
                move |path, remap| backend::split_off(path, remap.get(commit)?, &files),
                move |store, (rewritten, new), cx| {
                    store.follow(&rewritten, cx);
                    store.select(Some(Selection::Commit(selected, new)), cx);
                    store.snack_undoable("Split into a new commit", cx)
                },
                cx,
            )
        });
    }

    /// Take `files` out of `commit` as if it had never made them, after asking.
    pub fn discard_from_commit(
        &mut self,
        project: ProjectId,
        commit: CommitId,
        files: std::sync::Arc<[Selected]>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let what = match &*files {
            [file] if file.hunks.is_some() => "this hunk".to_owned(),
            [file] => file.file.path.clone(),
            files => format!("{} files", files.len()),
        };
        self.confirm_then(
            format!("Discard {what} from commit {}?", commit.short()),
            "The commit, and the ones after it, lose the change. You can undo this with ⌘Z.",
            "Discard",
            window,
            cx,
            move |this, cx| {
                this.store.update(cx, |store, cx| {
                    store.run(
                        &project,
                        "Discard from commit",
                        Overlay::commit_files(store.key_of(&project, commit), files.iter()),
                        move |path, remap| {
                            backend::discard_from_commit(path, remap.get(commit)?, &files)
                        },
                        |store, rewritten, cx| {
                            store.follow(&rewritten, cx);
                            store.snack_undoable("Discarded from the commit", cx)
                        },
                        cx,
                    )
                });
            },
        );
    }
}
