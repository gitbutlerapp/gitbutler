use anyhow::Context as _;
use but_ctx::Context;
use but_tools::{emit::Emitter, workspace::commit_toolset};

use crate::OpenAiProvider;

pub(crate) fn branch_changes(
    emitter: std::sync::Arc<Emitter>,
    ctx: &mut Context,
    openai: &OpenAiProvider,
    changes: Vec<but_core::TreeChange>,
) -> anyhow::Result<()> {
    let paths = changes
        .iter()
        .map(|change| change.path.clone())
        .collect::<Vec<_>>();
    let project_status = but_tools::workspace::get_project_status(ctx, Some(paths))?;
    let serialized_status = serde_json::to_string_pretty(&project_status)
        .context("Failed to serialize project status")?;

    let mut toolset = commit_toolset(ctx, emitter.clone());

    let system_message ="
        You are an expert in grouping and committing file changes into logical units for version control.
        When given the status of a project, you should be able to identify related changes and suggest how they should be grouped into commits.
        ";

    let prompt = format!("
        Please, figure out how to group the file changes into logical units for version control and commit them.
        Follow these steps:
        1. Create a new branch for the change. All commits should be made to this branch.
        1. Take a look at the existing branches and the file changes. You can see all this information in the **project status** below.
        2. Determine which are the related changes that should be grouped together. You can do this by looking at the diffs, assignments, and dependency locks, if any.
        3. For each group of changes, create a commit (using the provided tool) with a detailed summary of the changes in the group (not the intention, but an overview of the actual changes made and why they are related).
        4. When you're done, only send the message 'done'

        Grouping rules:
        - Group changes that modify files within the same feature, module, or directory.
        - If multiple files are changed together to implement a single feature or fix, group them in one commit.
        - Dependency updates (e.g., lockfiles or package manifests) should be grouped separately from code changes unless they are tightly coupled.
        - Refactoring or formatting changes that affect many files but do not change functionality should be grouped together.
        - Avoid grouping unrelated changes in the same commit.
        - Aim to keep commits small and focused, but don't go overboard with tiny commits that don't add value.

        Here is the project status:
        <project_status>
                {serialized_status}
        </project_status>
    ");

    crate::openai::tool_calling_loop(
        openai,
        system_message,
        vec![prompt.into()],
        &mut toolset,
        None,
    )?;

    Ok(())
}
