//! Edits to the selected commit or branch beyond moving it: one step up or down, empty
//! commits, cherry-picks, squashing a branch or the picked commits, new branches below or
//! ending at a commit,
//! unstacking, unapplying, updating one stack and rewrapping a message. The navigator
//! offers each only where it applies, so menus, keys and the palette agree.

use gpui::{App, Context, Div, KeyBinding, Window, actions, prelude::*};

use crate::app::AppView;
use crate::backend::{self, CommitId, Destination, Selected, Workspace};
use crate::state::{Cut, Overlay, ProjectId, Selection};
use crate::transfer::{Transfer, remapped, short_ref};

actions!(
    edits,
    [
        MoveUp,
        MoveDown,
        AddEmptyCommitAbove,
        AddEmptyCommitBelow,
        CherryPick,
        SquashAll,
        SquashPicked,
        NewBranchBelow,
        NewBranchHere,
        Unstack,
        UnapplyStack,
        UpdateStack,
        RewrapMessage,
        CopyChangeId,
        CopyMessage,
        LandBranch,
        PullRemote,
        TakeRemote,
        MoveToNewBranch,
    ]
);

pub fn bind_keys(cx: &mut App) {
    let nav = Some("Navigator && !TextInput");
    cx.bind_keys([
        KeyBinding::new("alt-up", MoveUp, nav),
        KeyBinding::new("alt-down", MoveDown, nav),
    ]);
}

/// Body lines longer than this get rewrapped, as `git` and `but reword` expect.
const WRAP_AT: usize = 72;

/// Whether `message` has body lines too long to read in `git log`.
pub fn needs_rewrap(message: &str) -> bool {
    message
        .lines()
        .skip(1)
        .any(|line| line.chars().count() > WRAP_AT)
}

/// Where moving the commit `id` one step lands it: past its neighbour in the branch, or
/// across into the next branch of its stack.
fn step(workspace: &Workspace, id: CommitId, up: bool) -> Option<Destination> {
    let (branch, _) = workspace.find_commit(id)?;
    let (stack, at) = workspace.stack_of(&branch.ref_name)?;
    let ix = branch.commits.iter().position(|c| c.id == id)?;
    if up {
        match ix.checked_sub(1) {
            Some(above) => Some(Destination::AboveCommit(branch.commits[above].id)),
            None if at > 0 && !branch.ref_name.is_empty() => {
                Some(Destination::AboveBranch(branch.ref_name.clone()))
            }
            None => None,
        }
    } else {
        match branch.commits.get(ix + 1) {
            Some(below) => Some(Destination::BelowCommit(below.id)),
            None => stack
                .branches
                .get(at + 1)
                .filter(|below| !below.ref_name.is_empty())
                .map(|below| Destination::Branch(below.ref_name.clone())),
        }
    }
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

impl AppView {
    /// Register the edits that apply to `selection` on the element around the navigator.
    pub fn edit_actions(&self, el: Div, cx: &mut Context<Self>) -> Div {
        let store = self.store.read(cx);
        // Only where they'd do something, which is also what the palette offers.
        let picked = store.picked.len() > 1;
        let pullable = match &store.selection {
            Some(Selection::Branch(project, ref_name)) => {
                (store.workspace(project)).is_some_and(|ws| {
                    ws.find_branch(ref_name)
                        .is_some_and(|b| !b.incoming.is_empty())
                        && !ws.has_branches_above(ref_name)
                })
            }
            _ => false,
        };
        match store.selection {
            Some(Selection::Commit(..)) => el
                .on_action(cx.listener(|this, _: &MoveUp, _, cx| this.move_step(true, cx)))
                .on_action(cx.listener(|this, _: &MoveDown, _, cx| this.move_step(false, cx)))
                .on_action(cx.listener(|this, _: &AddEmptyCommitAbove, _, cx| {
                    this.add_empty_commit(true, cx)
                }))
                .on_action(cx.listener(|this, _: &AddEmptyCommitBelow, _, cx| {
                    this.add_empty_commit(false, cx)
                }))
                .on_action(cx.listener(Self::cherry_pick))
                .on_action(cx.listener(|this, _: &MoveToNewBranch, _, cx| {
                    if let Some((project, id)) = this.selected_commit(cx) {
                        this.store
                            .update(cx, |store, cx| store.move_to_new_branch(&project, id, cx));
                    }
                }))
                .when(picked, |el| el.on_action(cx.listener(Self::squash_picked)))
                .on_action(cx.listener(Self::new_branch_here))
                .on_action(cx.listener(Self::rewrap_message))
                .on_action(cx.listener(|this, _: &CopyChangeId, _, cx| this.copy_commit(true, cx)))
                .on_action(cx.listener(|this, _: &CopyMessage, _, cx| this.copy_commit(false, cx))),
            Some(Selection::Branch(..)) => el
                .on_action(cx.listener(|this, _: &AddEmptyCommitAbove, _, cx| {
                    this.add_empty_commit(true, cx)
                }))
                .on_action(cx.listener(Self::squash_all))
                .on_action(cx.listener(Self::new_branch_below))
                .on_action(cx.listener(Self::unstack))
                .on_action(cx.listener(Self::unapply_stack))
                .on_action(cx.listener(Self::update_stack))
                .on_action(cx.listener(Self::land_branch))
                .when(pullable, |el| {
                    el.on_action(cx.listener(|this, _: &PullRemote, window, cx| {
                        this.pull_remote(false, window, cx)
                    }))
                    .on_action(cx.listener(
                        |this, _: &TakeRemote, window, cx| this.pull_remote(true, window, cx),
                    ))
                }),
            _ => el,
        }
    }

    /// Bring the commits only the selected branch's remote has into it, the local ones
    /// rebased on top; with `theirs`, after asking, swap the branch for the remote's.
    pub fn pull_remote(&mut self, theirs: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some((project, ref_name)) = self.selected_branch_ref(cx) else {
            return;
        };
        let Some(workspace) = self.store.read(cx).workspace(&project) else {
            return;
        };
        let incoming = (workspace.find_branch(&ref_name)).map_or(0, |b| b.incoming.len());
        if incoming == 0 || workspace.has_branches_above(&ref_name) {
            return;
        }
        let name = short_ref(&ref_name).to_owned();
        let question = format!("Make {name} the remote's version?");
        let run = move |this: &mut AppView, cx: &mut Context<AppView>| {
            let (label, done) = if theirs {
                (
                    "Take the remote's version",
                    format!("{name} is the remote's now"),
                )
            } else {
                ("Pull from the remote", format!("Pulled into {name}"))
            };
            this.store.update(cx, |store, cx| {
                store.run(
                    &project,
                    label,
                    Overlay::default(),
                    move |path, _| backend::pull_remote(path, &ref_name, theirs),
                    move |store, (), cx| store.snack_undoable(done, cx),
                    cx,
                )
            });
        };
        if !theirs {
            return run(self, cx);
        }
        self.confirm_then(
            question,
            "Its commits the remote doesn't have are dropped. You can undo this with ⌘Z.",
            "Take the remote's",
            window,
            cx,
            run,
        );
    }

    fn selected_commit(&self, cx: &App) -> Option<(ProjectId, CommitId)> {
        match self.store.read(cx).selection.clone()? {
            Selection::Commit(project, id) => Some((project, id)),
            _ => None,
        }
    }

    fn selected_branch_ref(&self, cx: &App) -> Option<(ProjectId, String)> {
        match self.store.read(cx).selection.clone()? {
            Selection::Branch(project, ref_name) if !ref_name.is_empty() => {
                Some((project, ref_name))
            }
            _ => None,
        }
    }

    fn move_step(&mut self, up: bool, cx: &mut Context<Self>) {
        let Some((project, id)) = self.selected_commit(cx) else {
            return;
        };
        let to = self
            .store
            .read(cx)
            .workspace(&project)
            .and_then(|ws| step(ws, id, up));
        match to {
            Some(to) => {
                self.store.update(cx, |store, cx| {
                    Transfer::MoveCommit { id, to }.perform(&project, store, cx);
                });
            }
            None => self.store.update(cx, |store, cx| {
                let edge = if up { "top" } else { "bottom" };
                store.snack(format!("Already at the {edge} of its stack"), cx)
            }),
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
                move |path, remap| backend::insert_empty_commit(path, &remapped(to, remap)?),
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
        if let Some((project, id)) = self.selected_commit(cx) {
            self.cut(Cut::Pick(project, id), cx);
        }
    }

    /// Squash every commit of the selected branch into its oldest, keeping all messages.
    fn squash_all(&mut self, _: &SquashAll, _: &mut Window, cx: &mut Context<Self>) {
        let Some((project, ref_name)) = self.selected_branch_ref(cx) else {
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
        let rest = rest.iter().map(|c| c.id).collect();
        self.squash(project, rest, oldest.id, overlay, "Squash all".into(), cx);
    }

    /// Squash the picked commits into the oldest of them.
    fn squash_picked(&mut self, _: &SquashPicked, _: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let Some(Selection::Commit(project, _)) = store.selection.clone() else {
            return;
        };
        let Some(workspace) = store.workspace(&project) else {
            return;
        };
        // The workspace lists commits newest first.
        let mut picked: Vec<_> = workspace
            .commits()
            .filter(|c| store.picked.contains(&c.id))
            .collect();
        let Some(oldest) = picked.pop().map(|c| c.id) else {
            return;
        };
        if picked.is_empty() {
            return;
        }
        let overlay = Overlay::commits(picked.iter().map(|c| c.key.clone()));
        let label = format!("Squash {} commits", picked.len() + 1);
        let rest = picked.iter().map(|c| c.id).collect();
        self.squash(project, rest, oldest, overlay, label, cx);
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
        let Some((project, ref_name)) = self.selected_branch_ref(cx) else {
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
        self.prompt_branch(
            project,
            &title,
            "Create",
            "",
            window,
            cx,
            move |path, remap, name| {
                backend::create_branch_at_commit(path, remap.get(oldest)?, true, name)
            },
        );
    }

    /// Split the selected commit's branch: a new branch ending at it takes it and the
    /// commits below it.
    fn new_branch_here(&mut self, _: &NewBranchHere, window: &mut Window, cx: &mut Context<Self>) {
        let Some((project, id)) = self.selected_commit(cx) else {
            return;
        };
        let title = format!("New branch ending at {}", id.short());
        self.prompt_branch(
            project,
            &title,
            "Create",
            "",
            window,
            cx,
            move |path, remap, name| {
                backend::create_branch_at_commit(path, remap.get(id)?, false, name)
            },
        );
    }

    fn unstack(&mut self, _: &Unstack, _: &mut Window, cx: &mut Context<Self>) {
        let Some((project, ref_name)) = self.selected_branch_ref(cx) else {
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
        let Some((project, ref_name)) = self.selected_branch_ref(cx) else {
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

    fn update_stack(&mut self, _: &UpdateStack, _: &mut Window, cx: &mut Context<Self>) {
        if let Some((project, ref_name)) = self.selected_branch_ref(cx) {
            self.store.update(cx, |store, cx| {
                store.update_from_target(&project, Some(ref_name), cx)
            });
        }
    }

    /// Put the selected branch onto the target and push it there, after asking: the push
    /// can't be undone. The bottom branch of a stack lands alone; the top one with the
    /// branches below it.
    fn land_branch(&mut self, _: &LandBranch, window: &mut Window, cx: &mut Context<Self>) {
        let Some((project, ref_name)) = self.selected_branch_ref(cx) else {
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
        let Some((project, id)) = self.selected_commit(cx) else {
            return;
        };
        let Some((_, commit)) = self
            .store
            .read(cx)
            .workspace(&project)
            .and_then(|ws| ws.find_commit(id))
        else {
            return;
        };
        let message = backend::wrapped_message(&commit.message);
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
        let Some((project, id)) = self.selected_commit(cx) else {
            return;
        };
        let Some((_, commit)) = self
            .store
            .read(cx)
            .workspace(&project)
            .and_then(|ws| ws.find_commit(id))
        else {
            return;
        };
        let text = if change_id {
            commit.key.clone()
        } else {
            commit.message.clone()
        };
        self.copy_text(text, cx);
    }
}

impl AppView {
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
                Overlay::default(),
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
                        Overlay::default(),
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
