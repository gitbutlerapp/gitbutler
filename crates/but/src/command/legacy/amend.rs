use but_api::WorkspaceState;
use but_ctx::Context;
use but_graph::Workspace;
use but_workspace::RefInfo;
use nonempty::NonEmpty;

use crate::{
    CliResult, CliResultExt, IdMap,
    args::{
        amend::{ERROR_EXAMPLES, Platform},
        atoms::{CliIdArg, Priority, Purpose, ResolvedCliIdArg},
    },
    bad_input,
    command::legacy::squash::{
        self, HowToRewordTarget, ResolveTargetError, ResolvedSquashArgsRef, SquashOperation,
    },
    id::CommitId,
    utils::{
        IntermediateChannel,
        change_source::{ChangeSourceId, InvokedFrom, UncommittedSelection},
        merged_upstream::MergedUpstream,
    },
};

pub fn amend(
    ctx: &mut Context,
    _out: IntermediateChannel<'_>,
    args: Platform,
    invoked_from: &InvokedFrom,
) -> CliResult<(squash::SquashOutcome, Option<WorkspaceState>)> {
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let id_map = IdMap::new_from_context(ctx, guard.read_permission(), Some(invoked_from))?;

    let head_info = but_api::legacy::workspace::head_info(ctx)?;
    let merged = MergedUpstream::new(
        &*ctx.repo.get()?,
        &ctx.project_meta()?,
        &head_info,
        args.allow_merged,
    );

    let (repo, ws, _) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
    let operation = resolve(args, &ws, &repo, &id_map, &head_info, &merged, invoked_from)?;
    drop(repo);
    drop(ws);

    squash::run(ctx, &mut meta, guard.write_permission(), operation)
}

fn resolve(
    args: Platform,
    ws: &Workspace,
    repo: &gix::Repository,
    id_map: &IdMap,
    head_info: &RefInfo,
    merged: &MergedUpstream,
    invoked_from: &InvokedFrom,
) -> CliResult<SquashOperation<'static>> {
    let Platform {
        target,
        sources,
        allow_merged: _,
    } = args;

    let changes = if sources.is_empty() {
        id_map.uncommitted_files_in(&invoked_from.managed_source(id_map)?)
    } else {
        let mut changes = Vec::new();
        for source in sources {
            if target.is_none() && names_target_instead_of_change(&source, repo, id_map)? {
                return Err(bad_input(format!(
                    "Could not find uncommitted change: '{source}'"
                ))
                .hint(format!(
                    "That is a commit or branch. To amend into it, name it with `--target`:\n  but amend -t {source} <file-or-hunk>..."
                ))
                .into());
            }
            changes.extend(source.resolve_uncommitted(repo, id_map)?);
        }
        changes
    };
    // The checkout can be clean or a source can expand to nothing, e.g. a worktree without
    // changes, and squash classification requires at least one source.
    let Some(changes) = NonEmpty::from_vec(changes) else {
        return Err(crate::bad_input("No changes to amend")
            .hint("Run `but status` to show applicable targets")
            .into());
    };
    let selection = UncommittedSelection::new(changes)?;

    let target_hint = "--target must be an applied commit or branch";
    let hint = format!("{}. {}", target_hint, CliIdArg::TARGET_MISSING_HINT);
    let target = match target {
        Some(target) => target
            .resolve_in_workspace(
                repo,
                id_map,
                Purpose::Target,
                Some(Priority::BranchAndCommit),
            )
            .with_hint(|| hint.clone())?,
        None => ResolvedCliIdArg::Commit(default_target(ws, id_map, selection.source())?),
    };
    let target = match squash::resolve_target(
        target.as_ref(),
        &[],
        HowToRewordTarget::UseTargetMessage,
        head_info,
        repo,
    ) {
        Ok(target) => target,
        Err(err) => {
            return Err(match err {
                ResolveTargetError::CannotBeEmptyBranch => {
                    bad_input("--target cannot be an empty branch").into()
                }
                ResolveTargetError::NotFound => bad_input("target not found").hint(hint).into(),
                ResolveTargetError::AnonymousSegment(id) => {
                    crate::args::atoms::anonymous_segment_error(&id)
                }
                ResolveTargetError::UseTargetMessageUnavailable
                | ResolveTargetError::UseSourceMessageUnavailable
                | ResolveTargetError::NoMessageUnavailable
                | ResolveTargetError::MessageUnavailable
                | ResolveTargetError::OwnedByAnotherWorktree { .. }
                | ResolveTargetError::InvalidTarget => bad_input(target_hint)
                    .hint(CliIdArg::TARGET_MISSING_HINT)
                    .into(),
                ResolveTargetError::Other(err) => err.into(),
            });
        }
    };

    let resolved_sources = selection
        .into_changes()
        .into_iter()
        .map(|change| ResolvedCliIdArg::UncommittedHunkOrFile(Box::new(change)))
        .collect::<Vec<_>>();
    let sources = resolved_sources
        .iter()
        .map(ResolvedCliIdArg::as_ref)
        .collect();

    let args = ResolvedSquashArgsRef::Normal { sources, target };
    Ok(squash::resolve(args, ws, repo, merged)?.into_fully_owned())
}

/// Whether `source` is no uncommitted change, but a commit or branch: what `--target` takes.
fn names_target_instead_of_change(
    source: &CliIdArg,
    repo: &gix::Repository,
    id_map: &IdMap,
) -> CliResult<bool> {
    Ok(source.try_resolve_uncommitted(repo, id_map)?.is_none()
        && matches!(
            source.try_resolve(
                repo,
                id_map,
                Purpose::Target,
                Some(Priority::BranchAndCommit)
            )?,
            Some(ResolvedCliIdArg::Commit(_) | ResolvedCliIdArg::Branch(_))
        ))
}

/// The commit to amend into when no target is named: the newest commit of the only branch the
/// changes of `checkout` can be meant for. That's the branch checked out in a linked worktree or
/// in single-branch mode, whose stack also lists the branches below it, or a workspace's only
/// branch.
///
/// A branch `but status` shows without commits is refused, as the commit at its tip belongs to
/// another branch or to the target.
fn default_target(
    ws: &Workspace,
    id_map: &IdMap,
    checkout: &ChangeSourceId,
) -> CliResult<CommitId> {
    let lane = match checkout {
        ChangeSourceId::Worktree(name) => id_map.worktree_lane(name.as_ref()),
        ChangeSourceId::Head => match id_map.stacks() {
            [] => {
                return Err(bad_input("There is no branch to amend into")
                    .hint("Run `but commit` to make a first commit")
                    .into());
            }
            [stack] if stack.segments.len() == 1 || ws.kind.has_adhoc_ref() => Some(stack),
            _ => None,
        },
    };
    let Some((branch, segment)) = lane
        .and_then(|lane| lane.segments.first())
        .and_then(|segment| Some((segment.branch_name()?, segment)))
    else {
        return Err(bad_input("Unclear which commit to amend into")
            .hint(format!("Name it with `--target`.\n{ERROR_EXAMPLES}"))
            .into());
    };
    match segment.workspace_commits.first() {
        Some(tip) => Ok(tip.id()),
        None => Err(
            bad_input(format!("Branch {branch} has no commits to amend"))
                .hint("Run `but commit` to make its first commit")
                .into(),
        ),
    }
}
