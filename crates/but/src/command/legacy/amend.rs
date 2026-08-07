use but_api::WorkspaceState;
use but_ctx::Context;
use but_graph::Workspace;
use but_workspace::RefInfo;

use crate::{
    CliResult, CliResultExt, IdMap,
    args::{
        amend::Platform,
        atoms::{CliIdArg, Priority, Purpose, ResolvedCliIdArg},
    },
    bad_input,
    command::legacy::squash::{
        self, HowToRewordTarget, ResolveTargetError, ResolvedSquashArgsRef, SquashOperation,
    },
    utils::{IntermediateChannel, merged_upstream::MergedUpstream},
};

pub fn amend(
    ctx: &mut Context,
    _out: IntermediateChannel<'_>,
    args: Platform,
) -> CliResult<(squash::SquashOutcome, Option<WorkspaceState>)> {
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let id_map = IdMap::new_from_context(ctx, guard.read_permission())?;

    let head_info = but_api::legacy::workspace::head_info(ctx)?;
    let merged = MergedUpstream::new(&*ctx.repo.get()?, &head_info, args.allow_merged);

    let (repo, ws, _) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
    let operation = resolve(args, &ws, &repo, &id_map, &head_info, &merged)?;
    drop(repo);
    drop(ws);

    Ok(squash::run(
        ctx,
        &mut meta,
        guard.write_permission(),
        operation,
    )?)
}

fn resolve(
    args: Platform,
    ws: &Workspace,
    repo: &gix::Repository,
    id_map: &IdMap,
    head_info: &RefInfo,
    merged: &MergedUpstream,
) -> CliResult<SquashOperation<'static>> {
    let Platform {
        target,
        sources,
        allow_merged: _,
    } = args;

    let resolved_sources = if sources.is_empty() {
        Vec::from([ResolvedCliIdArg::Uncommitted])
    } else {
        let mut resolved_sources = Vec::new();
        for source in sources {
            resolved_sources.extend(
                source
                    .resolve_uncommitted(repo, id_map)?
                    .into_iter()
                    .map(|source| ResolvedCliIdArg::UncommittedHunkOrFile(Box::new(source))),
            );
        }
        resolved_sources
    };
    let sources = resolved_sources
        .iter()
        .map(ResolvedCliIdArg::as_ref)
        .collect();

    let target_hint = "--target must be an applied commit or branch";
    let hint = format!("{}. {}", target_hint, CliIdArg::TARGET_MISSING_HINT);
    let target = target
        .resolve_in_workspace(
            repo,
            id_map,
            Purpose::Target,
            Some(Priority::BranchAndCommit),
        )
        .with_hint(|| hint.clone())?;
    let target = match squash::resolve_target(
        target.as_ref(),
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
                ResolveTargetError::UseTargetMessageUnavailable
                | ResolveTargetError::UseSourceMessageUnavailable
                | ResolveTargetError::NoMessageUnavailable
                | ResolveTargetError::MessageUnavailable
                | ResolveTargetError::InvalidTarget => bad_input(target_hint)
                    .hint(CliIdArg::TARGET_MISSING_HINT)
                    .into(),
                ResolveTargetError::Other(err) => err.into(),
            });
        }
    };

    let args = ResolvedSquashArgsRef::Normal { sources, target };
    Ok(squash::resolve(args, ws, repo, merged)?.into_fully_owned())
}
