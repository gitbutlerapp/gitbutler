use anyhow::bail;
use bstr::ByteSlice as _;
use but_core::RefMetadata;
use but_graph::edit::MaterializeOptions;

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

    let graph = ws.graph.clone().into_mut(repo)?;
    let (rebase, edited_commit_selector) =
        but_workspace::commit::reword(graph, input.commit_id, message.as_bytes().as_bstr())?;
    let new_commit_id = match rebase.pick_at(edited_commit_selector) {
        Some(pick) => pick.id,
        None => bail!("BUG: reworded commit selector did not resolve to a pick"),
    };
    *ws = rebase
        .materialize_changes(&*meta, MaterializeOptions::default())?
        .workspace;

    Ok((new_commit_id, message))
}
