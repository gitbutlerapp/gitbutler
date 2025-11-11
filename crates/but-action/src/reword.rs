use async_openai::{Client, config::OpenAIConfig};
use but_graph::VirtualBranchesTomlMetadata;
use but_oxidize::{ObjectIdExt, OidExt};
use but_settings::AppSettings;
use but_workspace::{StacksFilter, legacy::ui::StackEntry};
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use uuid::Uuid;

use crate::workflow::{self, Workflow};

#[derive(Debug, Clone)]
pub struct CommitEvent {
    pub project: Project,
    pub app_settings: AppSettings,
    pub external_summary: String,
    pub external_prompt: String,
    pub branch_name: String,
    pub commit_id: gix::ObjectId,
    pub trigger: Uuid,
}

pub async fn commit(
    client: &Client<OpenAIConfig>,
    event: CommitEvent,
) -> anyhow::Result<Option<(gix::ObjectId, String)>> {
    let ctx = &mut CommandContext::open(
        &event.project,
        AppSettings::load_from_default_path_creating()?,
    )?;
    let repo = &ctx.gix_repo_for_merging_non_persisting()?;
    let changes = but_core::diff::ui::commit_changes_by_worktree_dir(repo, event.commit_id)?;
    let diff = changes
        .try_to_unidiff(repo, ctx.app_settings().context_lines)?
        .to_string();
    let message = crate::generate::commit_message(
        client,
        &event.external_summary,
        &event.external_prompt,
        &diff,
    )
    .await?;
    let stacks = stacks(ctx)?;
    let stack_id = stacks
        .iter()
        .find(|s| s.heads.iter().any(|h| h.name == event.branch_name))
        .and_then(|s| s.id)
        .ok_or_else(|| anyhow::anyhow!("Stack with name '{}' not found", event.branch_name))?;
    let result = gitbutler_branch_actions::update_commit_message(
        ctx,
        stack_id,
        event.commit_id.to_git2(),
        &message,
    );
    let status = match &result {
        Ok(_) => workflow::Status::Completed,
        Err(e) => workflow::Status::Failed(e.to_string()),
    };

    let new_commit_id = result.map(|id| id.to_gix()).ok();
    let output_commits = new_commit_id.map(|id| vec![id]).unwrap_or_default();

    Workflow::new(
        workflow::Kind::Reword(Some(workflow::RewordOutcome {
            stack_id,
            branch_name: event.branch_name.clone(),
            commit_id: output_commits
                .first()
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("No output commit found"))?,
            new_message: message.clone(),
        })),
        workflow::Trigger::Snapshot(event.trigger),
        status,
        vec![event.commit_id],
        output_commits,
        None,
    )
    .persist(ctx)
    .ok();

    Ok(new_commit_id.map(|id| (id, message)))
}

fn stacks(ctx: &CommandContext) -> anyhow::Result<Vec<StackEntry>> {
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
