use std::sync::Arc;

use but_graph::VirtualBranchesTomlMetadata;
use gitbutler_command_context::CommandContext;
use gitbutler_oplog::{OplogExt, SnapshotExt};
use schemars::{JsonSchema, schema_for};

use crate::tool::{Tool, Toolset};

/// Creates a toolset for workspace-related operations.
pub fn commit_toolset(ctx: &mut CommandContext) -> anyhow::Result<Toolset> {
    let mut toolset = Toolset::new(ctx);

    toolset.register_tool(Commit);
    toolset.register_tool(CreateBranch);

    Ok(toolset)
}

pub struct Commit;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommitParameters {
    /// The commit message to use.
    #[schemars(description = "The commit message to use")]
    pub message: String,
    /// The branch name to commit to.
    #[schemars(description = "The branch name to commit to")]
    pub branch_name: String,
    /// The list of files to commit.
    #[schemars(
        description = "The list of files to commit. This should be the paths of the files relative to the project root."
    )]
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
        "Commit the file changes into a branch".to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        let schema = schema_for!(CommitParameters);
        serde_json::to_value(&schema).unwrap_or_default()
    }

    fn call(
        self: Arc<Self>,
        parameters: serde_json::Value,
        ctx: &mut CommandContext,
    ) -> anyhow::Result<serde_json::Value> {
        let params: CommitParameters = serde_json::from_value(parameters)
            .map_err(|e| anyhow::anyhow!("Failed to parse input parameters: {}", e))?;

        create_commit(ctx, params)
    }
}

pub fn create_commit(
    ctx: &mut CommandContext,
    params: CommitParameters,
) -> Result<serde_json::Value, anyhow::Error> {
    let repo = ctx.gix_repo()?;
    let mut guard = ctx.project().exclusive_worktree_access();

    let worktree = but_core::diff::worktree_changes(&repo)?;
    let file_changes: Vec<but_workspace::DiffSpec> = worktree
        .changes
        .iter()
        .filter(|change| params.files.contains(&change.path.to_string()))
        .map(Into::into)
        .collect::<Vec<_>>();

    let stacks = stacks(ctx, &repo)?;

    let stack_id = stacks
        .iter()
        .find(|s| s.heads.iter().any(|h| h.name == params.branch_name))
        .map(|s| s.id)
        .ok_or_else(|| anyhow::anyhow!("Branch '{}' not found", params.branch_name))?;

    let snapshot_tree = ctx.prepare_snapshot(guard.read_permission());

    let outcome = but_workspace::commit_engine::create_commit_simple(
        ctx,
        stack_id,
        None,
        file_changes,
        params.message.clone(),
        params.branch_name,
        guard.write_permission(),
    );

    let _ = snapshot_tree.and_then(|snapshot_tree| {
        ctx.snapshot_commit_creation(
            snapshot_tree,
            outcome.as_ref().err(),
            params.message,
            None,
            guard.write_permission(),
        )
    });

    let outcome: but_workspace::commit_engine::ui::CreateCommitOutcome = outcome?.into();
    Ok(serde_json::to_value(outcome)?)
}

fn stacks(
    ctx: &CommandContext,
    repo: &gix::Repository,
) -> anyhow::Result<Vec<but_workspace::ui::StackEntry>> {
    let meta = VirtualBranchesTomlMetadata::from_path(
        ctx.project().gb_dir().join("virtual_branches.toml"),
    )?;
    but_workspace::stacks_v3(repo, &meta, but_workspace::StacksFilter::InWorkspace)
}

pub struct CreateBranch;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateBranchParameters {
    /// The name of the new branch to create.
    #[schemars(description = "The name of the new branch to create")]
    pub branch_name: String,
    /// The description of the new branch.
    #[schemars(
        description = "The description of the new branch. This should be a short summary of the branch's purpose."
    )]
    pub description: String,
}

impl Tool for CreateBranch {
    fn name(&self) -> String {
        "createBranch".to_string()
    }

    fn description(&self) -> String {
        "Create a new branch in the workspace".to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        let schema = schema_for!(CreateBranchParameters);
        serde_json::to_value(&schema).unwrap_or_default()
    }

    fn call(
        self: Arc<Self>,
        parameters: serde_json::Value,
        ctx: &mut CommandContext,
    ) -> anyhow::Result<serde_json::Value> {
        let params: CreateBranchParameters = serde_json::from_value(parameters)
            .map_err(|e| anyhow::anyhow!("Failed to parse input parameters: {}", e))?;

        create_branch(ctx, params)
    }
}

pub fn create_branch(
    ctx: &mut CommandContext,
    params: CreateBranchParameters,
) -> Result<serde_json::Value, anyhow::Error> {
    let mut guard = ctx.project().exclusive_worktree_access();
    let perm = guard.write_permission();

    let branch = gitbutler_branch::BranchCreateRequest {
        name: Some(params.branch_name),
        ..Default::default()
    };

    let stack = gitbutler_branch_actions::create_virtual_branch(ctx, &branch, perm)?;

    let outcome = serde_json::to_value(stack)?;
    Ok(outcome)
}
