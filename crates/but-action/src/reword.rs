use anyhow::bail;
use bstr::ByteSlice as _;
use but_ctx::{Context, ThreadSafeContext};
use but_rebase::graph_rebase::{Editor, LookupStep as _};
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
    let result = (|| -> anyhow::Result<gix::ObjectId> {
        if message.is_empty() {
            bail!("commit message can not be empty");
        }

        let _guard = ctx.exclusive_worktree_access();
        let mut meta = ctx.meta()?;
        #[expect(
            deprecated,
            reason = "temporary use while this plumbing still owns Context"
        )]
        let (repo, mut ws, _) = ctx.workspace_mut_and_db_without_guard()?;
        let editor = Editor::create(&mut ws, &mut meta, &repo)?;
        let (rebase, edited_commit_selector) =
            but_workspace::commit::reword(editor, event.commit_id, message.as_bytes().as_bstr())?;
        let new_commit_id = rebase.lookup_pick(edited_commit_selector)?;
        rebase.materialize()?;
        Ok(new_commit_id)
    })();
    let status = match &result {
        Ok(_) => workflow::Status::Completed,
        Err(e) => workflow::Status::Failed(e.to_string()),
    };

    let new_commit_id = result.ok();
    let reword_outcome = new_commit_id.map(|commit_id| workflow::RewordOutcome {
        stack_id,
        branch_name: event.branch_name.clone(),
        commit_id,
        new_message: message.clone(),
    });
    let output_commits = reword_outcome
        .as_ref()
        .map(|outcome| vec![outcome.commit_id])
        .unwrap_or_default();

    Workflow::new(
        workflow::Kind::Reword(reword_outcome),
        workflow::Trigger::Snapshot(event.trigger),
        status,
        vec![event.commit_id],
        output_commits,
        None,
    )
    .persist(&ctx)
    .ok();

    Ok(new_commit_id.map(|id| (id, message)))
}

#[expect(deprecated, reason = "calls but_workspace::legacy::stacks_v3")]
fn stacks(ctx: &Context) -> anyhow::Result<Vec<StackEntry>> {
    let repo = ctx.clone_repo_for_merging_non_persisting()?;
    let meta = ctx.legacy_meta()?;
    but_workspace::legacy::stacks_v3(&repo, &meta, StacksFilter::default(), None)
}
