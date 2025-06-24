use std::fmt::Debug;

use async_openai::types::{ChatCompletionRequestSystemMessage, ChatCompletionRequestUserMessage};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{OpenAiProvider, ProjectStatus, openai};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub enum BranchSuggestion {
    #[serde(rename = "new")]
    New(String),
    #[serde(rename = "existing")]
    Existing(String),
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct Group {
    #[schemars(
        description = "A list of the files in this group. They should be file paths relative to the project root."
    )]
    pub files: Vec<String>,
    #[schemars(description = "A detailed explanation of the changes in this group.
        It should not include the intention, only an overview of the actual changes made and why they are related.")]
    pub summary: String,
    #[schemars(description = "The suggested branch to apply these changes to.
        This can be either an existing branch or a new one.")]
    pub suggested_branch: BranchSuggestion,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct Grouping {
    #[schemars(description = "A list of file change groups.
        The files are grouped by related changes that should be committed together.")]
    pub steps: Vec<Group>,
}

pub fn group(openai: &OpenAiProvider, project_status: &ProjectStatus) -> anyhow::Result<Grouping> {
    let system_message ="
        You are an expert in grouping file changes into logical units for version control.
        When given the status of a project, you should be able to identify related changes and suggest how they should be grouped into commits.
        It's also important to suggest a branch for each group of changes.
        The branch can be either an existing branch or a new one.
        In order to determine the branch, you should consider diffs, the assignments and the dependency locks, if any.
        ";

    let serialized_project_status = serde_json::to_string_pretty(project_status)
        .map_err(|e| anyhow::anyhow!("Failed to serialize project status: {}", e))?;
    let user_message = format!(
        "
        Please group the file changes into logical units for version control.
        Each group should include:
        - A list of files in the group (relative paths to the project root).
        - A detailed summary of the changes in the group (not the intention, but an overview
        of the actual changes made and why they are related).
        - A suggested branch for the group of changes (either an existing branch or a new one
        with a descriptive name).

        Be granular in the grouping.
        Multiple groups can (and usually should) belong to the same branch, but each group should be a logical unit of work.
        Name the branches descriptively, in kebab-case (e.g., `fix-user-login-bug`). Don't use slashes (/).

        Here is the status of the project:
        <project_status>
        {}
        </project_status>
            ",
        serialized_project_status
    );

    println!("User message: {}", user_message);

    let messages = vec![
        ChatCompletionRequestSystemMessage::from(system_message).into(),
        ChatCompletionRequestUserMessage::from(user_message).into(),
    ];

    let grouping = openai::structured_output_blocking::<Grouping>(openai, messages)?
        .ok_or_else(|| anyhow::anyhow!("Failed to get grouping from OpenAI"))?;

    Ok(grouping)
}
