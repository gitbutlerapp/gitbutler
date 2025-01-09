use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use gitbutler_repo::{
    hooks::{self, HookResult, MessageHookResult},
    staging,
};
use gitbutler_stack::BranchOwnershipClaims;

pub fn pre_commit(
    project: &Project,
    ownership: &BranchOwnershipClaims,
) -> Result<HookResult, anyhow::Error> {
    let ctx = CommandContext::open(project)?;
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
    hooks::pre_commit(&ctx, &selected_files)
}

pub fn post_commit(project: &Project) -> Result<HookResult, anyhow::Error> {
    let ctx = CommandContext::open(project)?;
    hooks::post_commit(&ctx)
}

pub fn commit_msg(project: &Project, message: String) -> Result<MessageHookResult, anyhow::Error> {
    let ctx = CommandContext::open(project)?;
    hooks::commit_msg(&ctx, message)
}
