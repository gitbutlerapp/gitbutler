#![allow(warnings)]

use but_api::WorkspaceState;
use but_core::{
    RefMetadata,
    sync::{RepoExclusive, RepoExclusiveGuard},
};
use but_ctx::Context;
use but_graph::Workspace;

use crate::{
    CliResult, IdMap,
    args::{
        atoms::{Priority, Purpose},
        uncommit::Platform,
    },
    command::legacy::squash::{self, ResolvedSquashArgsRef, SquashOperation, SquashTarget},
    theme::Theme,
    utils::{
        CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils,
        merged_upstream::MergedUpstream,
    },
};

pub fn uncommit(
    ctx: &mut Context,
    mut out: IntermediateChannel<'_>,
    args: Platform,
) -> CliResult<(squash::SquashOutcome, Option<WorkspaceState>)> {
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let id_map = IdMap::new_from_context(ctx, guard.read_permission())?;

    let merged = MergedUpstream::from_ctx(ctx, args.allow_merged)?;

    let (repo, ws, _) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
    let op = resolve(args, &ws, &repo, &id_map, &merged)?;
    drop(repo);
    drop(ws);

    Ok(squash::run(ctx, &mut meta, guard.write_permission(), op)?)
}

fn resolve(
    args: Platform,
    ws: &Workspace,
    repo: &gix::Repository,
    id_map: &IdMap,
    merged: &MergedUpstream,
) -> CliResult<SquashOperation<'static>> {
    let Platform {
        sources,
        allow_merged: _,
    } = args;

    let sources = sources
        .into_iter()
        .map(|source| {
            source.resolve_in_workspace(repo, id_map, Purpose::Source, Some(Priority::Commit))
        })
        .collect::<CliResult<Vec<_>>>()?;
    let sources = sources
        .iter()
        .map(|source| source.as_ref())
        .collect::<Vec<_>>();

    let squash_args = ResolvedSquashArgsRef::Normal {
        sources,
        target: SquashTarget::Uncommitted,
    };

    Ok(squash::resolve(squash_args, ws, repo, merged)?.into_fully_owned())
}
