use anyhow::bail;
use bstr::ByteSlice as _;
use but_core::RefMetadata;
use but_rebase::graph_rebase::{Editor, LookupStep as _};

#[derive(Debug, Clone)]
pub struct RewordInput {
    pub external_summary: String,
    pub external_prompt: String,
    pub commit_id: gix::ObjectId,
}

/// Generate and apply an AI commit-message reword for `input.commit_id`.
///
/// `llm` produces the replacement message from the event summaries and the commit diff.
/// `input` carries the commit and prompt context. `repo`, `ws`, and `meta` are supplied by the
/// caller so this action does not acquire repository guards or rebuild workspace state itself.
/// `context_lines` controls the amount of diff context shown to the message generator.
pub fn commit(
    llm: &but_llm::LLMProvider,
    input: RewordInput,
    repo: &gix::Repository,
    ws: &mut but_graph::Workspace,
    meta: &mut impl RefMetadata,
    context_lines: u32,
) -> anyhow::Result<(gix::ObjectId, String)> {
    let changes =
        but_core::diff::ui::commit_changes_with_line_stats_by_worktree_dir(repo, input.commit_id)?;
    let diff = changes.try_to_unidiff(repo, context_lines)?.to_string();
    let message = crate::generate::commit_message(
        llm,
        &input.external_summary,
        &input.external_prompt,
        &diff,
    )?;

    // Format the commit message to follow email RFC format (80 char line wrapping)
    let message = crate::commit_format::format_commit_message(&message);

    if message.is_empty() {
        bail!("commit message cannot be empty");
    }

    let editor = Editor::create(ws, meta, repo)?;
    let (rebase, edited_commit_selector) =
        but_workspace::commit::reword(editor, input.commit_id, message.as_bytes().as_bstr())?;
    let new_commit_id = rebase.lookup_pick(edited_commit_selector)?;
    rebase.materialize(Default::default())?;

    Ok((new_commit_id, message))
}
