use anyhow::Context;
use but_tools::workspace::amend_toolset;
use gitbutler_command_context::CommandContext;

use crate::OpenAiProvider;

pub fn absorb(
    app_handle: &tauri::AppHandle,
    ctx: &mut CommandContext,
    openai: &OpenAiProvider,
    changes: Vec<but_core::TreeChange>,
) -> anyhow::Result<()> {
    let repo = ctx.gix_repo()?;

    let paths = changes
        .iter()
        .map(|change| change.path.clone())
        .collect::<Vec<_>>();
    let path_strings = paths.iter().map(|p| p.to_string()).collect::<Vec<String>>();
    let path_strings = path_strings.join("\n");
    let project_status = but_tools::workspace::get_project_status(ctx, &repo, Some(paths))?;
    let serialized_status = serde_json::to_string_pretty(&project_status)
        .context("Failed to serialize project status")?;

    let mut toolset = amend_toolset(ctx, Some(app_handle))?;

    let system_message ="
       You are an expert in finding where to put file changes in a project.
        When given the status of a project, you should be able to identify the commits in which the changes should be amended.
        ";

    let prompt = format!("
        Please, figure out how to absorb the file changes into the existing commits in the project.
        Follow these steps:
        1. Take a look at the existing commits and the file changes.
        2. Determine which files belong to which commit.
        3. Amend a file (or set of files) into their corresponding commit.
        4. Get the project status again and repeat the process until all changes are absorbed.

        ### Determining where to put changes:
        In order to determine where to put the changes do this in order:
        1. Take a look at the existing dependency locks. If there are any locks pointing to a commit ID, that is the commit where the changes should be absorbed.
        2. If there are no locks, look at the assingments. If there are any assignments pointing to a stack ID, that is the stack where the changes should be absorbed.
           Already knowing the stack ID, look at the commit messages inside the stack branches and try to find the commit that is related to the changes.
        3. If there are no assignments, look at the descriptions of the branches and commit messages. Try to find the branch and commit that most closely matches the changes.
        4. If there are no branche or commits that match the change, don't do anything. The changes will be left unabsorbed.

        <important_note>
            Only absorb changes specified by the user
        </important_note>

        Here are the file changes to absorb:
        <file_changes>
                {}
        </file_changes>

        Here is the project status:
        <project_status>
                {}
        </project_status>
    ", path_strings,  serialized_status);

    crate::openai::tool_calling_loop(openai, system_message, &prompt, &mut toolset)?;

    Ok(())
}
