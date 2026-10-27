//! The actions on the selected commits, branches and changes, besides cut and paste: push,
//! rename, delete and discard, move, squash and split commits, new branches, land, pull,
//! unstack and unapply. `edit_actions` registers them around the navigator; Push is on the
//! window instead, so the branch page's button reaches it.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use gpui::{App, Context, Div, SharedString, Window, actions, prelude::*};

use crate::app::{
    AppView, DeleteSelection, FocusNavigator, NewBranchOnTop, Push, RenameSelection,
    UncommitSelection,
};
use crate::backend::{
    self, Branch, Checkout, Commit, CommitId, Destination, FullName, Selected, Undiscarded,
    Workspace, short_ref,
};
use crate::overlays::run_named;
use crate::state::{Cut, Label, Overlay, ProjectId, Remap, Rereads, Selection, Store, snack};

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
    (crate::backend::subject_and_body(message).1.lines()).any(|line| line.chars().count() > WRAP_AT)
}

/// What discarding changes is called, "…" when it asks first: in a linked worktree, as
/// [`AppView::discard`] does.
pub fn discard_label(what: &str, linked: bool) -> String {
    let more = if linked { "…" } else { "" };
    format!("Discard {what}{more}")
}

/// Throw away `changes` of `project`'s main checkout, though undo can't take it back as
/// GitButler couldn't record it.
fn discard_unrecorded(
    store: &mut Store,
    project: &ProjectId,
    changes: &[Selected],
    cx: &mut Context<Store>,
) {
    let overlay = Overlay {
        rereads: Rereads::Files,
        ..Overlay::files(changes)
    };
    let (changes, named) = (changes.to_vec(), project.clone());
    store.run(
        project,
        "Discard",
        overlay,
        move |path, _| backend::discard(path, None, &changes, true),
        move |store, outcome, cx| match outcome {
            None => store.snack(store.card_title(&named, "Changes discarded".into()), cx),
            Some(Undiscarded::Partway(why) | Undiscarded::Unrecorded(why)) => {
                store.error(store.card_title(&named, "Discard failed".into()), why, cx)
            }
        },
        cx,
    );
}

/// What a confirmation runs: operation `label` on `project`, as [`Store::run`] runs it.
fn then_run<T: Send + 'static>(
    project: ProjectId,
    label: impl Into<Label>,
    overlay: Overlay,
    op: impl FnOnce(&Path, &Remap) -> anyhow::Result<T> + Send + 'static,
    done: impl FnOnce(&mut Store, T, &mut Context<Store>) + 'static,
) -> impl FnOnce(&mut AppView, &mut Context<AppView>) + 'static {
    let label = label.into();
    move |this, cx| {
        (this.store).update(cx, |store, cx| {
            store.run(&project, label, overlay, op, done, cx)
        })
    }
}

/// What force pushing the target branch does, asked every time as others' work builds on it:
/// the commits only its remote has that it drops, the newest named, or else that it rewrites
/// commits already there.
fn target_force_detail(target: &backend::Target) -> String {
    let (what, them) = match target.commits_behind {
        0 => {
            return "Others' work builds on the target branch, and this rewrites commits already \
                    on its remote."
                .into();
        }
        1 => ("the commit".to_owned(), "it"),
        n => (format!("the {n} commits"), "them"),
    };
    let titles: Vec<String> = (target.incoming.iter().take(3))
        .map(|commit| format!("\"{}\"", commit.title))
        .collect();
    let named = match target.commits_behind.saturating_sub(titles.len()) {
        _ if titles.is_empty() => String::new(),
        0 => format!(": {}", titles.join(", ")),
        more => format!(": {}, and {more} more", titles.join(", ")),
    };
    format!(
        "Others' work builds on the target branch, and this drops {what} only {} has{named}. \
         Update first to keep {them}.",
        target.name
    )
}

/// Whether `ref_name` can land, and if so whether with the branches below it: the bottom
/// branch of a stack lands alone, the top one of a taller stack lands the whole stack. What
/// lands needs commits, and none conflicted, which the engine won't land.
fn land_scope(workspace: &Workspace, ref_name: &FullName) -> Option<bool> {
    let (stack, at) = workspace.stack_of(Some(ref_name))?;
    let whole = match (at, at + 1 == stack.branches.len()) {
        (_, true) => false,
        (0, false) => true,
        _ => return None,
    };
    let landing: Vec<_> = (stack.branches.get(at..)?.iter())
        .flat_map(|branch| &branch.commits)
        .collect();
    // Not when all of it is in the target already, as a branch merged there is.
    let new = landing
        .iter()
        .any(|c| c.state != crate::backend::CommitState::Integrated);
    (new && !landing.iter().any(|c| c.conflicted)).then_some(whole)
}

/// Which edits of a named branch have something to do, so its menu and the palette offer
/// the same ones.
#[derive(Default)]
pub struct BranchEdits {
    /// It has commits for a new branch below to keep.
    pub new_below: bool,
    /// It has more than one commit to squash into one, and none conflicted, which the
    /// engine won't squash.
    pub squash_all: bool,
    /// Other branches share its stack, so it can be unstacked.
    pub stacked: bool,
    /// It can land on the target, and whether with the branches below it.
    pub land: Option<bool>,
}

impl BranchEdits {
    pub fn of(workspace: &Workspace, ref_name: &FullName) -> Self {
        let Some((lane, branch)) = (workspace.lane_of(Some(ref_name)))
            .and_then(|(lane, at)| Some((lane, lane.branches.get(at)?)))
        else {
            return Self::default();
        };
        let commits = branch.commits.len();
        let conflicted = branch.commits.iter().any(|c| c.conflicted);
        // A linked worktree's lane takes new branches, but can't come apart yet.
        let stack = workspace.stack_of(Some(ref_name)).is_some();
        Self {
            new_below: commits > 0,
            squash_all: commits > 1 && !conflicted,
            stacked: stack && lane.branches.len() > 1,
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
            new_branch_here: branch.ref_name.is_some()
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
        let store = self.store.read(cx);
        let el = el
            .on_action(cx.listener(Self::delete_selection))
            .on_action(cx.listener(Self::rename_selection))
            .on_action(cx.listener(Self::new_branch_on_top))
            .on_action(cx.listener(Self::uncommit_selection));
        let picked = store.picked.len() > 1;
        match &store.selection {
            Some(Selection::Commit(project, id)) => {
                let managed = store.workspace(project).is_some_and(|ws| ws.managed);
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
                // A branch of its own sits beside the others, which only the workspace has.
                .when(managed, |el| {
                    el.on_action(cx.listener(|this, _: &MoveToNewBranch, _, cx| {
                        let Some((project, commit)) = this.selected_commit(cx) else {
                            return;
                        };
                        let id = commit.id;
                        this.store.update(cx, |store, cx| {
                            // With the commits picked with it, as dropping them on New branch does.
                            let ids = store.with_picks(&project, id);
                            store.move_to_new_branch(&project, ids, cx)
                        });
                    }))
                })
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
                let can_pull = (ref_name.as_ref()).is_some_and(|r| workspace.can_pull(r));
                let el = el.when(can_pull, |el| {
                    el.on_action(cx.listener(|this, _: &PullRemote, window, cx| {
                        this.pull_remote(false, window, cx)
                    }))
                    .on_action(cx.listener(
                        |this, _: &TakeRemote, window, cx| this.pull_remote(true, window, cx),
                    ))
                });
                // The rest work on a named branch only, as its menu says.
                let Some(ref_name) = ref_name.as_ref() else {
                    return el;
                };
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
                .when(
                    workspace.managed && workspace.stack_of(Some(ref_name)).is_some(),
                    |el| el.on_action(cx.listener(Self::unapply_stack)),
                )
                .when(edits.land.is_some(), |el| {
                    el.on_action(cx.listener(Self::land_branch))
                })
            }
            // Through its top branch: landing it lands the whole stack, and unapplying any
            // branch takes the whole stack out.
            Some(Selection::Stack(project, bottom)) => {
                let Some(workspace) = store.workspace(project) else {
                    return el;
                };
                let top = (workspace.lane(bottom.as_ref()))
                    .and_then(|lane| lane.branches.first())
                    .and_then(|top| top.ref_name.as_ref());
                let lands =
                    top.is_some_and(|top| BranchEdits::of(workspace, top).land == Some(true));
                el.when(lands, |el| el.on_action(cx.listener(Self::land_branch)))
                    .when(
                        workspace.managed
                            && top.is_some()
                            && workspace.stack_of(bottom.as_ref()).is_some(),
                        |el| el.on_action(cx.listener(Self::unapply_stack)),
                    )
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
                store.pull_remote(&project, ref_name, None, cx)
            });
        }
        let name = short_ref(&ref_name).into_owned();
        self.confirm_then(
            Some(Selection::Branch(project.clone(), Some(ref_name.clone()))),
            format!("Replace {name} with the remote's version?"),
            &crate::ui::keys_text(
                "This drops its commits the remote doesn't have. You can undo this with ⌘Z.",
            ),
            "Replace",
            window,
            cx,
            then_run(
                project,
                "Replace with the remote's version",
                Overlay::default(),
                move |path, _| backend::pull_remote(path, &ref_name, true),
                move |store, (), cx| {
                    store.snack_undoable(format!("{name} now matches the remote"), cx)
                },
            ),
        );
    }

    /// The selected commit, and the project it's in.
    fn selected_commit<'a>(&self, cx: &'a App) -> Option<(ProjectId, &'a Commit)> {
        let store = self.store.read(cx);
        let Some(Selection::Commit(project, id)) = &store.selection else {
            return None;
        };
        let (_, commit) = store.find_commit(project, *id)?;
        Some((project.clone(), commit))
    }

    /// The selected branch, or a selected stack's top one, when it has a name.
    fn selected_named_branch(&self, cx: &App) -> Option<(ProjectId, FullName)> {
        match self.store.read(cx).selection.as_ref()? {
            Selection::Branch(..) | Selection::Stack(..) => (self.branch_of_selection(cx))
                .and_then(|(project, ref_name)| Some((project, ref_name?))),
            _ => None,
        }
    }

    /// The selected branch, or the branch holding the selected commit, named or not. A
    /// stack acts through its top branch, which takes the ones below it along.
    fn branch_of_selection(&self, cx: &App) -> Option<(ProjectId, Option<FullName>)> {
        let selection = self.selection(cx)?;
        let store = self.store.read(cx);
        let branch = match &selection {
            Selection::Stack(project, bottom) => store
                .workspace(project)?
                .lane(bottom.as_ref())?
                .branches
                .first()?,
            _ => store.branch_for(&selection)?,
        };
        Some((selection.project().clone(), branch.ref_name.clone()))
    }

    /// Push the selected branch, or from the uncommitted changes the branch they commit to.
    pub fn push_selected(&mut self, _: &Push, window: &mut Window, cx: &mut Context<Self>) {
        let branch = match self.selection(cx) {
            // A message starts with a capital letter, which typed where the keys act, as after
            // clicking the diff, would push the branch it's for.
            Some(Selection::Uncommitted(_)) if self.detail.read(cx).composing() => {
                return snack(
                    &self.store,
                    "Not while writing a commit: push from its branch",
                    cx,
                );
            }
            Some(Selection::Uncommitted(project)) => {
                let target = self.detail.read(cx).effective_target(&project, cx);
                let Some(target) = target else {
                    return snack(&self.store, "Select a branch to push", cx);
                };
                Some((project, Some(target)))
            }
            _ => self.branch_of_selection(cx),
        };
        let Some((project, ref_name)) = branch else {
            return;
        };
        let Some(ws) = self.store.read(cx).workspace(&project) else {
            return;
        };
        if let Some(refused) = ws.push_blocked(ref_name.as_ref()) {
            return snack(&self.store, refused, cx);
        }
        let Some(ref_name) = ref_name else {
            return;
        };
        let drops = ws.push_drops(&ref_name);
        let detail = match ws.forced_target(&ref_name) {
            // Others' work builds on it, so forcing it always asks.
            Some(target) => target_force_detail(target),
            None => {
                let (what, them) = match drops.len() {
                    0 => {
                        return self.store.update(cx, |store, cx| {
                            store.push(&project, ref_name, true, Some(drops), cx)
                        });
                    }
                    1 => ("the commit".to_owned(), "it"),
                    n => (format!("the {n} commits"), "them"),
                };
                // A branch it takes along whose commits only its remote has can't be pulled
                // first when others are stacked on it or a linked worktree rests on it, nor
                // here when a linked worktree has it.
                let stuck = (ws.pushed_with(&ref_name)).find(|b| {
                    !b.incoming.is_empty() && !(b.ref_name.as_ref()).is_some_and(|r| ws.can_pull(r))
                });
                let linked = (stuck.and_then(|b| ws.lane_of(b.ref_name.as_ref())))
                    .is_some_and(|(lane, _)| lane.checkout.is_linked());
                match stuck {
                    None => {
                        format!("It drops {what} only the remote has. Pull first to keep {them}.")
                    }
                    Some(stuck) if linked => format!(
                        "It drops {what} only the remote has. To keep {name}'s, pull it in its \
                         worktree first.",
                        name = stuck.name
                    ),
                    Some(stuck) if (stuck.ref_name.as_ref()).is_some_and(|r| ws.rested_on(r)) => {
                        format!(
                            "It drops {what} only the remote has. {name}'s can't be pulled first \
                         while a worktree rests on it.",
                            name = stuck.name
                        )
                    }
                    Some(stuck) => format!(
                        "It drops {what} only the remote has. To keep {name}'s, unstack the \
                         branches on it and pull it first.",
                        name = stuck.name
                    ),
                }
            }
        };
        // Asked here, as the project's own protection is off unless turned on.
        self.confirm_then(
            Some(Selection::Branch(project.clone(), Some(ref_name.clone()))),
            format!("Force push {}?", short_ref(&ref_name)),
            &detail,
            "Force push",
            window,
            cx,
            move |this, cx| {
                this.store.update(cx, |store, cx| {
                    store.push(&project, ref_name, false, Some(drops), cx)
                })
            },
        );
    }

    fn delete_selection(
        &mut self,
        _: &DeleteSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.selection(cx) {
            // With the commits picked with it.
            Some(Selection::Commit(project, id)) => {
                let ids = self.store.read(cx).with_picks(&project, id);
                self.discard_commits(project, ids, window, cx)
            }
            // An unnamed branch has no ref to delete, as its menu says by offering none.
            Some(Selection::Branch(project, Some(ref_name))) => {
                self.remove_branch(project, ref_name, window, cx)
            }
            Some(Selection::Uncommitted(project)) => {
                let changes = self.detail.read(cx).selected_changes(cx);
                self.discard(project, Checkout::Main, changes, window, cx);
            }
            Some(Selection::Worktree(project, name)) => {
                let changes = self.detail.read(cx).selected_changes(cx);
                self.discard(project, Checkout::Linked(name), changes, window, cx);
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
            // An unnamed branch with no commits has nothing to name.
            Some(Selection::Branch(project, None))
                if (self.store.read(cx).workspace(&project))
                    .and_then(|ws| ws.find_branch(None))
                    .is_none_or(|b| b.commits.is_empty()) => {}
            Some(Selection::Branch(project, ref_name)) => {
                let store = self.store.read(cx);
                let branch = Selection::Branch(project.clone(), ref_name.clone());
                let undoable = store.undoable(&project, store.checkout_of(&branch));
                let workspace = store.workspace(&project);
                // An unnamed branch has no ref to rename, so it gets one at its newest commit.
                let unnamed_top = (workspace.filter(|_| ref_name.is_none()))
                    .and_then(|ws| Some(ws.find_branch(None)?.commits.first()?.id));
                // As the desktop app says: a rename doesn't reach the remote.
                let pushed = (workspace.and_then(|ws| ws.find_branch(ref_name.as_ref())))
                    .is_some_and(|branch| branch.push != backend::PushState::NeverPushed);
                let renaming = ref_name.clone();
                let (store, detail) = (self.store.clone(), self.detail.downgrade());
                let (at, selected, from) = (project.clone(), project.clone(), ref_name.clone());
                let submit = move |name: String, cx: &mut App| {
                    let renamed = from.clone();
                    let op =
                        move |path: &Path, remap: &Remap, name: &str| match (&renamed, unnamed_top)
                        {
                            (Some(renamed), _) => backend::rename_branch(path, renamed, name),
                            (None, Some(top)) => backend::name_branch(path, remap.get(top)?, name),
                            (None, None) => anyhow::bail!("The branch has no commit to name it at"),
                        };
                    let done = move |store: &mut Store, to: FullName, cx: &mut Context<Store>| {
                        (detail.update(cx, |detail, cx| {
                            detail.branch_renamed(&selected, from.as_ref(), &to, cx)
                        }))
                        .ok();
                        if !undoable {
                            store.keep_last_snapshot(cx);
                        }
                        if pushed {
                            let message = format!(
                                "Renamed to {}. The remote keeps the old name, and pushing \
                                 again makes a new remote branch, apart from any pull request \
                                 of the old one",
                                short_ref(&to)
                            );
                            store.snack_after(undoable, message, cx);
                        }
                        store.select(Some(Selection::Branch(selected, Some(to))), cx)
                    };
                    store.update(cx, |store, cx| {
                        run_named(store, &at, renaming, name, op, done, cx)
                    })
                };
                // The name is edited in its row there.
                if !self.navigator_open {
                    self.set_navigator(true, window, cx);
                }
                (self.navigator).update(cx, |navigator, cx| {
                    navigator.rename(project, ref_name, window, cx, submit)
                })
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
        let Some((project, below)) = self
            .branch_of_selection(cx)
            .and_then(|(project, ref_name)| Some((project, ref_name?)))
        else {
            return;
        };
        // A linked worktree's branch isn't one the composer here picks. On top of the one it
        // has checked out, the worktree checks the new one out and commits there, so the title
        // says so.
        let store = self.store.read(cx);
        let worktree = (store.selection.as_ref()).and_then(|s| store.worktree_for(s));
        let checked_out_in = (store.workspace(&project))
            .and_then(|ws| ws.checked_out_in(&below))
            .map(|w| w.name.clone());
        let here = worktree.is_none();
        let checkout = store.checkout_of(&Selection::Branch(project.clone(), Some(below.clone())));
        let op = move |path: &Path, _: &Remap| backend::create_dependent_branch(path, &below);
        self.new_branch_at(project, here, checked_out_in, checkout, window, cx, op);
    }

    /// Make a branch with a made-up name with `op` and select it, for r to rename, as the other
    /// apps do. One made on top (`on_top`) is what the next commit goes to; one splitting a
    /// stack isn't. `checked_out_in` names the linked worktree that checks it out, and
    /// `checkout` is the one whose lane it's made in, `None` for a stack of its own.
    #[expect(clippy::too_many_arguments)]
    pub(crate) fn new_branch_at(
        &mut self,
        project: ProjectId,
        on_top: bool,
        checked_out_in: Option<bstr::BString>,
        checkout: Option<Checkout>,
        window: &mut Window,
        cx: &mut Context<Self>,
        op: impl FnOnce(&Path, &Remap) -> anyhow::Result<FullName> + Send + 'static,
    ) {
        let (selected, detail) = (project.clone(), self.detail.downgrade());
        let undoable = self.store.read(cx).undoable(&project, checkout);
        let done = move |store: &mut Store, branch: FullName, cx: &mut Context<Store>| {
            if on_top {
                (detail.update(cx, |detail, cx| {
                    detail.branch_created(&selected, branch.clone(), cx)
                }))
                .ok();
            }
            let message = match checked_out_in {
                Some(worktree) => {
                    format!("Created {}, checked out in {worktree}", short_ref(&branch))
                }
                None => format!("Created {}", short_ref(&branch)),
            };
            store.select(Some(Selection::Branch(selected, Some(branch))), cx);
            store.snack_after(undoable, message, cx)
        };
        // The navigator, where it's selected, takes the keys, as r to rename it.
        self.focus_navigator(&FocusNavigator, window, cx);
        self.store.update(cx, |store, cx| {
            store.run(&project, "Create branch", Overlay::default(), op, done, cx)
        });
    }

    /// Take the selection out and keep it: a commit out of its branch, into uncommitted
    /// changes, and a branch, with its stack, out of the workspace.
    fn uncommit_selection(
        &mut self,
        _: &UncommitSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.selection(cx) {
            Some(Selection::Commit(project, id)) => self.store.update(cx, |store, cx| {
                let ids = store.with_picks(&project, id).to_vec();
                store.uncommit(&project, ids, cx)
            }),
            Some(Selection::Branch(..)) => self.unapply_stack(&UnapplyStack, window, cx),
            _ => {}
        }
    }

    fn remove_branch(
        &mut self,
        project: ProjectId,
        ref_name: FullName,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = short_ref(&ref_name).into_owned();
        let store = self.store.read(cx);
        let Some((lane, _)) =
            (store.workspace(&project)).and_then(|ws| ws.lane_of(Some(&ref_name)))
        else {
            return;
        };
        // The lane as the deletes already asked for leave it, which this one follows.
        let lane = store.after_deletes(&project, &lane.branches);
        let Some(at) = lane
            .iter()
            .position(|branch| branch.ref_name.as_ref() == Some(&ref_name))
        else {
            return;
        };
        // A linked worktree keeps the branch it has checked out; the ones stacked under it
        // can go, their commits joining it.
        let selection = Selection::Branch(project.clone(), Some(ref_name.clone()));
        let linked = store.in_linked(&selection);
        if at == 0 && linked {
            let message = format!("{name} is checked out in a worktree, which keeps it");
            return snack(&self.store, message, cx);
        }
        // Only the ref goes. Its commits stay in the stack while another named branch can hold
        // them: the one above, or below when there's none above. Otherwise they go with it.
        let named = |b: &&Branch| b.ref_name.is_some();
        let commits = lane.get(at).map_or(&[][..], |b| &b.commits);
        let lane_commits: usize = lane.iter().map(|b| b.commits.len()).sum();
        let with_commits = lane.iter().filter(named).count() < 2 && lane_commits > 0;
        // Checked out on its own, it can't be unapplied, only switched away from.
        let managed = store.workspace(&project).is_some_and(|ws| ws.managed);
        let target = (store.workspace(&project).filter(|ws| !ws.managed))
            .and_then(|ws| ws.target_branch.as_ref())
            .map(short_ref)
            .filter(|target| *target != name);
        // So does one checked out without GitButler's workspace. Unnamed commits in its stack,
        // or a linked worktree's, aren't its own to take along.
        if with_commits && (!managed || linked || commits.len() < lane_commits) {
            let message = match target {
                Some(target) => format!(
                    "To delete {name}, check out another branch, as with `git switch {target}`, \
                     then delete it from Branches"
                ),
                None => format!(
                    "To delete {name} and its commits, unapply it, then delete it from Branches"
                ),
            };
            return snack(&self.store, message, cx);
        }
        // Dropping them would rewrite a linked worktree resting on them too, which undo can't
        // bring back.
        let ids: Vec<CommitId> = commits.iter().map(|c| c.id).collect();
        let moved = store.moved_by(&project, &ids);
        if with_commits && let Some(worktree) = moved.iter().find_map(Checkout::linked) {
            let message = format!("The {worktree} worktree rests on {name}, which keeps it");
            return snack(&self.store, message, cx);
        }
        // The oplog doesn't keep a linked worktree's branches, so it can't bring one back.
        let undoable = store.undoable(&project, store.checkout_of(&selection));
        let above = at.checked_sub(1).and_then(|ix| lane.get(ix));
        let joins = (above.filter(named).or(lane.get(at + 1)))
            .filter(|_| !with_commits && !commits.is_empty())
            .map(|next| next.name.clone());
        let (detail, deleted) = match joins {
            _ if with_commits => match commits.len() {
                1 => (
                    "Its commit goes too. This can't be undone.".to_owned(),
                    format!("Deleted {name} and its commit"),
                ),
                n => (
                    format!("Its {n} commits go too. This can't be undone."),
                    format!("Deleted {name} and its {n} commits"),
                ),
            },
            Some(next) => (
                format!("Its commits stay, as part of {next}. This can't be undone."),
                format!("Deleted {name}, its commits joining {next}"),
            ),
            None => (
                "This can't be undone.".to_owned(),
                format!("Deleted {name}"),
            ),
        };
        let overlay = match with_commits {
            true => Overlay {
                branches: HashSet::from([ref_name.clone()]),
                ..Overlay::rewriting_main()
            },
            false => Overlay {
                deleted_branches: HashSet::from([ref_name.clone()]),
                ..Overlay::default()
            },
        };
        // What the user saw on top, so new commits made meanwhile aren't dropped unseen.
        let tip = (commits.first()).filter(|_| with_commits).map(|c| c.id);
        let run = then_run(
            project,
            "Delete branch",
            overlay,
            move |path, _| match tip {
                Some(tip) => backend::delete_branch_and_commits(path, &ref_name, tip),
                None => backend::remove_branch(path, &ref_name, None),
            },
            move |store, (), cx| store.snack_after(undoable, deleted, cx),
        );
        if undoable {
            return run(self, cx);
        }
        let message = format!("Delete branch {name}?");
        self.confirm_then(Some(selection), message, &detail, "Delete", window, cx, run);
    }

    /// Drop commits `ids` of `project` and what they changed, the newest listed first.
    fn discard_commits(
        &mut self,
        project: ProjectId,
        ids: Arc<[CommitId]>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(&newest) = ids.first() else {
            return;
        };
        let store = self.store.read(cx);
        let moved = store.moved_by(&project, &ids);
        let in_worktree = moved.iter().any(Checkout::is_linked);
        let undoable = store.undoable(&project, moved);
        let overlay = Overlay::commits(ids.iter().filter_map(|id| store.key_of(&project, *id)));
        let (message, done) = match ids.len() {
            1 => (
                format!("Discard commit {}?", newest.short()),
                "Commit discarded".to_owned(),
            ),
            n => (
                format!("Discard {n} commits?"),
                format!("{n} commits discarded"),
            ),
        };
        let detail = match in_worktree {
            false => "Their changes are dropped. This can't be undone.",
            true => "Their changes are dropped from the worktree too. This can't be undone.",
        };
        let at = Selection::Commit(project.clone(), newest);
        let run = then_run(
            project,
            "Discard commit",
            overlay,
            move |path, remap| {
                let now = match in_worktree {
                    false => remap.all(&ids)?,
                    true => (ids.iter())
                        .map(|id| remap.exactly(*id))
                        .collect::<anyhow::Result<_>>()?,
                };
                backend::discard_commits(path, &now)
            },
            move |store, (), cx| store.snack_after(undoable, done, cx),
        );
        if undoable {
            return run(self, cx);
        }
        self.confirm_then(Some(at), message, detail, "Discard", window, cx, run);
    }

    /// Throw away uncommitted changes of `worktree`.
    /// The main checkout's go at once, with Undo on the snackbar as in Zed's review. A linked
    /// worktree's always ask, as the oplog can't bring them back, and so do the main
    /// checkout's while it can't record them.
    pub fn discard(
        &mut self,
        project: ProjectId,
        worktree: Checkout,
        mut changes: Vec<Selected>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Not what a pending discard or commit already takes, as after a quick second press.
        let store = self.store.read(cx);
        changes.retain(|change| {
            let path = &change.file.path;
            match &change.hunks {
                None => !store.hides_file(&project, &worktree, path),
                Some(hunks) => {
                    !(hunks.iter()).all(|h| store.hides_hunk(&project, &worktree, path, *h))
                }
            }
        });
        let lasting = match &worktree {
            Checkout::Linked(_) => Some("Changes discarded in a linked worktree can't be undone."),
            Checkout::Main if (store.workspace(&project)).is_some_and(|ws| ws.unrecorded) => Some(
                "With no commits yet, no branch checked out or no target, discarded changes can't \
                 be undone.",
            ),
            Checkout::Main => None,
        };
        // A submodule's own work goes with it too, which the oplog doesn't record.
        let why = (changes.iter())
            .find(|change| change.file.submodule().is_some())
            .map(|sub| match sub.file.kind {
                backend::ChangeKind::Added => format!(
                    "{} is a repository of its own. Discarding deletes it and everything in it, \
                     and undo can't bring it back.",
                    sub.file.path
                ),
                _ => format!(
                    "Discarding resets {} to the commit recorded for it. Uncommitted changes in it \
                     are deleted, and undo can't bring them back.",
                    sub.file.path
                ),
            })
            .or(lasting.map(str::to_owned));
        let ask = match (why, &changes[..]) {
            (_, []) => return,
            (None, _) => None,
            (Some(why), picked) => {
                let what = match picked {
                    [one] if one.hunks.is_some() => format!("part of {}", one.file.path),
                    [one] => format!("changes to {}", one.file.path),
                    _ => format!("changes to {} files", picked.len()),
                };
                Some((format!("Discard {what}?"), why))
            }
        };
        let undoable = lasting.is_none();
        let at = Selection::uncommitted_in(project.clone(), worktree.clone());
        let overlay = Overlay {
            rereads: Rereads::Files,
            files_in: worktree.clone(),
            ..Overlay::files(&changes)
        };
        // Asked already when undo can't take it back; else, when GitButler can't record it
        // after all, nothing goes, and the card asks.
        let (asked, again) = (project.clone(), changes.clone());
        let run = then_run(
            project,
            "Discard",
            overlay,
            move |path, _| backend::discard(path, worktree.linked(), &changes, !undoable),
            move |store, outcome, cx| match outcome {
                None => store.snack_after(undoable, "Changes discarded", cx),
                Some(Undiscarded::Unrecorded(why)) => store.error_with_retry(
                    store.card_title(&asked, "Changes weren't discarded".into()),
                    format!("GitButler couldn't record them to undo: {why}"),
                    "Discard anyway",
                    move |store, cx| discard_unrecorded(store, &asked, &again, cx),
                    cx,
                ),
                Some(Undiscarded::Partway(why)) => store.error(
                    store.card_title(&asked, "Discard failed".into()),
                    format!("{why}\nUndo puts back anything it threw away before that."),
                    cx,
                ),
            },
        );
        match ask {
            None => run(self, cx),
            Some((message, detail)) => {
                self.confirm_then(Some(at), message, &detail, "Discard", window, cx, run)
            }
        }
    }

    /// An empty commit above or below the selected commit, or on top of the selected branch. The
    /// selection stays where it was, following its commit if the insert rewrites it.
    fn add_empty_commit(&mut self, above: bool, cx: &mut Context<Self>) {
        let at = match self.store.read(cx).selection.clone() {
            Some(Selection::Commit(project, id)) if above => {
                (project, Destination::AboveCommit(id))
            }
            Some(Selection::Commit(project, id)) => (project, Destination::BelowCommit(id)),
            Some(Selection::Branch(project, Some(ref_name))) => {
                (project, Destination::Branch(ref_name))
            }
            _ => return,
        };
        let (project, to) = at;
        self.store.update(cx, |store, cx| {
            // It shows where it goes right away, dimmed until it's there.
            let at = store.new_at(&project, &to);
            let overlay = Overlay {
                new_commit: at.map(|at| (at, SharedString::default())),
                ..Overlay::default()
            };
            store.run(
                &project,
                "Add empty commit",
                overlay,
                move |path, remap| backend::insert_empty_commit(path, &remap.destination(to)?),
                move |store, _, cx| store.snack_undoable("Empty commit added", cx),
                cx,
            )
        });
    }

    /// Pick up the selected commit, and those picked with it, to put copies of them where ⌘V
    /// lands.
    fn cherry_pick(&mut self, _: &CherryPick, _: &mut Window, cx: &mut Context<Self>) {
        if let Some((project, commit)) = self.selected_commit(cx) {
            let ids = self.store.read(cx).with_picks(&project, commit.id);
            self.cut(Cut::CherryPick(project, ids), cx);
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
            .and_then(|ws| ws.find_branch(Some(&ref_name)))
        else {
            return;
        };
        let Some((oldest, rest)) = branch.commits.split_last() else {
            return;
        };
        if rest.is_empty() {
            return;
        }
        let (rest, oldest) = (rest.iter().map(|c| c.id).collect(), oldest.id);
        self.store
            .update(cx, |store, cx| store.squash(&project, rest, oldest, cx));
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
        let picked: Vec<_> = (workspace.commits())
            .map(|c| c.id)
            .filter(|id| store.picked.contains(id))
            .collect();
        let oldest = picked.last().copied();
        let Some(into) = into.filter(|id| store.picked.contains(id)).or(oldest) else {
            return;
        };
        let rest: Vec<_> = picked.into_iter().filter(|id| *id != into).collect();
        if rest.is_empty() {
            return;
        }
        // A conflicted one's change would be dropped, or the engine refuses.
        if (workspace.commits()).any(|c| c.conflicted && store.picked.contains(&c.id)) {
            return snack(
                &self.store,
                "Resolve the conflicted commits first, then squash",
                cx,
            );
        }
        self.store.update(cx, |store, cx| {
            store.picked.clear();
            store.squash(&project, rest, into, cx)
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
            .and_then(|ws| ws.find_branch(Some(&ref_name)))
            .and_then(|branch| branch.commits.last())
            .map(|commit| commit.id);
        let Some(oldest) = oldest else {
            return;
        };
        let branch = Selection::Branch(project.clone(), Some(ref_name));
        let checkout = self.store.read(cx).checkout_of(&branch);
        let op = move |path: &Path, remap: &Remap| {
            backend::create_branch_at_commit(path, remap.get(oldest)?, true)
        };
        self.new_branch_at(project, false, None, checkout, window, cx, op);
    }

    /// Split the selected commit's branch: a new branch ending at it takes it and the
    /// commits below it.
    fn new_branch_here(&mut self, _: &NewBranchHere, window: &mut Window, cx: &mut Context<Self>) {
        let Some((project, commit)) = self.selected_commit(cx) else {
            return;
        };
        let id = commit.id;
        let checkout = (self.store.read(cx)).checkout_of(&Selection::Commit(project.clone(), id));
        let op = move |path: &Path, remap: &Remap| {
            backend::create_branch_at_commit(path, remap.get(id)?, false)
        };
        self.new_branch_at(project, false, None, checkout, window, cx, op);
    }

    fn unstack(&mut self, _: &Unstack, _: &mut Window, cx: &mut Context<Self>) {
        let Some((project, ref_name)) = self.selected_named_branch(cx) else {
            return;
        };
        self.store
            .update(cx, |store, cx| store.unstack(&project, ref_name, cx));
    }

    fn unapply_stack(&mut self, _: &UnapplyStack, _: &mut Window, cx: &mut Context<Self>) {
        let Some((project, ref_name)) = self.selected_named_branch(cx) else {
            return;
        };
        // The whole stack goes, at once, and the snackbar counts the other branches in it.
        // An unnamed one has no ref to tell it from another stack's.
        let Some(stack): Option<HashSet<FullName>> = (self.store.read(cx).workspace(&project))
            .and_then(|ws| ws.stack_of(Some(&ref_name)))
            .map(|(stack, _)| {
                (stack.branches.iter())
                    .filter_map(|b| b.ref_name.clone())
                    .collect()
            })
        else {
            return;
        };
        let name = match stack.len().max(1) {
            1 => short_ref(&ref_name).into_owned(),
            2 => format!("{} and the other branch in its stack", short_ref(&ref_name)),
            n => format!(
                "{} and the {} other branches in its stack",
                short_ref(&ref_name),
                n - 1
            ),
        };
        self.store.update(cx, |store, cx| {
            store.run(
                &project,
                "Unapply",
                Overlay {
                    branches: stack,
                    ..Overlay::rewriting_main()
                },
                move |path, _| backend::unapply_stack(path, &ref_name),
                move |store, (), cx| store.snack_undoable(format!("Unapplied {name}"), cx),
                cx,
            )
        });
    }

    /// Put the selected branch onto the target and push it there, after asking: the push
    /// can't be undone. A target with no upstream just moves, which can be. The bottom branch
    /// of a stack lands alone; the top one with the branches below it.
    fn land_branch(&mut self, _: &LandBranch, window: &mut Window, cx: &mut Context<Self>) {
        let Some((project, ref_name)) = self.selected_named_branch(cx) else {
            return;
        };
        let store = self.store.read(cx);
        let Some(workspace) = store.workspace(&project) else {
            return;
        };
        let Some((target, behind, local)) =
            (workspace.target.as_ref()).map(|t| (t.name.clone(), t.needs_update, t.local))
        else {
            return;
        };
        let Some(whole_stack) = land_scope(workspace, &ref_name) else {
            return;
        };
        // What lands leaves at once: the whole stack, each branch of it listed so a selection
        // on any, or on its caption, moves off; or the bottom branch and its commits.
        let overlay = match whole_stack {
            true => Overlay {
                branches: (workspace.stack_of(Some(&ref_name)).into_iter())
                    .flat_map(|(stack, _)| stack.branches.iter().filter_map(|b| b.ref_name.clone()))
                    .collect(),
                ..Overlay::default()
            },
            false => Overlay {
                commits: (workspace.find_branch(Some(&ref_name)).into_iter())
                    .flat_map(|branch| branch.commits.iter().map(|c| c.key.clone()))
                    .collect(),
                deleted_branches: HashSet::from([ref_name.clone()]),
                ..Overlay::default()
            },
        };
        // The branch and those below it, where its ref is even when it has no commits of its
        // own.
        let landing: Vec<&Branch> = (workspace.stack_of(Some(&ref_name)).into_iter())
            .flat_map(|(stack, at)| stack.branches.get(at..).unwrap_or_default())
            .collect();
        // What the question shows, to land only that: the newest commit among them.
        let Some(tip) = landing.iter().flat_map(|branch| &branch.commits).next() else {
            return;
        };
        let tip = tip.id;
        // Commits only the landing branches' remotes have, as a teammate's not pulled yet.
        let remote_only: usize = landing.iter().map(|branch| branch.incoming.len()).sum();
        let (name, landed) = (short_ref(&ref_name).into_owned(), project.clone());
        let (what, whose, remotes) = match whole_stack {
            true => (
                match landing.len() {
                    2 => format!("{name} and the branch below it"),
                    _ => format!("{name} and the branches below it"),
                },
                "Their",
                "their remotes",
            ),
            false => (name.clone(), "Its", "its remote"),
        };
        let left_behind = match remote_only {
            0 => String::new(),
            1 => format!(" 1 commit on {remotes} isn't pulled yet, so it won't land."),
            n => format!(" {n} commits on {remotes} aren't pulled yet, so they won't land."),
        };
        // Landing brings the target's new commits into the workspace, as Update does.
        let update = match behind {
            true => format!(
                " Your other branches and changes then update onto {target}, which can leave \
                 conflicts."
            ),
            false => String::new(),
        };
        self.confirm_then(
            Some(Selection::Branch(project.clone(), Some(ref_name.clone()))),
            format!("Land {what} on {target}?"),
            &match local {
                true => format!("{whose} commits go straight onto {target}.{left_behind}{update}"),
                false => format!(
                    "{whose} commits go straight onto {target} and get pushed, skipping \
                     review. A push can't be undone.{left_behind}{update}"
                ),
            },
            "Land",
            window,
            cx,
            then_run(
                project,
                Label::new("Land", format!("Land {name}")),
                overlay,
                {
                    let ref_name = ref_name.clone();
                    move |path, _| backend::land(path, &ref_name, tip, whole_stack)
                },
                move |store, left: backend::Landed, cx| {
                    // Undo would bring the branch back here while the target keeps it.
                    store.drop_last_step();
                    // What the update after it left conflicted, as Update says.
                    let mut said = vec![format!("Landed on {target}{}", store.in_project(&landed))];
                    said.extend(crate::state::conflicted_parts(&left.conflicted));
                    if left.marked {
                        said.push("uncommitted changes got conflict markers".to_owned());
                    }
                    if left.update_needed {
                        said.push("the other branches need an Update".to_owned());
                    }
                    store.snack(said.join(", "), cx)
                },
            ),
        );
    }

    fn rewrap_message(&mut self, _: &RewrapMessage, _: &mut Window, cx: &mut Context<Self>) {
        let Some((project, commit)) = self.selected_commit(cx) else {
            return;
        };
        let (id, message) = (commit.id, backend::wrapped_message(&commit.message));
        let overlay = Overlay {
            reworded: Some((commit.key.clone(), message.clone())),
            ..Overlay::default()
        };
        self.store.update(cx, |store, cx| {
            store.run(
                &project,
                "Rewrap message",
                overlay,
                move |path, remap| backend::reword(path, remap.get(id)?, &message),
                move |store, rewritten, cx| {
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
        if text.trim().is_empty() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
            return snack(&self.store, "Copied an empty message", cx);
        }
        self.copy_text(text, cx);
    }

    /// Move `files` out of `commit` into a new commit right above it, and select that one.
    pub fn split_off(
        &mut self,
        project: ProjectId,
        commit: CommitId,
        files: Arc<[Selected]>,
        cx: &mut Context<Self>,
    ) {
        let selected = project.clone();
        self.store.update(cx, |store, cx| {
            store.run(
                &project,
                "Split commit",
                Overlay::commit_files(store.key_of(&project, commit), files.iter()),
                move |path, remap| {
                    backend::split_off(path, remap.taken_from(commit, &files)?, &files)
                },
                move |store, (_, new), cx| {
                    store.select(Some(Selection::Commit(selected, new)), cx);
                    store.snack_undoable("Split into a new commit", cx)
                },
                cx,
            )
        });
    }

    /// Take `files` out of `commit` as if it had never made them, asking first when that
    /// can't be undone.
    pub fn discard_from_commit(
        &mut self,
        project: ProjectId,
        commit: CommitId,
        files: Arc<[Selected]>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let what = match &*files {
            [file] if file.hunks.is_some() => format!("part of {}", file.file.path),
            [file] => file.file.path.clone(),
            files => format!("{} files", files.len()),
        };
        let store = self.store.read(cx);
        // The oplog can't bring back a linked worktree's files.
        let moved = store.moved_by(&project, &[commit]);
        let in_worktree = moved.iter().any(Checkout::is_linked);
        let undoable = store.undoable(&project, moved);
        let overlay = Overlay::commit_files(store.key_of(&project, commit), files.iter());
        let at = Selection::Commit(project.clone(), commit);
        let run = then_run(
            project,
            "Discard from commit",
            overlay,
            move |path, remap| {
                let from = match in_worktree {
                    false => remap.taken_from(commit, &files)?,
                    true => remap.exactly(commit)?,
                };
                backend::discard_from_commit(path, from, &files)
            },
            move |store, rewritten, cx| {
                store.follow(&rewritten, cx);
                store.snack_after(undoable, "Discarded from the commit", cx)
            },
        );
        if undoable {
            return run(self, cx);
        }
        self.confirm_then(
            Some(at),
            format!("Discard {what} from commit {}?", commit.short()),
            "The commit, and the ones after it, lose the change. This can't be undone.",
            "Discard",
            window,
            cx,
            run,
        );
    }
}
