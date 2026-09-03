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
    id::{CommitId, CommittedFileId},
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

    let mut builder = DiffSpecBuilder::new(&repo, context_lines);

    let mut tree_changes = None;
    let mut head_source_commit = None;

    for source in sources {
        match source.resolve_in_workspace(&repo, id_map, Purpose::Source, None)? {
            ResolvedCliIdArg::CommittedFile(committed_file) => {
                ensure_distinct_source_commit(&mut head_source_commit, &committed_file)?;

                builder.push_changes_from_committed_file(
                    committed_file.commit_id,
                    committed_file.path.as_ref(),
                )?;
            }
            ResolvedCliIdArg::CommittedHunk(hunk) => {
                ensure_distinct_source_commit(&mut head_source_commit, &hunk.committed_file)?;

                if tree_changes.is_none() {
                    let source_commit = repo.find_commit(hunk.committed_file.commit_id)?;
                    tree_changes = Some(
                        but_core::diff::tree_changes(
                            &repo,
                            source_commit.parent_ids().next().map(|id| id.detach()),
                            hunk.committed_file.commit_id,
                        )?
                        .into_iter()
                        .map(Into::into)
                        .collect::<Vec<but_core::ui::TreeChange>>(),
                    );
                }

                builder.push_hunks_with_changes(
                    [hunk.hunk],
                    tree_changes
                        .as_ref()
                        .expect("tree changes are initialized for committed hunks"),
                )
            }
            ResolvedCliIdArg::AnonymousSegment(segment) => {
                return Err(crate::args::atoms::anonymous_segment_error(&segment.id));
            }
            other @ (ResolvedCliIdArg::Commit(..)
            | ResolvedCliIdArg::Branch(..)
            | ResolvedCliIdArg::UncommittedHunkOrFile(..)
            | ResolvedCliIdArg::Uncommitted
            | ResolvedCliIdArg::Worktree(..)
            | ResolvedCliIdArg::PathPrefix { .. }
            | ResolvedCliIdArg::Stack { .. }) => {
                return Err(bad_input(format!(
                    "Expected a committed change, got {}",
                    other.kind_for_humans()
                ))
                .into());
            }
        }
    }

    let changes = NonEmpty::from_vec(builder.into_diff_specs())
        .expect("BUG: Cannot possibly not have any changes here");
    let head_source_commit =
        head_source_commit.expect("BUG: Cannot possibly not have a head source commit here");

    Ok(MoveOperation::ChangesRelativeTo(
        MoveChangesRelativeToOperation {
            source_commit: head_source_commit.clone(),
            changes,
            target: MoveTarget::Commit {
                commit: head_source_commit,
                side: Side::Above,
            },
        },
    ))
}

fn ensure_distinct_source_commit(
    head_source_commit: &mut Option<CommitId>,
    committed_file: &CommittedFileId,
) -> CliResult<()> {
    let source_commit = head_source_commit.get_or_insert_with(|| CommitId {
        commit_id: committed_file.commit_id,
        change_id: committed_file.change_id.clone(),
    });

    if source_commit.commit_id != committed_file.commit_id {
        return Err(bad_input(format!(
            "Can only split changes from one commit. Got {} and {}",
            theme::Commit(source_commit.as_ref()),
            theme::Commit(committed_file.as_commit_ref())
        ))
        .into());
    }

    Ok(())
}
