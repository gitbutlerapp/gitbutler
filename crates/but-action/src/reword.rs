use but_ctx::{Context, ThreadSafeContext};
use but_oxidize::{ObjectIdExt, OidExt};
use but_workspace::legacy::{StacksFilter, ui::StackEntry};
use uuid::Uuid;

use crate::workflow::{self, Workflow};

#[derive(Debug, Clone)]
pub struct CommitEvent {
    pub ctx: ThreadSafeContext,
    pub external_summary: String,
    pub external_prompt: String,
    pub branch_name: String,
    pub commit_id: gix::ObjectId,
    pub trigger: Uuid,
}

pub fn commit(
    llm: &but_llm::LLMProvider,
    event: CommitEvent,
) -> anyhow::Result<Option<(gix::ObjectId, String)>> {
    let (diff, sync_ctx) = {
        let ctx = event.ctx.into_thread_local();
        let repo = &ctx.clone_repo_for_merging_non_persisting()?;
        let changes = but_core::diff::ui::commit_changes_with_line_stats_by_worktree_dir(
            repo,
            event.commit_id,
        )?;
        (
            changes
                .try_to_unidiff(repo, ctx.settings.context_lines)?
                .to_string(),
            ctx.into_sync(),
        )
    };
    let message = crate::generate::commit_message(
        llm,
        &event.external_summary,
        &event.external_prompt,
        &diff,
    )?;

    // Format the commit message to follow email RFC format (80 char line wrapping)
    let message = crate::commit_format::format_commit_message(&message);

    let mut ctx = sync_ctx.into_thread_local();
    let stacks = stacks(&ctx)?;
    let stack_id = stacks
        .iter()
        .find(|s| s.heads.iter().any(|h| h.name == event.branch_name))
        .and_then(|s| s.id)
        .ok_or_else(|| anyhow::anyhow!("Stack with name '{}' not found", event.branch_name))?;
    let result = gitbutler_branch_actions::update_commit_message(
        &mut ctx,
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
    .persist(&mut ctx)
    .ok();

    Ok(new_commit_id.map(|id| (id, message)))
}

fn stacks(ctx: &Context) -> anyhow::Result<Vec<StackEntry>> {
    let repo = ctx.clone_repo_for_merging_non_persisting()?;
    let meta = ctx.legacy_meta()?;
    but_workspace::legacy::stacks_v3(&repo, &meta, StacksFilter::default(), None)
}
