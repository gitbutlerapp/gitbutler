use async_openai::{Client, config::OpenAIConfig};
use but_workspace::StackId;
use gitbutler_command_context::CommandContext;
use gix::bstr::BString;

use crate::workflow::{self, Workflow};

pub struct RenameBranchParams {
    pub commit_id: gix::ObjectId,
    pub commit_message: BString,
    pub stack_id: StackId,
    pub current_branch_name: String,
    pub existing_branch_names: Vec<String>,
}

pub async fn rename_branch(
    ctx: &mut CommandContext,
    client: &Client<OpenAIConfig>,
    parameters: RenameBranchParams,
    trigger_id: uuid::Uuid,
) -> anyhow::Result<()> {
    let RenameBranchParams {
        commit_id,
        commit_message,
        stack_id,
        current_branch_name,
        existing_branch_names,
    } = parameters;
    let commit_messages = vec![commit_message.to_string()];
    let branch_name =
        crate::generate::branch_name(client, &commit_messages, &existing_branch_names).await?;
    let normalized_branch_name = gitbutler_reference::normalize_branch_name(&branch_name)?;

    let update = gitbutler_branch_actions::stack::update_branch_name(
        ctx,
        stack_id,
        current_branch_name.clone(),
        normalized_branch_name.clone(),
    )
    .map_err(|e| anyhow::anyhow!("Failed to rename branch: {}", e));

    let status = match &update {
        Ok(_) => workflow::Status::Completed,
        Err(e) => workflow::Status::Failed(e.to_string()),
    };

    Workflow::new(
        workflow::Kind::RenameBranch(workflow::RenameBranchOutcome {
            stack_id,
            old_branch_name: current_branch_name,
            new_branch_name: normalized_branch_name,
        }),
        workflow::Trigger::Snapshot(trigger_id),
        status,
        vec![commit_id],
        vec![commit_id],
        None,
    )
    .persist(ctx)
    .ok();

    Ok(())
}
