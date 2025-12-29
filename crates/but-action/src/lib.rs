//! This crate implements various automations that GitButler can perform.

use std::{
    fmt::{Debug, Display},
    str::FromStr,
    sync::Arc,
};

use but_core::TreeChange;
use but_ctx::{Context, access::WorktreeWritePermission};
use but_oxidize::ObjectIdExt;
use but_tools::emit::{Emittable, Emitter, TokenUpdate};
use but_workspace::legacy::ui::StackEntry;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_project::{Project, ProjectId};
use gitbutler_stack::{Target, VirtualBranchesHandle};
pub use openai::{CredentialsKind, OpenAiProvider};
use serde::{Deserialize, Serialize};

mod absorb;
mod action;
mod auto_commit;
mod branch_changes;
pub mod cli;
mod generate;
mod grouping;
mod openai;
pub mod rename_branch;
pub mod reword;
mod simple;
mod workflow;
pub use action::{ActionListing, Source, list_actions};
use but_core::ref_metadata::StackId;
use but_meta::VirtualBranchesTomlMetadata;
pub use openai::{
    ChatMessage, ToolCallContent, ToolResponseContent, structured_output_blocking,
    tool_calling_loop, tool_calling_loop_stream,
};
use strum::EnumString;
use uuid::Uuid;
pub use workflow::{WorkflowList, list_workflows};

pub fn freestyle(
    project_id: ProjectId,
    message_id: String,
    emitter: Arc<Emitter>,
    ctx: &mut Context,
    openai: &OpenAiProvider,
    chat_messages: Vec<openai::ChatMessage>,
    model: Option<String>,
) -> anyhow::Result<String> {
    let project_status = but_tools::workspace::get_project_status(ctx, None)?;
    let serialized_status = serde_json::to_string_pretty(&project_status)
        .map_err(|e| anyhow::anyhow!("Failed to serialize project status: {}", e))?;

    let mut toolset =
        but_tools::workspace::workspace_toolset(ctx, emitter.clone(), message_id.clone());

    let system_message ="
    You are a GitButler agent that can perform various actions on a Git project.
    Your name is ButBot. Your main goal is to help the user with handling file changes in the project.
    Use the tools provided to you to perform the actions and respond with a summary of the action you've taken.
    Don't be too verbose, but be thorough and outline everything you did.

    ### Core concepts
    - **Project**: A Git repository that has been initialized with GitButler.
    - **Stack**: A collection of dependent branches that are used to manage changes in the project. With GitButler (as opposed to normal Git), multiple stacks can be applied at the same time.
    - **Branch**: A pointer to a specific commit in the project. Branches can contain multiple commits. Commits are always listed newest to oldest.
    - **Commit**: A snapshot of the project at a specific point in time.
    - **File changes**: A set of changes made to the files in the project. This can include additions, deletions, and modifications of files. The user can assign these changes to stacks to keep things ordering.
    - **Lock**: A lock or dependency on a file change. This refers to the fact that certain uncommitted file changes can only be committed to a specific stack.
        This is because the uncommitted changes were done on top of previously committed file changes that are part of the stack.

    ### Main task
    Please, take a look at the provided prompt and the project status below, and perform the actions you think are necessary.
    In order to do that, please follow these steps:
        1. Take a look at the prompt and reflect on what the intention of the user is.
        2. Take a look at the project status and see what changes are present in the project. It's important to understand what stacks and branch are present, and what the file changes are.
        3. Try to correlate the prompt with the project status and determine what actions you can take to help the user.
        4. Use the tools provided to you to perform the actions.

    ### Capabilities
    You can generally perform the normal Git operations, such as creating branches and committing to them.
    You can also perform more advanced operations, such as:
    - `absorb`: Take a set of file changes and amend them into the existing commits in the project.
      This requires you to figure out where the changes should go based on the locks, assignments and any other user provided information.
    - `split a commit`: Take an existing commit and split it into multiple commits based on the the user directive.
        This can be achieved by using the `split_commit` tool.
    - `split a branch`: Take an existing branch and split it into two branches. This basically takes a set of committed file changes and moves them to a new branch, removing them from the original branch.
        This is useful when you want to separate the changes into a new branch for further work.
        In order to do this, you will need to get the branch changes for the intended source branch (call the `get_branch_changes` tool), and then call the split branch tool with the changes you want to split off.

    ### Important notes
    - Only perform the action on the file changes specified in the prompt.
    - If the prompt is not clear, ask the user for clarification.
    - When told to commit to the existing branch, commit to the applied stack-branch. Don't create a new branch unless explicitly asked to do so.
    ";

    let mut internal_chat_messages = chat_messages;

    // Add the project status to the chat messages.
    internal_chat_messages.push(ChatMessage::ToolCall(ToolCallContent {
        id: "project_status".to_string(),
        name: "get_project_status".to_string(),
        arguments: "{\"filterChanges\": null}".to_string(),
    }));

    internal_chat_messages.push(ChatMessage::ToolResponse(ToolResponseContent {
        id: "project_status".to_string(),
        result: serialized_status,
    }));

    // Now we trigger the tool calling loop.
    let message_id_cloned = message_id.clone();
    let project_id_cloned = project_id;
    let on_token_cb: Arc<dyn Fn(&str) + Send + Sync + 'static> = Arc::new({
        let emitter = emitter.clone();
        let message_id = message_id_cloned;
        let project_id = project_id_cloned;
        move |token: &str| {
            let token_update = TokenUpdate {
                token: token.to_string(),
                project_id,
                message_id: message_id.clone(),
            };
            let (name, payload) = token_update.emittable();
            (emitter)(&name, payload);
        }
    });
    let (response, _) = crate::openai::tool_calling_loop_stream(
        openai,
        system_message,
        internal_chat_messages,
        &mut toolset,
        model,
        on_token_cb,
    )?;

    Ok(response)
}

pub fn absorb(
    emitter: Arc<Emitter>,
    ctx: &mut Context,
    openai: &OpenAiProvider,
    changes: Vec<TreeChange>,
) -> anyhow::Result<()> {
    absorb::absorb(emitter, ctx, openai, changes)
}

pub fn branch_changes(
    emitter: Arc<Emitter>,
    ctx: &mut Context,
    openai: &OpenAiProvider,
    changes: Vec<TreeChange>,
) -> anyhow::Result<()> {
    branch_changes::branch_changes(emitter, ctx, openai, changes)
}

pub fn auto_commit(
    emitter: Arc<Emitter>,
    ctx: &mut Context,
    openai: &OpenAiProvider,
    changes: Vec<TreeChange>,
) -> anyhow::Result<()> {
    auto_commit::auto_commit(emitter, ctx, openai, changes)
}

pub fn handle_changes(
    ctx: &mut Context,
    change_summary: &str,
    external_prompt: Option<String>,
    handler: ActionHandler,
    source: Source,
    exclusive_stack: Option<StackId>,
) -> anyhow::Result<(Uuid, Outcome)> {
    match handler {
        ActionHandler::HandleChangesSimple => simple::handle_changes(
            ctx,
            change_summary,
            external_prompt,
            source,
            exclusive_stack,
        ),
    }
}

fn default_target_setting_if_none(
    ctx: &Context,
    vb_state: &VirtualBranchesHandle,
) -> anyhow::Result<Target> {
    if let Ok(default_target) = vb_state.get_default_target() {
        return Ok(default_target);
    }
    // Lets do the equivalent of `git symbolic-ref refs/remotes/origin/HEAD --short` to guess the default target.

    let repo = ctx.repo.get()?;
    let remote_name = repo
        .remote_default_name(gix::remote::Direction::Push)
        .ok_or_else(|| anyhow::anyhow!("No push remote set"))?
        .to_string();

    let mut head_ref = repo
        .find_reference(&format!("refs/remotes/{remote_name}/HEAD"))
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

fn stacks(ctx: &Context, repo: &gix::Repository) -> anyhow::Result<Vec<StackEntry>> {
    let meta = ref_metadata_toml(&ctx.legacy_project)?;
    but_workspace::legacy::stacks_v3(
        repo,
        &meta,
        but_workspace::legacy::StacksFilter::InWorkspace,
        None,
    )
}

fn ref_metadata_toml(project: &Project) -> anyhow::Result<VirtualBranchesTomlMetadata> {
    VirtualBranchesTomlMetadata::from_path(project.gb_dir().join("virtual_branches.toml"))
}

/// Returns the currently applied stacks, creating one if none exists.
fn stacks_creating_if_none(
    ctx: &Context,
    vb_state: &VirtualBranchesHandle,
    repo: &gix::Repository,
    perm: &mut WorktreeWritePermission,
) -> anyhow::Result<Vec<StackEntry>> {
    let stacks = stacks(ctx, repo)?;
    if stacks.is_empty() {
        let template = gitbutler_stack::canned_branch_name(&*ctx.git2_repo.get()?)?;
        let branch_name = gitbutler_stack::Stack::next_available_name(
            &*ctx.repo.get()?,
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
        Ok(vec![stack.into()])
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
    pub stack_id: StackId,
    pub branch_name: String,
    pub new_commits: Vec<String>,
}
