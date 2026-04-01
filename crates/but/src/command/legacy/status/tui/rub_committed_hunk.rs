use std::sync::Arc;

use but_ctx::Context;

use crate::{
    CliId,
    command::legacy::status::tui::{SelectAfterReload, mode::CommittedHunk},
};

pub(super) fn route_operation<'a>(
    source: &'a CommittedHunk,
    target: &'a CliId,
) -> Option<Operation<'a>> {
    // TODO(david): support more operations
    let todo_ = ();

    match target {
        CliId::Commit { commit_id, .. } => Some(Operation::AmendCommit {
            source,
            target: *commit_id,
        }),
        CliId::Uncommitted(..)
        | CliId::PathPrefix { .. }
        | CliId::CommittedFile { .. }
        | CliId::Branch { .. }
        | CliId::Unassigned { .. }
        | CliId::Stack { .. } => None,
    }
}

#[derive(Debug, Copy, Clone)]
pub(super) enum Operation<'a> {
    AmendCommit {
        source: &'a CommittedHunk,
        target: gix::ObjectId,
    },
}

impl<'a> Operation<'a> {
    pub(super) fn execute(self, ctx: &mut Context) -> anyhow::Result<SelectAfterReload> {
        match self {
            Operation::AmendCommit { source, target } => {
                let CommittedHunk {
                    commit_id: source_commit_id,
                    header,
                    path,
                } = source;

                let changes: Vec<but_core::DiffSpec> = Vec::from([but_core::DiffSpec {
                    previous_path: None,
                    path: Arc::unwrap_or_clone(Arc::clone(path)),
                    hunk_headers: Vec::from([*header]),
                }]);

                let move_result = but_api::commit::move_changes::commit_move_changes_between(
                    ctx,
                    *source_commit_id,
                    target,
                    changes,
                )?;

                Ok(move_result
                    .replaced_commits
                    .get(&target)
                    .copied()
                    .map(SelectAfterReload::Commit)
                    .unwrap_or(SelectAfterReload::Unassigned))
            }
        }
    }
}

pub(super) fn rub_operation_display(
    source: &CommittedHunk,
    target: &CliId,
) -> Option<&'static str> {
    Some(match route_operation(source, target)? {
        Operation::AmendCommit { .. } => "amend commit",
    })
}
