use std::sync::Arc;

use anyhow::Context as _;
use but_core::{DryRun, ref_metadata::StackId};
use but_ctx::Context;

use crate::{
    CliId,
    command::legacy::status::tui::{SelectAfterReload, mode::CommittedHunk},
    id::ShortId,
    utils::diff_specs::DiffSpecBuilder,
};

pub(super) fn route_operation<'a>(
    source: &'a CommittedHunk,
    target: &'a CliId,
) -> Option<Operation<'a>> {
    match target {
        CliId::Commit { commit_id, .. } => Some(Operation::CommittedHunkToCommit {
            source,
            target: *commit_id,
        }),
        CliId::Unassigned { id } => Some(Operation::CommittedHunkToUnassigned { source, id }),
        CliId::Stack { stack_id, .. } => Some(Operation::CommittedHunkToStack { source, stack_id }),
        CliId::Branch { stack_id, .. } => stack_id
            .as_ref()
            .map(|stack_id| Operation::CommittedHunkToStack { source, stack_id }),
        CliId::Uncommitted(..) | CliId::PathPrefix { .. } | CliId::CommittedFile { .. } => None,
    }
}

#[derive(Debug, Copy, Clone)]
#[expect(clippy::enum_variant_names)]
pub(super) enum Operation<'a> {
    CommittedHunkToCommit {
        source: &'a CommittedHunk,
        target: gix::ObjectId,
    },
    CommittedHunkToUnassigned {
        source: &'a CommittedHunk,
        #[expect(dead_code)]
        id: &'a ShortId,
    },
    CommittedHunkToStack {
        source: &'a CommittedHunk,
        stack_id: &'a StackId,
    },
}

impl<'a> Operation<'a> {
    pub(super) fn execute(self, ctx: &mut Context) -> anyhow::Result<SelectAfterReload> {
        match self {
            Operation::CommittedHunkToCommit { source, target } => {
                let CommittedHunk {
                    commit_id: source_commit_id,
                    header,
                    path,
                } = source;

                let changes =
                    single_hunk_changes(ctx, Arc::unwrap_or_clone(Arc::clone(path)), *header)?;

                let move_result = but_api::commit::move_changes::commit_move_changes_between(
                    ctx,
                    *source_commit_id,
                    target,
                    changes,
                    DryRun::No,
                )?;

                Ok(SelectAfterReload::Commit(
                    move_result
                        .workspace
                        .replaced_commits
                        .get(&target)
                        .with_context(|| {
                            format!("{target} not found in move_result.workspace.replaced_commits")
                        })
                        .copied()?,
                ))
            }
            Operation::CommittedHunkToUnassigned { source, id: _ } => {
                let CommittedHunk {
                    commit_id: source_commit_id,
                    header,
                    path,
                } = source;

                let changes =
                    single_hunk_changes(ctx, Arc::unwrap_or_clone(Arc::clone(path)), *header)?;

                but_api::commit::uncommit::commit_uncommit_changes(
                    ctx,
                    *source_commit_id,
                    changes,
                    None,
                    DryRun::No,
                )?;

                Ok(SelectAfterReload::Unassigned)
            }
            Operation::CommittedHunkToStack { source, stack_id } => {
                let CommittedHunk {
                    commit_id: source_commit_id,
                    header,
                    path,
                } = source;

                let changes =
                    single_hunk_changes(ctx, Arc::unwrap_or_clone(Arc::clone(path)), *header)?;

                but_api::commit::uncommit::commit_uncommit_changes(
                    ctx,
                    *source_commit_id,
                    changes,
                    Some(*stack_id),
                    DryRun::No,
                )?;

                Ok(SelectAfterReload::Stack(*stack_id))
            }
        }
    }
}

fn single_hunk_changes(
    ctx: &Context,
    path: bstr::BString,
    header: but_core::HunkHeader,
) -> anyhow::Result<Vec<but_core::DiffSpec>> {
    let context_lines = ctx.settings.context_lines;
    let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
    let mut builder = DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines);
    builder.push_changes_from_single_hunk(path, header);
    Ok(builder.into_diff_specs())
}

pub(super) fn rub_operation_display(
    source: &CommittedHunk,
    target: &CliId,
) -> Option<&'static str> {
    Some(match route_operation(source, target)? {
        Operation::CommittedHunkToCommit { .. } => "amend commit",
        Operation::CommittedHunkToUnassigned { .. } | Operation::CommittedHunkToStack { .. } => {
            "unassign hunk"
        }
    })
}
