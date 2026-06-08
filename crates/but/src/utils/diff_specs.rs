use but_core::{DiffSpec, ref_metadata::StackId};

use crate::{
    CliId,
    id::{ShortId, UncommittedCliId},
};

/// A subset of [`CliId`] that supports being committed
#[derive(Debug)]
pub enum CommitSource {
    Unassigned(UnassignedCommitSource),
    Uncommitted(Box<UncommittedCliId>),
    Stack(StackCommitSource),
}

#[derive(Debug)]
pub struct UnassignedCommitSource {
    pub id: ShortId,
}

#[derive(Debug)]
pub struct StackCommitSource {
    pub stack_id: StackId,
}

pub fn prepare_changes_to_commit(
    db: &mut but_db::DbHandle,
    repo: &gix::Repository,
    workspace: &but_graph::Workspace,
    context_lines: u32,
    source: &CommitSource,
    scope_to_stack: Option<StackId>,
) -> anyhow::Result<Vec<DiffSpec>> {
    // find what to commit
    let changes_to_commit = match source {
        CommitSource::Unassigned(..) => {
            let changes = but_core::diff::ui::worktree_changes(repo)?.changes;
            let (assignments, _assignments_error) = but_hunk_assignment::assignments_with_fallback(
                db.hunk_assignments_mut()?,
                repo,
                workspace,
                Some(changes.clone()),
                context_lines,
            )?;
            let assignments = assignments
                .into_iter()
                .filter(|assignment| assignment.stack_id.is_none());
            but_hunk_assignment::diff_specs_from_assignments_with_changes(assignments, &changes)
        }
        CommitSource::Uncommitted(uncommitted_cli_id) => {
            let changes = but_core::diff::ui::worktree_changes(repo)?.changes;
            let assignments = uncommitted_cli_id
                .hunk_assignments
                .iter()
                .filter(|assignment| assignment.stack_id == scope_to_stack)
                .cloned();
            but_hunk_assignment::diff_specs_from_assignments_with_changes(assignments, &changes)
        }
        CommitSource::Stack(StackCommitSource { stack_id, .. }) => {
            let changes = but_core::diff::ui::worktree_changes(repo)?.changes;
            let (assignments, _assignments_error) = but_hunk_assignment::assignments_with_fallback(
                db.hunk_assignments_mut()?,
                repo,
                workspace,
                Some(changes.clone()),
                context_lines,
            )?;
            let assignments = assignments
                .into_iter()
                .filter(|assignment| assignment.stack_id.is_some_and(|id| &id == stack_id));
            but_hunk_assignment::diff_specs_from_assignments_with_changes(assignments, &changes)
        }
    };

    Ok(but_workspace::flatten_diff_specs(changes_to_commit))
}

impl CommitSource {
    pub fn try_new(id: CliId) -> Option<Self> {
        match id {
            CliId::Unassigned { id } => Some(Self::Unassigned(UnassignedCommitSource { id })),
            CliId::Uncommitted(uncommitted_cli_id) => {
                Some(Self::Uncommitted(Box::new(uncommitted_cli_id)))
            }
            CliId::Stack { stack_id, .. } => Some(Self::Stack(StackCommitSource { stack_id })),
            CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Branch { .. }
            | CliId::Commit { .. } => None,
        }
    }
}

impl PartialEq<CliId> for CommitSource {
    fn eq(&self, other: &CliId) -> bool {
        match self {
            CommitSource::Unassigned(UnassignedCommitSource { id: lhs_id }) => {
                if let CliId::Unassigned { id: rhs_id } = other {
                    lhs_id == rhs_id
                } else {
                    false
                }
            }
            CommitSource::Uncommitted(lhs) => {
                if let CliId::Uncommitted(rhs) = other {
                    &**lhs == rhs
                } else {
                    false
                }
            }
            CommitSource::Stack(StackCommitSource {
                stack_id: stack_id_lhs,
            }) => {
                if let CliId::Stack {
                    stack_id: stack_id_rhs,
                    ..
                } = other
                {
                    stack_id_lhs == stack_id_rhs
                } else {
                    false
                }
            }
        }
    }
}
