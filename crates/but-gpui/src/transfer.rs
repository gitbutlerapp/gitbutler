//! What happens when something cut or dragged lands somewhere. Drag-and-drop and
//! ⌘X/⌘V share this, so both always agree on what a drop means.

use std::sync::Arc;

use gpui::{
    App, Context, Entity, Modifiers, Pixels, Point, SharedString, Window, div, prelude::*, px,
};

use crate::backend::{self, CommitId, Destination, Selected, short_ref};
use crate::state::{Cut, Overlay, Selection, Store};
use crate::theme::Theme;
use crate::ui::{Icon, popover, small_icon};

pub enum Transfer {
    Uncommit(CommitId),
    UncommitFiles {
        from: CommitId,
        files: Arc<[Selected]>,
    },
    MoveCommit {
        id: CommitId,
        to: Destination,
    },
    Squash {
        id: CommitId,
        into: CommitId,
    },
    Amend {
        into: CommitId,
        files: Arc<[Selected]>,
    },
    MoveFiles {
        from: CommitId,
        to: CommitId,
        files: Arc<[Selected]>,
    },
    /// A copy of the commit, leaving it where it is.
    CherryPick {
        id: CommitId,
        to: Destination,
    },
    /// Stack a branch, and those above it, on another.
    StackBranch {
        branch: String,
        onto: String,
    },
    /// Uncommitted files dropped on a branch: get them ready to commit there.
    StageOnBranch {
        branch: String,
        files: Arc<[Selected]>,
    },
}

impl Transfer {
    /// What `payload` does when it lands on `target`, if anything. A commit landing on
    /// another goes above it; with ⌥ held it squashes into it instead, and with ⇧ it goes
    /// below it.
    pub fn resolve(
        payload: &Cut,
        target: &Selection,
        held: Modifiers,
        store: &Store,
    ) -> Option<Self> {
        if payload.project() != target.project() {
            return None;
        }
        // An unnamed branch has no ref to put things on or stack onto.
        if matches!(target, Selection::Branch(_, branch) if branch.is_empty()) {
            return None;
        }
        let workspace = store.workspace(target.project())?;
        Some(match (payload, target) {
            (Cut::Commit(_, id), Selection::Uncommitted(_)) => Transfer::Uncommit(*id),
            (Cut::Commit(_, id), Selection::Branch(_, branch)) => {
                let (current, _) = workspace.find_commit(*id)?;
                // Already the top of that branch.
                if current.ref_name == *branch && current.commits.first().map(|c| c.id) == Some(*id)
                {
                    return None;
                }
                Transfer::MoveCommit {
                    id: *id,
                    to: Destination::Branch(branch.clone()),
                }
            }
            (Cut::Commit(_, id), Selection::Commit(_, target_id)) if id != target_id => {
                if held.alt {
                    return Some(Transfer::Squash {
                        id: *id,
                        into: *target_id,
                    });
                }
                // Already right above it, or right below.
                let next = |after: CommitId| {
                    let (branch, _) = workspace.find_commit(after)?;
                    let mut commits = branch.commits.iter().skip_while(|c| c.id != after);
                    commits.nth(1).map(|c| c.id)
                };
                let (already, to) = if held.shift {
                    (
                        next(*target_id) == Some(*id),
                        Destination::BelowCommit(*target_id),
                    )
                } else {
                    (
                        next(*id) == Some(*target_id),
                        Destination::AboveCommit(*target_id),
                    )
                };
                if already {
                    return None;
                }
                Transfer::MoveCommit { id: *id, to }
            }
            (Cut::CherryPick(_, id), Selection::Branch(_, branch)) => Transfer::CherryPick {
                id: *id,
                to: Destination::Branch(branch.clone()),
            },
            (Cut::CherryPick(_, id), Selection::Commit(_, target_id)) if id != target_id => {
                Transfer::CherryPick {
                    id: *id,
                    to: if held.shift {
                        Destination::BelowCommit(*target_id)
                    } else {
                        Destination::AboveCommit(*target_id)
                    },
                }
            }
            (Cut::Branch(_, branch), Selection::Branch(_, onto)) if branch != onto => {
                // Already right on top of it.
                let (stack, at) = workspace.stack_of(branch)?;
                if stack
                    .branches
                    .get(at + 1)
                    .is_some_and(|b| b.ref_name == *onto)
                {
                    return None;
                }
                Transfer::StackBranch {
                    branch: branch.clone(),
                    onto: onto.clone(),
                }
            }
            (Cut::Files { from, files, .. }, target) => match (*from, target) {
                (None, Selection::Commit(_, into)) => Transfer::Amend {
                    into: *into,
                    files: files.clone(),
                },
                (None, Selection::Branch(_, branch)) => Transfer::StageOnBranch {
                    branch: branch.clone(),
                    files: files.clone(),
                },
                (Some(from), Selection::Uncommitted(_)) => Transfer::UncommitFiles {
                    from,
                    files: files.clone(),
                },
                (Some(from), Selection::Commit(_, to)) if from != *to => Transfer::MoveFiles {
                    from,
                    to: *to,
                    files: files.clone(),
                },
                _ => return None,
            },
            _ => return None,
        })
    }

    /// What a drop does, said from the target row's point of view.
    pub fn verb(&self) -> &'static str {
        match self {
            Transfer::Uncommit(_) | Transfer::UncommitFiles { .. } => "Uncommit",
            Transfer::MoveCommit {
                to: Destination::Branch(_),
                ..
            } => "Move to top",
            Transfer::MoveCommit {
                to: Destination::AboveCommit(_),
                ..
            } => "Move above",
            Transfer::MoveCommit {
                to: Destination::BelowCommit(_),
                ..
            } => "Move below",
            Transfer::CherryPick {
                to: Destination::Branch(_),
                ..
            } => "Copy to top",
            Transfer::CherryPick {
                to: Destination::BelowCommit(_),
                ..
            } => "Copy below",
            Transfer::CherryPick { .. } => "Copy above",
            Transfer::StackBranch { .. } => "Stack on top",
            Transfer::Squash { .. } => "Squash into",
            Transfer::Amend { .. } => "Amend",
            Transfer::MoveFiles { .. } => "Move into",
            Transfer::StageOnBranch { .. } => "Commit here",
        }
    }

    /// A short verb phrase for hints, like "Squash into a1b2c3d".
    pub fn describe(&self) -> SharedString {
        match self {
            Transfer::Uncommit(_) | Transfer::UncommitFiles { .. } => "Uncommit".into(),
            Transfer::MoveCommit { to, .. } => format!("Move {}", place(to)).into(),
            Transfer::CherryPick { to, .. } => format!("Copy {}", place(to)).into(),
            Transfer::StackBranch { onto, .. } => format!("Stack on {}", short_ref(onto)).into(),
            Transfer::Squash { into, .. } => format!("Squash into {}", into.short()).into(),
            Transfer::Amend { into, .. } => format!("Amend {}", into.short()).into(),
            Transfer::MoveFiles { to, .. } => format!("Move into {}", to.short()).into(),
            Transfer::StageOnBranch { branch, .. } => {
                format!("Commit to {}", short_ref(branch)).into()
            }
        }
    }

    /// Carry it out. Staging only changes the composer, so it comes back to the caller.
    pub fn perform(
        self,
        project: &SharedString,
        store: &mut Store,
        cx: &mut Context<Store>,
    ) -> Option<(String, Arc<[Selected]>)> {
        let label = self.describe();
        match self {
            Transfer::Uncommit(id) => store.uncommit(project, vec![id], cx),
            Transfer::UncommitFiles { from, files } => store.run(
                project,
                label,
                Overlay::commit_files(store.key_of(project, from), files.iter()),
                move |path, remap| backend::uncommit_files(path, remap.get(from)?, &files),
                |store, rewritten, cx| {
                    store.follow(&rewritten, cx);
                    store.snack_undoable("Moved to uncommitted changes", cx)
                },
                cx,
            ),
            Transfer::MoveCommit { id, to } => store.run(
                project,
                label,
                Overlay::default(),
                move |path, remap| {
                    backend::move_commits(path, &[remap.get(id)?], &remap.destination(to)?)
                },
                |store, rewritten, cx| store.follow(&rewritten, cx),
                cx,
            ),
            Transfer::CherryPick { id, to } => store.run(
                project,
                label,
                Overlay::default(),
                move |path, remap| {
                    // A commit from outside the workspace can't have been rewritten.
                    let id = remap.get(id).unwrap_or(id);
                    backend::cherry_pick(path, id, &remap.destination(to)?)
                },
                |store, rewritten, cx| {
                    store.follow(&rewritten, cx);
                    store.snack_undoable("Copied", cx)
                },
                cx,
            ),
            Transfer::StackBranch { branch, onto } => store.run(
                project,
                label,
                Overlay::default(),
                move |path, _| backend::stack_branch(path, &branch, &onto),
                |store, (), cx| store.snack_undoable("Branch moved", cx),
                cx,
            ),
            Transfer::Squash { id, into } => store.run(
                project,
                label,
                Overlay::commits(store.key_of(project, id)),
                move |path, remap| backend::squash(path, &[remap.get(id)?], remap.get(into)?),
                |store, rewritten, cx| {
                    store.follow(&rewritten, cx);
                    store.snack_undoable("Squashed", cx)
                },
                cx,
            ),
            Transfer::Amend { into, files } => store.amend(project, into, files, |_, _| {}, cx),
            Transfer::MoveFiles { from, to, files } => store.run(
                project,
                label,
                Overlay::commit_files(store.key_of(project, from), files.iter()),
                move |path, remap| {
                    backend::move_files(path, remap.get(from)?, remap.get(to)?, &files)
                },
                |store, rewritten, cx| store.follow(&rewritten, cx),
                cx,
            ),
            Transfer::StageOnBranch { branch, files } => return Some((branch, files)),
        }
        None
    }
}

/// Where `to` is, said after a verb: "to feature-x", "above a1b2c3d".
fn place(to: &Destination) -> String {
    match to {
        Destination::Branch(branch) => format!("to {}", short_ref(branch)),
        Destination::AboveCommit(id) => format!("above {}", id.short()),
        Destination::BelowCommit(id) => format!("below {}", id.short()),
    }
}

/// Start a drag of a `Cut` labelled `label`, letting the store know what's in flight so
/// the rows can say what a drop would do.
pub fn drag_preview(
    store: Entity<Store>,
    label: SharedString,
) -> impl Fn(&Cut, Point<Pixels>, &mut Window, &mut App) -> Entity<DragPreview> + 'static {
    move |payload, _, _, cx| {
        store.update(cx, |store, _| store.dragging = Some(payload.clone()));
        let preview = cx.new(|_| DragPreview {
            label: label.clone(),
            payload: payload.clone(),
        });
        // GPUI drops the preview when the drag ends, dropped or not.
        let store = store.clone();
        cx.observe_release(&preview, move |_, cx| {
            store.update(cx, |store, _| store.dragging = None);
        })
        .detach();
        preview
    }
}

/// The pill that follows the pointer while dragging, and what it carries.
pub struct DragPreview {
    pub label: SharedString,
    pub payload: Cut,
}

impl Render for DragPreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let held = window.modifiers();
        let hint = match &self.payload {
            Cut::Commit(..) if held.alt => Some("Squash"),
            Cut::Commit(..) | Cut::CherryPick(..) if held.shift => Some("Below"),
            Cut::Commit(..) => Some("⌥ squash · ⇧ below"),
            Cut::CherryPick(..) => Some("⇧ below"),
            Cut::Branch(..) | Cut::Files { .. } => None,
        };
        popover(theme)
            .flex_row()
            .items_center()
            .gap(px(8.))
            .px(px(10.))
            .h(px(28.))
            .max_w(px(320.))
            .border_color(theme.pop)
            .font_family(theme.ui_font.clone())
            .text_size(px(12.))
            .text_color(theme.text_1)
            .child(
                small_icon(match self.payload {
                    Cut::Commit(..) | Cut::CherryPick(..) => Icon::Commit,
                    Cut::Branch(..) => Icon::Branch,
                    Cut::Files { .. } => Icon::File,
                })
                .text_color(theme.pop),
            )
            .child(div().truncate().child(self.label.clone()))
            .when_some(hint, |el, hint| {
                el.child(div().flex_none().text_color(theme.text_2).child(hint))
            })
    }
}
