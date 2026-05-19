use anyhow::bail;
use bstr::ByteSlice as _;
use but_core::RefMetadata;
use but_ctx::ThreadSafeContext;
use but_rebase::graph_rebase::{Editor, LookupStep as _};
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

/// Generate and apply an AI commit-message reword for `event.commit_id`.
///
/// `llm` produces the replacement message from the event summaries and the commit diff.
/// `event` carries the commit, stack branch name, workflow trigger, and thread-safe context used
/// only for workflow persistence after the reword attempt. `repo`, `ws`, and `meta` are supplied by
/// the caller so this action does not acquire repository guards or rebuild workspace state itself.
/// `context_lines` controls the amount of diff context shown to the message generator.
pub fn commit(
    llm: &but_llm::LLMProvider,
    event: CommitEvent,
    repo: &gix::Repository,
    ws: &mut but_graph::Workspace,
    meta: &mut impl RefMetadata,
    context_lines: u32,
) -> anyhow::Result<Option<(gix::ObjectId, String)>> {
    let changes =
        but_core::diff::ui::commit_changes_with_line_stats_by_worktree_dir(repo, event.commit_id)?;
    let diff = changes.try_to_unidiff(repo, context_lines)?.to_string();
    let message = crate::generate::commit_message(
        llm,
        &event.external_summary,
        &event.external_prompt,
        &diff,
    )?;

    // Format the commit message to follow email RFC format (80 char line wrapping)
    let message = crate::commit_format::format_commit_message(&message);

    let stack_id = ws
        .stacks
        .iter()
        .find(|stack| {
            stack
                .ref_name()
                .is_some_and(|name| name.shorten() == event.branch_name)
        })
        .and_then(|stack| stack.id)
        .ok_or_else(|| anyhow::anyhow!("Stack with name '{}' not found", event.branch_name))?;
    let result = (|| -> anyhow::Result<gix::ObjectId> {
        if message.is_empty() {
            bail!("commit message can not be empty");
        }

        let editor = Editor::create(ws, meta, repo)?;
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
    .persist(&event.ctx.into_thread_local())
    .ok();

    Ok(new_commit_id.map(|id| (id, message)))
}
