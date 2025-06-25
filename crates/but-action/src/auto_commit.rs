use but_tools::workspace::{
    CommitParameters, CreateBranchParameters, create_branch, create_commit,
};
use gitbutler_command_context::CommandContext;

use crate::OpenAiProvider;

pub fn auto_commit(
    ctx: &mut CommandContext,
    openai: &OpenAiProvider,
    changes: Vec<but_core::TreeChange>,
) -> anyhow::Result<()> {
    let repo = ctx.gix_repo()?;

    let project_status = crate::get_project_status(ctx, &repo, Some(changes))?;

    let grouping = crate::grouping::group(openai, &project_status)?;
    for branch in &grouping.branches_to_create {
        let name = branch.branch_name.clone();
        let description = branch.description.clone();
        create_branch(
            ctx,
            CreateBranchParameters {
                branch_name: name,
                description,
            },
        )?;
    }

    for group in &grouping.groups {
        let commit_message = group.commit_message.clone();
        let files = group.files.clone();
        let branch_name = group.suggested_branch.name();

        create_commit(
            ctx,
            CommitParameters {
                message: commit_message,
                branch_name,
                files,
            },
        )?;
    }

    Ok(())
}
