use but_core::ref_metadata::StackId;
use but_meta::VirtualBranchesTomlMetadata;
use but_workspace::{
    legacy::{StacksFilter, ui::StackEntry},
    ui::StackDetails,
};
use gitbutler_command_context::CommandContext;

use crate::id::CliId;

pub(crate) fn all_commits(ctx: &CommandContext) -> anyhow::Result<Vec<CliId>> {
    let stacks = stacks(ctx)?
        .iter()
        .filter_map(|s| s.id.map(|id| stack_details(ctx, id)))
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    let mut matches = Vec::new();
    for stack in stacks {
        for branch in &stack.branch_details {
            for commit in &branch.upstream_commits {
                matches.push(CliId::commit(commit.id));
            }
            for commit in &branch.commits {
                matches.push(CliId::commit(commit.id));
            }
        }
    }
    Ok(matches)
}

pub(crate) fn stacks(ctx: &CommandContext) -> anyhow::Result<Vec<StackEntry>> {
    let repo = ctx.gix_repo_for_merging_non_persisting()?;
    if ctx.app_settings().feature_flags.ws3 {
        let meta = VirtualBranchesTomlMetadata::from_path(
            ctx.project().gb_dir().join("virtual_branches.toml"),
        )?;
        but_workspace::legacy::stacks_v3(&repo, &meta, StacksFilter::default(), None)
    } else {
        but_workspace::legacy::stacks(ctx, &ctx.project().gb_dir(), &repo, StacksFilter::default())
    }
}

pub(crate) fn stack_details(
    ctx: &CommandContext,
    stack_id: StackId,
) -> anyhow::Result<StackDetails> {
    if ctx.app_settings().feature_flags.ws3 {
        let repo = ctx.gix_repo_for_merging_non_persisting()?;
        let meta = VirtualBranchesTomlMetadata::from_path(
            ctx.project().gb_dir().join("virtual_branches.toml"),
        )?;
        but_workspace::legacy::stack_details_v3(Some(stack_id), &repo, &meta)
    } else {
        but_workspace::legacy::stack_details(&ctx.project().gb_dir(), stack_id, ctx)
    }
}
