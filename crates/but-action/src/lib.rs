//! This crate implements various automations that GitButler can perform.

use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

use but_core::TreeChange;
use but_workspace::ui::StackEntry;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::ObjectIdExt;
use gitbutler_project::{Project, access::WorktreeWritePermission};
use gitbutler_stack::{Target, VirtualBranchesHandle};
pub use openai::{CredentialsKind, OpenAiProvider};
use serde::{Deserialize, Serialize};

mod absorb;
mod action;
mod auto_commit;
mod branch_changes;
mod generate;
mod grouping;
mod openai;
pub mod reword;
mod serialize;
mod simple;
mod workflow;
pub use action::ActionListing;
pub use action::Source;
pub use action::list_actions;
use but_graph::VirtualBranchesTomlMetadata;
pub use openai::ChatMessage;
use strum::EnumString;
use uuid::Uuid;
pub use workflow::WorkflowList;
pub use workflow::list_workflows;

pub fn freestyle(
    app_handle: &tauri::AppHandle,
    ctx: &mut CommandContext,
    openai: &OpenAiProvider,
    chat_messages: Vec<openai::ChatMessage>,
) -> anyhow::Result<String> {
    let repo = ctx.gix_repo()?;

    let project_status = but_tools::workspace::get_project_status(ctx, &repo, None)?;
    let serialized_status = serde_json::to_string_pretty(&project_status)
        .map_err(|e| anyhow::anyhow!("Failed to serialize project status: {}", e))?;

    let mut toolset = but_tools::workspace::workspace_toolset(ctx, Some(app_handle))?;

    let system_message ="
    You are a GitButler agent that can perform various actions on a Git project.
    Your name is ButBot. Your main goal is to help the user with handling file changes in the project.
    Use the tools provided to you to perform the actions and respond with a summary of the action you've taken.\
    Don't be too verbose, but provide enough information to understand what you did.
    
    Please, take a look at the provided prompt and the project status below, and perform the actions you think are necessary.
    In order to do that, please follow these steps:
        1. Take a look at the prompt and reflect on what the intention of the user is.
        2. Take a look at the project status and see what changes are present in the project. It's important to understand what stacks and branche are present, and what the file changes are.
        3. Try to correlate the prompt with the project status and determine what actions you can take to help the user.
        4. Use the tools provided to you to perform the actions.
    ";

    let mut internal_chat_messages = vec![];
    let mut updated_last_user_message = false;
    for message in chat_messages.iter().rev() {
        match message {
            openai::ChatMessage::User(content) => {
                if !updated_last_user_message {
                    // Update the last user message with the prompt.
                    let prompt = format!(
                        "
                            <prompt>
                            {}
                            </prompt>

                            Here is the project status:
                            <project_status>
                            {}
                            </project_status>
                        ",
                        content, serialized_status
                    );

                    internal_chat_messages.push(openai::ChatMessage::User(prompt));
                    updated_last_user_message = true;
                } else {
                    // Add the user message as is.
                    internal_chat_messages.push(openai::ChatMessage::User(content.clone()));
                }
            }
            openai::ChatMessage::Assistant(content) => {
                // Add the assistant message as is.
                internal_chat_messages.push(openai::ChatMessage::Assistant(content.clone()));
            }
        }
    }

    // Reverse the messages to maintain the original order.
    internal_chat_messages.reverse();

    // Now we trigger the tool calling loop to absorb the remaining changes.
    let response = crate::openai::tool_calling_loop(
        openai,
        system_message,
        chat_messages,
        &mut toolset,
        Some("gpt-4.1".to_string()),
    )?;

    let response_message = response
        .choices
        .first()
        .and_then(|choice| choice.message.content.clone())
        .unwrap_or_else(|| "No response from OpenAI".to_string());

    Ok(response_message)
}

pub fn absorb(
    app_handle: &tauri::AppHandle,
    ctx: &mut CommandContext,
    openai: &OpenAiProvider,
    changes: Vec<TreeChange>,
) -> anyhow::Result<()> {
    absorb::absorb(app_handle, ctx, openai, changes)
}

pub fn branch_changes(
    app_handle: &tauri::AppHandle,
    ctx: &mut CommandContext,
    openai: &OpenAiProvider,
    changes: Vec<TreeChange>,
) -> anyhow::Result<()> {
    branch_changes::branch_changes(app_handle, ctx, openai, changes)
}

pub fn auto_commit(
    app_handle: &tauri::AppHandle,
    ctx: &mut CommandContext,
    openai: &OpenAiProvider,
    changes: Vec<TreeChange>,
) -> anyhow::Result<()> {
    auto_commit::auto_commit(app_handle, ctx, openai, changes)
}

pub fn handle_changes(
    ctx: &mut CommandContext,
    openai: &Option<OpenAiProvider>,
    change_summary: &str,
    external_prompt: Option<String>,
    handler: ActionHandler,
    source: Source,
) -> anyhow::Result<(Uuid, Outcome)> {
    match handler {
        ActionHandler::HandleChangesSimple => {
            simple::handle_changes(ctx, openai, change_summary, external_prompt, source)
        }
    }
}

fn default_target_setting_if_none(
    ctx: &CommandContext,
    vb_state: &VirtualBranchesHandle,
) -> anyhow::Result<Target> {
    if let Ok(default_target) = vb_state.get_default_target() {
        return Ok(default_target);
    }
    // Lets do the equivalent of `git symbolic-ref refs/remotes/origin/HEAD --short` to guess the default target.

    let repo = ctx.gix_repo()?;
    let remote_name = repo
        .remote_default_name(gix::remote::Direction::Push)
        .ok_or_else(|| anyhow::anyhow!("No push remote set"))?
        .to_string();

    let mut head_ref = repo
        .find_reference(&format!("refs/remotes/{}/HEAD", remote_name))
        .map_err(|_| anyhow::anyhow!("No HEAD reference found for remote {}", remote_name))?;

    let head_commit = head_ref.peel_to_commit()?;

    let remote_refname =
        gitbutler_reference::RemoteRefname::from_str(&head_ref.name().as_bstr().to_string())?;

    let target = Target {
        branch: remote_refname,
        remote_url: "".to_string(),
        sha: head_commit.id.to_git2(),
        push_remote_name: None,
    };

    vb_state.set_default_target(target.clone())?;
    Ok(target)
}

fn stacks(ctx: &CommandContext, repo: &gix::Repository) -> anyhow::Result<Vec<StackEntry>> {
    let project = ctx.project();
    if ctx.app_settings().feature_flags.ws3 {
        let meta = ref_metadata_toml(ctx.project())?;
        but_workspace::stacks_v3(repo, &meta, but_workspace::StacksFilter::InWorkspace)
    } else {
        but_workspace::stacks(
            ctx,
            &project.gb_dir(),
            repo,
            but_workspace::StacksFilter::InWorkspace,
        )
    }
}

fn ref_metadata_toml(project: &Project) -> anyhow::Result<VirtualBranchesTomlMetadata> {
    VirtualBranchesTomlMetadata::from_path(project.gb_dir().join("virtual_branches.toml"))
}

/// Returns the currently applied stacks, creating one if none exists.
fn stacks_creating_if_none(
    ctx: &CommandContext,
    vb_state: &VirtualBranchesHandle,
    repo: &gix::Repository,
    perm: &mut WorktreeWritePermission,
) -> anyhow::Result<Vec<StackEntry>> {
    let stacks = stacks(ctx, repo)?;
    if stacks.is_empty() {
        let template = gitbutler_stack::canned_branch_name(ctx.repo())?;
        let branch_name = gitbutler_stack::Stack::next_available_name(
            &ctx.gix_repo()?,
            vb_state,
            template,
            false,
        )?;
        let create_req = BranchCreateRequest {
            name: Some(branch_name),
            ownership: None,
            order: None,
            selected_for_changes: None,
        };
        let stack = gitbutler_branch_actions::create_virtual_branch(ctx, &create_req, perm)?;
        Ok(vec![stack])
    } else {
        Ok(stacks)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, EnumString, Default)]
#[serde(rename_all = "camelCase")]
pub enum ActionHandler {
    #[default]
    HandleChangesSimple,
}

impl Display for ActionHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub updated_branches: Vec<UpdatedBranch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedBranch {
    pub branch_name: String,
    pub new_commits: Vec<String>,
}
