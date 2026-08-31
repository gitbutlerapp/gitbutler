use but_api::WorkspaceState;
use but_ctx::Context;
use nonempty::NonEmpty;

use crate::{
    CliResult, IdMap,
    args::{
        atoms::{Purpose, ResolvedCliIdArg},
        split::Platform,
    },
    bad_input,
    command::legacy::r#move::{self, MoveChangesRelativeToOperation, MoveOperation, MoveTarget},
    id::CommittedFileId,
    theme,
    utils::{
        IntermediateChannel, diff_specs::DiffSpecBuilder, merged_upstream::MergedUpstream,
        targeting::Side,
    },
};

pub fn split(
    ctx: &mut Context,
    _out: IntermediateChannel<'_>,
    args: Platform,
) -> CliResult<(r#move::MoveOutcome, WorkspaceState)> {
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let id_map = IdMap::new_from_context(ctx, guard.read_permission())?;

    let allow_merged = args.allow_merged;
    let move_op = resolve(args, ctx, &id_map)?;
    r#move::ensure_not_touching_merged_upstream(
        &move_op,
        &MergedUpstream::from_ctx(ctx, allow_merged)?,
    )?;

    Ok(r#move::run(
        ctx,
        &mut meta,
        guard.write_permission(),
        move_op,
    )?)
}

fn resolve(args: Platform, ctx: &Context, id_map: &IdMap) -> CliResult<MoveOperation> {
    let Platform {
        sources,
        allow_merged: _,
    } = args;

    let context_lines = ctx.settings.context_lines;
    let repo = ctx.repo.get()?;

    let mut resolved_sources = Vec::<CommittedFileId>::new();
    let mut builder = DiffSpecBuilder::new(&repo, context_lines);
    for source in sources {
        match source.resolve_in_workspace(&repo, id_map, Purpose::Source, None)? {
            ResolvedCliIdArg::CommittedFile(committed_file) => {
                if let Some(first) = resolved_sources.first()
                    && first.commit_id != committed_file.commit_id
                {
                    return Err(bad_input(format!(
                        "Can only split files from one commit. Got {} and {}",
                        theme::Commit(first.as_commit_ref()),
                        theme::Commit(committed_file.as_commit_ref())
                    ))
                    .into());
                }
                builder.push_changes_from_committed_file(
                    committed_file.commit_id,
                    committed_file.path.as_ref(),
                )?;
                resolved_sources.push(committed_file);
            }
            ResolvedCliIdArg::AnonymousSegment(segment) => {
                return Err(crate::args::atoms::anonymous_segment_error(&segment.id));
            }
            other @ (ResolvedCliIdArg::Commit(..)
            | ResolvedCliIdArg::CommittedHunk(..)
            | ResolvedCliIdArg::Branch(..)
            | ResolvedCliIdArg::UncommittedHunkOrFile(..)
            | ResolvedCliIdArg::Uncommitted
            | ResolvedCliIdArg::Worktree(..)
            | ResolvedCliIdArg::PathPrefix { .. }
            | ResolvedCliIdArg::Stack { .. }) => {
                return Err(bad_input(format!(
                    "Expected a committed file, got {}",
                    other.kind_for_humans()
                ))
                .into());
            }
        }
    }

    let resolved_sources = NonEmpty::from_vec(resolved_sources)
        .expect("sources is required in the clap args so they'll never be empty");

    let changes = NonEmpty::from_vec(builder.into_diff_specs())
        .expect("BUG: Cannot possibly not have any changes here");

    let source_commit = resolved_sources.head.as_commit_ref().to_owned();

    Ok(MoveOperation::ChangesRelativeTo(
        MoveChangesRelativeToOperation {
            source_commit: source_commit.clone(),
            changes,
            target: MoveTarget::Commit {
                commit: source_commit,
                side: Side::Above,
            },
        },
    ))
}
