use std::{fmt::Debug, str};

use but_tools::workspace::ProjectStatus;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ChatMessage, OpenAiProvider, openai};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub enum BranchSuggestion {
    #[serde(rename = "new")]
    New(String),
    #[serde(rename = "existing")]
    Existing(String),
}

impl BranchSuggestion {
    #[expect(dead_code)]
    pub fn name(&self) -> String {
        match self {
            BranchSuggestion::New(name) => name.clone(),
            BranchSuggestion::Existing(name) => name.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct Group {
    #[schemars(
        description = "A list of the files in this group. They should be file paths relative to the project root."
    )]
    pub files: Vec<String>,
    #[schemars(
        description = "The commit message for this group, including a detailed explanation of the changes in this group.
        It should not include the intention, only an overview of the actual changes made and why they are related."
    )]
    pub commit_message: String,
    #[schemars(description = "The suggested branch to apply these changes to.
        This can be either an existing branch or a new one.")]
    pub suggested_branch: BranchSuggestion,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct BranchCreation {
    #[schemars(description = "The name of the branch to create.")]
    pub branch_name: String,
    #[schemars(
        description = "A detailed description of the changes in this branch, including the files changed and the commit messages."
    )]
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct Grouping {
    #[schemars(description = "The information of the branches to create.
    If any of the groups requires a new branch to be created, this field will contain the details of that branch.")]
    pub branches_to_create: Vec<BranchCreation>,
    #[schemars(description = "A list of file change groups.
        The files are grouped by related changes that should be committed together.")]
    pub groups: Vec<Group>,
}

#[expect(dead_code)]
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

        Multiple groups can (and usually should) belong to the same branch.
        Name the branches descriptively, in kebab-case (e.g., `fix-user-login-bug`). Don't use slashes (/).

        Here is the status of the project:
        <project_status>
        {serialized_project_status}
        </project_status>
            "
    );

    let messages = vec![ChatMessage::User(user_message)];

    let grouping =
        openai::structured_output_blocking::<Grouping>(openai, system_message, messages)?
            .ok_or_else(|| anyhow::anyhow!("Failed to get grouping from OpenAI"))?;

    Ok(grouping)
}
