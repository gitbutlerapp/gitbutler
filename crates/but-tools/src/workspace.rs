use std::sync::Arc;

use but_graph::VirtualBranchesTomlMetadata;
use but_workspace::ui::StackEntry;
use gitbutler_command_context::CommandContext;
use gitbutler_oplog::{OplogExt, SnapshotExt};
use gitbutler_project::Project;
use gitbutler_stack::{PatchReferenceUpdate, VirtualBranchesHandle};
use schemars::{JsonSchema, schema_for};

use crate::emit::EmitStackUpdate;
use crate::tool::{Tool, ToolResult, Toolset};

/// Creates a toolset for workspace-related operations.
pub fn commit_toolset<'a>(
    ctx: &'a mut CommandContext,
    app_handle: Option<&'a tauri::AppHandle>,
) -> anyhow::Result<Toolset<'a>> {
    let mut toolset = Toolset::new(ctx, app_handle);

    toolset.register_tool(Commit);
    toolset.register_tool(CreateBranch);

    Ok(toolset)
}

pub struct Commit;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommitParameters {
    /// The commit title.
    #[schemars(description = "
    <description>
        The commit message title.
        This is only a short summary of the commit.
    </description>

    <important_notes>
        The commit message title should be concise and descriptive.
        It is typically a single line that summarizes the changes made in the commit.
        For example: 'Fix issue with user login' or 'Update README with installation instructions'.
        Don't excede 50 characters in length.
    </important_notes>
    ")]
    pub message_title: String,
    /// The commit description.
    #[schemars(description = "
    <description>
        The commit message body.
        This is a more detailed description of the changes made in the commit.
    </description>

    <important_notes>
        The commit message body should provide context and details about the changes made.
        It should span multiple lines if necessary.
        A good description focuses on describing the 'what' of the changes.
        Don't make assumption about the 'why', only describe the changes in the context of the branch (and other commits if any).
    </important_notes>
    ")]
    pub message_body: String,
    /// The branch name to commit to.
    #[schemars(description = "
    <description>
        The name of the branch to commit to.
        If this is the name of a branch that does not exist, it will be created.
    </description>

    <important_notes>
        The branch name should be a valid Git branch name.
        It should not contain spaces or special characters.
        Keep it to maximum 5 words, and use hyphens to separate words.
        Don't use slashes or other special characters.
    </important_notes>
    ")]
    pub branch_name: String,
    /// The branch description.
    #[schemars(description = "
    <description>
        The description of the branch.
        This is a short summary of the branch's purpose.
        If the branch already exists, this will be overwritten with this description.
    </description>

    <important_notes>
        The branch description should be a concise summary of the branch's purpose and changes.
        It's important to keep it clear and informative.
        This description should also point out which kind of changes should be assigned to this branch.
    </important_notes>
    ")]
    pub branch_description: String,
    /// The list of files to commit.
    #[schemars(description = "
        <description>
            The list of file paths to commit.
        </description>

        <important_notes>
            The file paths should be relative to the workspace root.
        </important_notes>
        ")]
    pub files: Vec<String>,
}

/// Commit tool.
///
/// Takes in a commit message, target branch name, and a list of file paths to commit.
impl Tool for Commit {
    fn name(&self) -> String {
        "commit".to_string()
    }

    fn description(&self) -> String {
        "
        <description>
            Commit file changes to a branch in the workspace.
        </description>

        <important_notes>
            This tool allows you to commit changes to a specific branch in the workspace.
            You can specify the commit message, target branch name, and a list of file paths to commit.
            If the branch does not exist, it will be created.
        </important_notes>
        ".to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        let schema = schema_for!(CommitParameters);
        serde_json::to_value(&schema).unwrap_or_default()
    }

    fn call(
        self: Arc<Self>,
        parameters: serde_json::Value,
        ctx: &mut CommandContext,
        app_handle: Option<&tauri::AppHandle>,
    ) -> anyhow::Result<serde_json::Value> {
        let params: CommitParameters = serde_json::from_value(parameters)
            .map_err(|e| anyhow::anyhow!("Failed to parse input parameters: {}", e))?;

        let value = create_commit(ctx, app_handle, params).to_json("create_commit");
        Ok(value)
    }
}

pub fn create_commit(
    ctx: &mut CommandContext,
    app_handle: Option<&tauri::AppHandle>,
    params: CommitParameters,
) -> Result<but_workspace::commit_engine::ui::CreateCommitOutcome, anyhow::Error> {
    let repo = ctx.gix_repo()?;
    let mut guard = ctx.project().exclusive_worktree_access();
    let worktree = but_core::diff::worktree_changes(&repo)?;
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());

    let file_changes: Vec<but_workspace::DiffSpec> = worktree
        .changes
        .iter()
        .filter(|change| params.files.contains(&change.path.to_string()))
        .map(Into::into)
        .collect::<Vec<_>>();

    let stacks = stacks(ctx, &repo)?;

    let stack_id = stacks
        .iter()
        .find_map(|s| {
            let found = s.heads.iter().any(|h| h.name == params.branch_name);
            if found { Some(s.id) } else { None }
        })
        .unwrap_or_else(|| {
            let perm = guard.write_permission();

            let branch = gitbutler_branch::BranchCreateRequest {
                name: Some(params.branch_name.clone()),
                ..Default::default()
            };

            let stack = gitbutler_branch_actions::create_virtual_branch(ctx, &branch, perm)
                .expect("Failed to create virtual branch");
            stack.id
        });

    // Update the branch description.
    let mut stack = vb_state.get_stack(stack_id)?;
    stack.update_branch(
        ctx,
        params.branch_name.clone(),
        &PatchReferenceUpdate {
            description: Some(Some(params.branch_description)),
            ..Default::default()
        },
    )?;

    let snapshot_tree = ctx.prepare_snapshot(guard.read_permission());

    let message = format!(
        "{}\n\n{}",
        params.message_title.trim(),
        params.message_body.trim()
    );

    let outcome = but_workspace::commit_engine::create_commit_simple(
        ctx,
        stack_id,
        None,
        file_changes,
        message.clone(),
        params.branch_name.clone(),
        guard.write_permission(),
    );

    let _ = snapshot_tree.and_then(|snapshot_tree| {
        ctx.snapshot_commit_creation(
            snapshot_tree,
            outcome.as_ref().err(),
            message.clone(),
            None,
            guard.write_permission(),
        )
    });

    // If there's an app handle provided, emit an event to update the stack details in the UI.
    if let Some(app_handle) = app_handle {
        let project_id = ctx.project().id;
        app_handle.emit_stack_update(project_id, stack_id);
    }

    let outcome: but_workspace::commit_engine::ui::CreateCommitOutcome = outcome?.into();
    Ok(outcome)
}

fn stacks(
    ctx: &CommandContext,
    repo: &gix::Repository,
) -> anyhow::Result<Vec<but_workspace::ui::StackEntry>> {
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

pub struct CreateBranch;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateBranchParameters {
    /// The name of the branch to create.
    #[schemars(description = "
    <description>
        The name of the branch to create.
        If this is the name of a branch that does not exist, it will be created.
    </description>

    <important_notes>
        The branch name should be a valid Git branch name.
        It should not contain spaces or special characters.
        Keep it to maximum 5 words, and use hyphens to separate words.
        Don't use slashes or other special characters.
    </important_notes>
    ")]
    pub branch_name: String,
    /// The branch description.
    #[schemars(description = "
    <description>
        The description of the branch.
        This is a short summary of the branch's purpose.
    </description>

    <important_notes>
        The branch description should be a concise summary of the branch's purpose and changes.
        It's important to keep it clear and informative.
        This description should also point out which kind of changes should be assigned to this branch.
    </important_notes>
    ")]
    pub branch_description: String,
}

impl Tool for CreateBranch {
    fn name(&self) -> String {
        "create_branch".to_string()
    }

    fn description(&self) -> String {
        "
        <description>
            Create a new branch in the workspace.
        </description>
        "
        .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        let schema = schema_for!(CreateBranchParameters);
        serde_json::to_value(&schema).unwrap_or_default()
    }

    fn call(
        self: Arc<Self>,
        parameters: serde_json::Value,
        ctx: &mut CommandContext,
        app_handle: Option<&tauri::AppHandle>,
    ) -> anyhow::Result<serde_json::Value> {
        let params: CreateBranchParameters = serde_json::from_value(parameters)
            .map_err(|e| anyhow::anyhow!("Failed to parse input parameters: {}", e))?;

        let stack = create_branch(ctx, app_handle, params).to_json("create branch");
        Ok(stack)
    }
}

pub fn create_branch(
    ctx: &mut CommandContext,
    app_handle: Option<&tauri::AppHandle>,
    params: CreateBranchParameters,
) -> Result<StackEntry, anyhow::Error> {
    let mut guard = ctx.project().exclusive_worktree_access();
    let perm = guard.write_permission();
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());

    let name = params.branch_name;
    let description = params.branch_description;

    let branch = gitbutler_branch::BranchCreateRequest {
        name: Some(name.clone()),
        ..Default::default()
    };

    let stack_entry = gitbutler_branch_actions::create_virtual_branch(ctx, &branch, perm)?;

    // Update the branch description.
    let mut stack = vb_state.get_stack(stack_entry.id)?;
    stack.update_branch(
        ctx,
        name,
        &PatchReferenceUpdate {
            description: Some(Some(description)),
            ..Default::default()
        },
    )?;

    // If there's an app handle provided, emit an event to update the stack details in the UI.
    if let Some(app_handle) = app_handle {
        let project_id = ctx.project().id;
        app_handle.emit_stack_update(project_id, stack.id);
    }

    Ok(stack_entry)
}

fn ref_metadata_toml(project: &Project) -> anyhow::Result<VirtualBranchesTomlMetadata> {
    VirtualBranchesTomlMetadata::from_path(project.gb_dir().join("virtual_branches.toml"))
}
