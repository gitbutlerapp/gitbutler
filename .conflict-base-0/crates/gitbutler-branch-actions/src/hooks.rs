use gitbutler_command_context::CommandContext;
use gitbutler_repo::{
    hooks::{self, HookResult},
    staging,
};
use gitbutler_stack::BranchOwnershipClaims;

pub fn pre_commit(
    ctx: &CommandContext,
    ownership: &BranchOwnershipClaims,
) -> Result<HookResult, anyhow::Error> {
    let repo = ctx.repo();
    let diffs = gitbutler_diff::workdir(ctx.repo(), repo.head()?.peel_to_commit()?.id())?;
    let selected_files = staging::filter_diff_by_hunk_ids(
        diffs,
        ownership
            .claims
            .iter()
            .map(|claim| (&claim.file_path, &claim.hunks))
            .collect(),
    )?;
    hooks::pre_commit(ctx, &selected_files)
}
