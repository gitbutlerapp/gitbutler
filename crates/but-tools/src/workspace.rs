use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    sync::Arc,
};

use anyhow::Context as _;
use bstr::BString;
use but_core::{TreeChange, UnifiedPatch, ref_metadata::StackId};
use but_ctx::Context;
use but_meta::VirtualBranchesTomlMetadata;
use but_oxidize::{ObjectIdExt, OidExt, git2_to_gix_object_id};
use but_workspace::legacy::{CommmitSplitOutcome, ui::StackEntryNoOpt};
use gitbutler_branch_actions::{BranchManagerExt, update_workspace_commit};
use gitbutler_oplog::{
    OplogExt, SnapshotExt,
    entry::{OperationKind, SnapshotDetails},
};
use gitbutler_project::Project;
use gitbutler_reference::{LocalRefname, Refname};
use gitbutler_stack::{PatchReferenceUpdate, VirtualBranchesHandle};
use schemars::{JsonSchema, schema_for};

use crate::{
    emit::{Emittable, Emitter, StackUpdate},
    tool::{Tool, ToolResult, Toolset, WorkspaceToolset, error_to_json, result_to_json},
};

/// Creates a toolset for any kind of workspace operations.
pub fn workspace_toolset(
    ctx: &mut Context,
    emitter: std::sync::Arc<crate::emit::Emitter>,
    message_id: String,
) -> WorkspaceToolset<'_> {
    let mut toolset = WorkspaceToolset::new(ctx, emitter, Some(message_id));

    toolset.register_tool(Commit);
    toolset.register_tool(CreateBranch);
    toolset.register_tool(Amend);
    toolset.register_tool(SquashCommits);
    toolset.register_tool(GetProjectStatus);
    toolset.register_tool(MoveFileChanges);
    toolset.register_tool(GetCommitDetails);
    toolset.register_tool(GetBranchChanges);
    toolset.register_tool(SplitBranch);
    toolset.register_tool(SplitCommit);

    toolset
}

/// Creates a toolset for workspace-related operations.
pub fn commit_toolset(
    ctx: &mut Context,
    emitter: std::sync::Arc<crate::emit::Emitter>,
) -> WorkspaceToolset<'_> {
    let mut toolset = WorkspaceToolset::new(ctx, emitter, None);

    toolset.register_tool(Commit);
    toolset.register_tool(CreateBranch);

    toolset
}

/// Creates a toolset for amend operations.
pub fn amend_toolset(
    ctx: &mut Context,
    emitter: std::sync::Arc<crate::emit::Emitter>,
) -> WorkspaceToolset<'_> {
    let mut toolset = WorkspaceToolset::new(ctx, emitter, None);

    toolset.register_tool(Amend);
    toolset.register_tool(GetProjectStatus);

    toolset
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
        Don't exceed 50 characters in length.
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
        "
        .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        let schema = schema_for!(CommitParameters);
        serde_json::to_value(&schema).unwrap_or_default()
    }

    fn call(
        self: Arc<Self>,
        parameters: serde_json::Value,
        ctx: &mut Context,
        emitter: Arc<Emitter>,
        _: &mut HashMap<gix::ObjectId, gix::ObjectId>,
    ) -> anyhow::Result<serde_json::Value> {
        let params: CommitParameters = serde_json::from_value(parameters)
            .map_err(|e| anyhow::anyhow!("Failed to parse input parameters: {}", e))?;

        let value = create_commit(ctx, emitter, params).to_json("create_commit");
        Ok(value)
    }
}

pub fn create_commit(
    ctx: &mut Context,
    emitter: Arc<Emitter>,
    params: CommitParameters,
) -> Result<but_workspace::commit_engine::ui::CreateCommitOutcome, anyhow::Error> {
    let repo = ctx.repo.get()?;
    let mut guard = ctx.exclusive_worktree_access();
    let worktree = but_core::diff::worktree_changes(&repo)?;
    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());

    let file_changes: Vec<but_core::DiffSpec> = worktree
        .changes
        .iter()
        .filter(|change| params.files.contains(&change.path.to_string()))
        .map(Into::into)
        .collect::<Vec<_>>();

    let stacks = stacks(ctx)?;

    let stack_id = stacks
        .iter()
        .find_map(|s| {
            let found = s.heads.iter().any(|h| h.name == params.branch_name);
            if found { s.id } else { None }
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

    let outcome = but_workspace::legacy::commit_engine::create_commit_simple(
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
    let project_id = ctx.legacy_project.id;
    let stack_update = StackUpdate {
        project_id,
        stack_id,
    };

    let (name, payload) = stack_update.emittable();
    (emitter)(&name, payload);

    let outcome: but_workspace::commit_engine::ui::CreateCommitOutcome = outcome?.into();
    Ok(outcome)
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
        ctx: &mut Context,
        emitter: Arc<Emitter>,
        _: &mut HashMap<gix::ObjectId, gix::ObjectId>,
    ) -> anyhow::Result<serde_json::Value> {
        let params: CreateBranchParameters = serde_json::from_value(parameters)
            .map_err(|e| anyhow::anyhow!("Failed to parse input parameters: {}", e))?;

        let stack = create_branch(ctx, emitter, params).to_json("create branch");
        Ok(stack)
    }
}

pub fn create_branch(
    ctx: &mut Context,
    emitter: Arc<Emitter>,
    params: CreateBranchParameters,
) -> Result<StackEntryNoOpt, anyhow::Error> {
    let mut guard = ctx.exclusive_worktree_access();
    let perm = guard.write_permission();
    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());

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

    // Emit an event to update the stack details in the UI.
    let project_id = ctx.legacy_project.id;
    let stack_update = StackUpdate {
        project_id,
        stack_id: stack_entry.id,
    };
    let (name, payload) = stack_update.emittable();
    (emitter)(&name, payload);

    Ok(stack_entry)
}

pub struct Amend;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AmendParameters {
    /// The commit id to amend.
    #[schemars(description = "
    <description>
        The commit id of the commit to amend.
        This should be the id of the commit you want to modify.
    </description>

    <important_notes>
        The commit id should refer to a commit on the specified branch.
    </important_notes>
    ")]
    pub commit_id: String,
    /// The new commit title.
    #[schemars(description = "
    <description>
        The new commit message title.
        This is only a short summary of the commit.
    </description>

    <important_notes>
        The commit message title should be concise and descriptive.
        It is typically a single line that summarizes the changes made in the commit.
        For example: 'Fix issue with user login' or 'Update README with installation instructions'.
        Don't exceed 50 characters in length.
    </important_notes>
    ")]
    pub message_title: String,
    /// The new commit description.
    #[schemars(description = "
    <description>
        The new commit message body.
        This is a more detailed description of the changes made in the commit.
    </description>

    <important_notes>
        This should be an update of the existing commit message body in order to accommodate the changes amended into it.
        If the description already matches the changes, you can pass in the same description.
        The commit message body should provide context and details about the changes made.
        It should span multiple lines if necessary.
        A good description focuses on describing the 'what' of the changes.
        Don't make assumption about the 'why', only describe the changes in the context of the branch (and other commits if any).
    </important_notes>
    ")]
    pub message_body: String,
    /// The id of the stack to amend the commit on.
    #[schemars(description = "
    <description>
        This is the Id of the stack that contains the commit to amend.
    </description>

    <important_notes>
        The ID should refer to a stack in the workspace.
    </important_notes>
    ")]
    pub stack_id: String,
    /// The list of files to include in the amended commit.
    #[schemars(description = "
        <description>
            The list of file paths to include in the amended commit.
        </description>

        <important_notes>
            The file paths should be relative to the workspace root.
            Leave this empty if you only want to edit the commit message.
        </important_notes>
        ")]
    pub files: Vec<String>,
}

impl Tool for Amend {
    fn name(&self) -> String {
        "amend".to_string()
    }

    fn description(&self) -> String {
        "
        <description>
            Amend an existing commit on a branch in the workspace.
        </description>

        <important_notes>
            This tool allows you to amend a specific commit on a branch in the workspace.
            You can specify the new commit message, target branch name, commit id, and a list of file paths to include in the amended commit.
            Use this tool if:
            - You want to add uncommitted changes to an existing commit.
            - You want to update the commit message of an existing commit.
        </important_notes>
        ".to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        let schema = schema_for!(AmendParameters);
        serde_json::to_value(&schema).unwrap_or_default()
    }

    fn call(
        self: Arc<Self>,
        parameters: serde_json::Value,
        ctx: &mut Context,
        emitter: Arc<Emitter>,
        commit_mapping: &mut HashMap<gix::ObjectId, gix::ObjectId>,
    ) -> anyhow::Result<serde_json::Value> {
        let params: AmendParameters = serde_json::from_value(parameters)
            .map_err(|e| anyhow::anyhow!("Failed to parse input parameters: {}", e))?;

        let value = amend_commit(ctx, emitter, params, commit_mapping).to_json("amend_commit");
        Ok(value)
    }
}

pub fn amend_commit(
    ctx: &mut Context,
    emitter: Arc<Emitter>,
    params: AmendParameters,
    commit_mapping: &mut HashMap<gix::ObjectId, gix::ObjectId>,
) -> Result<but_workspace::commit_engine::ui::CreateCommitOutcome, anyhow::Error> {
    let outcome = amend_commit_inner(ctx, emitter, params, Some(commit_mapping))?;

    // Update the commit mapping with the new commit id.
    if let Some(rebase_output) = outcome.rebase_output.clone() {
        for (_, old_commit_id, new_commit_id) in rebase_output.commit_mapping.iter() {
            commit_mapping.insert(*old_commit_id, *new_commit_id);
        }
    }

    Ok(outcome.into())
}

pub fn amend_commit_inner(
    ctx: &mut Context,
    emitter: Arc<Emitter>,
    params: AmendParameters,
    commit_mapping: Option<&HashMap<gix::ObjectId, gix::ObjectId>>,
) -> anyhow::Result<but_workspace::commit_engine::CreateCommitOutcome> {
    let repo = ctx.repo.get()?;
    let settings = ctx.settings();
    let mut guard = ctx.exclusive_worktree_access();
    let worktree = but_core::diff::worktree_changes(&repo)?;

    let file_changes: Vec<but_core::DiffSpec> = worktree
        .changes
        .iter()
        .filter(|change| params.files.contains(&change.path.to_string()))
        .map(Into::into)
        .collect::<Vec<_>>();

    let message = format!(
        "{}\n\n{}",
        params.message_title.trim(),
        params.message_body.trim()
    );

    let stack_id = StackId::from_str(&params.stack_id)?;
    let commit_id = gix::ObjectId::from_str(&params.commit_id)?;
    let commit_id = if let Some(commit_mapping) = commit_mapping {
        find_the_right_commit_id(commit_id, commit_mapping)
    } else {
        commit_id
    };

    let outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs_with_project(
        &repo,
        &ctx.project_data_dir(),
        Some(stack_id),
        but_workspace::commit_engine::Destination::AmendCommit {
            commit_id,
            new_message: Some(message),
        },
        file_changes,
        settings.context_lines,
        guard.write_permission(),
    );

    // Emit an event to update the stack details in the UI.
    let project_id = ctx.legacy_project.id;
    let stack_update = StackUpdate {
        project_id,
        stack_id,
    };
    let (name, payload) = stack_update.emittable();
    (emitter)(&name, payload);

    outcome
}

pub struct GetProjectStatus;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetProjectStatusParameters {
    /// Optional filter for file changes.
    #[schemars(description = "
    <description>
        Optional filter for file changes.
        This can be used to limit the file changes returned in the project status.
    </description>

    <important_notes>
        The filter should be a list of file paths to include in the project status.
        If not provided, all file changes will be included.
    </important_notes>
    ")]
    pub filter_changes: Option<Vec<String>>,
}

impl Tool for GetProjectStatus {
    fn name(&self) -> String {
        "get_project_status".to_string()
    }

    fn description(&self) -> String {
        "
        <description>
            Get the current status of the project, including stacks and file changes.
        </description>
        "
        .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        let schema = schema_for!(GetProjectStatusParameters);
        serde_json::to_value(&schema).unwrap_or_default()
    }

    fn call(
        self: Arc<Self>,
        parameters: serde_json::Value,
        ctx: &mut Context,
        _emitter: Arc<Emitter>,
        _commit_mapping: &mut HashMap<gix::ObjectId, gix::ObjectId>,
    ) -> anyhow::Result<serde_json::Value> {
        let params: GetProjectStatusParameters = serde_json::from_value(parameters)
            .map_err(|e| anyhow::anyhow!("Failed to parse input parameters: {}", e))?;

        let paths = params
            .filter_changes
            .map(|f| f.into_iter().map(BString::from).collect::<Vec<BString>>());

        let value = get_project_status(ctx, paths).to_json("get_project_status");
        Ok(value)
    }
}

pub struct CreateBlankCommit;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateBlankCommitParameters {
    /// The commit message title.
    #[schemars(description = "
    <description>
        The commit message title.
        This is only a short summary of the commit.
    </description>

    <important_notes>
        The commit message title should be concise and descriptive.
        It is typically a single line that summarizes the changes made in the commit.
        For example: 'Fix issue with user login' or 'Update README with installation instructions'.
        Don't exceed 50 characters in length.
    </important_notes>
    ")]
    pub message_title: String,
    /// The commit message body.
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
    /// The stack id to create the blank commit on.
    #[schemars(description = "
    <description>
        The stack id where the blank commit should be created.
    </description>

    <important_notes>
        The stack id should refer to an existing stack in the workspace.
    </important_notes>
    ")]
    pub stack_id: String,
    /// The ID of the commit to insert the blank commit on top of.
    #[schemars(description = "
    <description>
        The ID of the commit to insert the blank commit on top of.
    </description>

    <important_notes>
        This should be the ID of an existing commit in the stack.
    </important_notes>
    ")]
    pub parent_id: String,
}

impl Tool for CreateBlankCommit {
    fn name(&self) -> String {
        "create_blank_commit".to_string()
    }

    fn description(&self) -> String {
        "
        <description>
            Create a blank commit on a specific stack in the workspace.
        </description>

        <important_notes>
            Use this tool when you want to create a new commit without any file changes and only want to prepare a branch structure.
        </important_notes>
        "
        .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        let schema = schema_for!(CreateBlankCommitParameters);
        serde_json::to_value(&schema).unwrap_or_default()
    }

    fn call(
        self: Arc<Self>,
        parameters: serde_json::Value,
        ctx: &mut Context,
        emitter: Arc<Emitter>,
        commit_mapping: &mut HashMap<gix::ObjectId, gix::ObjectId>,
    ) -> anyhow::Result<serde_json::Value> {
        let params: CreateBlankCommitParameters = serde_json::from_value(parameters)
            .map_err(|e| anyhow::anyhow!("Failed to parse input parameters: {}", e))?;

        let value = create_blank_commit(ctx, emitter, params, commit_mapping)
            .to_json("create_blank_commit");
        Ok(value)
    }
}

pub fn create_blank_commit(
    ctx: &mut Context,
    emitter: Arc<Emitter>,
    params: CreateBlankCommitParameters,
    commit_mapping: &mut HashMap<gix::ObjectId, gix::ObjectId>,
) -> Result<gix::ObjectId, anyhow::Error> {
    let stack_id = StackId::from_str(&params.stack_id)?;
    let commit_oid = gix::ObjectId::from_str(&params.parent_id)
        .map(|id| find_the_right_commit_id(id, commit_mapping))?;
    let commit_oid = commit_oid.to_git2();

    let message = format!(
        "{}\n\n{}",
        params.message_title.trim(),
        params.message_body.trim()
    );

    let (new_commit, outcome) = gitbutler_branch_actions::insert_blank_commit(
        ctx,
        stack_id,
        commit_oid,
        -1,
        Some(&message),
    )?;

    // Emit an event to update the stack details in the UI.
    let project_id = ctx.legacy_project.id;
    let stack_update = StackUpdate {
        project_id,
        stack_id,
    };
    let (name, payload) = stack_update.emittable();
    (emitter)(&name, payload);

    // Update the commit mapping with the new commit id.
    for (old_commit_id, new_commit_id) in outcome.iter() {
        commit_mapping.insert(*old_commit_id, *new_commit_id);
    }

    Ok(new_commit)
}

pub struct MoveFileChanges;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MoveFileChangesParameters {
    /// The commit id to move file changes from.
    #[schemars(description = "
    <description>
        The commit id of the commit to move file changes from.
    </description>

    <important_notes>
        The commit id should refer to a commit on the specified source stack.
    </important_notes>
    ")]
    pub source_commit_id: String,

    /// The stack id of the source commit.
    #[schemars(description = "
    <description>
        The stack id containing the source commit.
    </description>

    <important_notes>
        The stack id should refer to a stack in the workspace.
    </important_notes>
    ")]
    pub source_stack_id: String,

    /// The commit id to move file changes to.
    #[schemars(description = "
    <description>
        The commit id of the commit to move file changes to.
    </description>

    <important_notes>
        The commit id should refer to a commit on the specified destination stack.
    </important_notes>
    ")]
    pub destination_commit_id: String,

    /// The stack id of the destination commit.
    #[schemars(description = "
    <description>
        The stack id containing the destination commit.
    </description>

    <important_notes>
        The stack id should refer to a stack in the workspace.
    </important_notes>
    ")]
    pub destination_stack_id: String,

    /// The list of files to move.
    #[schemars(description = "
    <description>
        The list of file paths to move from the source commit to the destination commit.
    </description>

    <important_notes>
        The file paths should be relative to the workspace root.
        The file paths should be contained in the source commit.
        Only the specified files will be moved.
    </important_notes>
    ")]
    pub files: Vec<String>,
}

impl Tool for MoveFileChanges {
    fn name(&self) -> String {
        "move_file_changes".to_string()
    }

    fn description(&self) -> String {
        "
        <description>
            Move file changes from one commit to another in the workspace.
        </description>

        <important_notes>
            Use this tool when you want to move file changes from one commit to another.
            This is useful when you want to split a commit into more parts.
        </important_notes>
        "
        .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        let schema = schema_for!(MoveFileChangesParameters);
        serde_json::to_value(&schema).unwrap_or_default()
    }

    fn call(
        self: Arc<Self>,
        parameters: serde_json::Value,
        ctx: &mut Context,
        emitter: Arc<Emitter>,
        commit_mapping: &mut HashMap<gix::ObjectId, gix::ObjectId>,
    ) -> anyhow::Result<serde_json::Value> {
        let params: MoveFileChangesParameters = serde_json::from_value(parameters)
            .map_err(|e| anyhow::anyhow!("Failed to parse input parameters: {}", e))?;

        match move_file_changes(ctx, emitter, params, commit_mapping) {
            Ok(_) => Ok("Success".into()),
            Err(e) => Ok(error_to_json(&e, "move_file_changes")),
        }
    }
}

pub fn move_file_changes(
    ctx: &mut Context,
    emitter: Arc<Emitter>,
    params: MoveFileChangesParameters,
    commit_mapping: &mut HashMap<gix::ObjectId, gix::ObjectId>,
) -> Result<Vec<(gix::ObjectId, gix::ObjectId)>, anyhow::Error> {
    let source_commit_id = gix::ObjectId::from_str(&params.source_commit_id)
        .map(|id| find_the_right_commit_id(id, commit_mapping))?;
    let source_stack_id = StackId::from_str(&params.source_stack_id)?;
    let destination_commit_id = gix::ObjectId::from_str(&params.destination_commit_id)
        .map(|id| find_the_right_commit_id(id, commit_mapping))?;
    let destination_stack_id = StackId::from_str(&params.destination_stack_id)?;

    let changes = params
        .files
        .iter()
        .map(|f| but_core::DiffSpec {
            path: BString::from(f.as_str()),
            previous_path: None,
            hunk_headers: vec![],
        })
        .collect::<Vec<_>>();

    let result = but_workspace::legacy::move_changes_between_commits(
        ctx,
        source_stack_id,
        source_commit_id,
        destination_stack_id,
        destination_commit_id,
        changes,
        ctx.settings().context_lines,
    )?;

    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    gitbutler_branch_actions::update_workspace_commit(&vb_state, ctx, false)?;

    // Emit an event to update the stack details in the UI.
    let project_id = ctx.legacy_project.id;
    let stack_update = StackUpdate {
        project_id,
        stack_id: source_stack_id,
    };
    let (name, payload) = stack_update.emittable();
    (emitter)(&name, payload);

    // Emit an event to update the destination stack details in the UI.
    let destination_stack_update = StackUpdate {
        project_id,
        stack_id: destination_stack_id,
    };
    let (name, payload) = destination_stack_update.emittable();
    (emitter)(&name, payload);

    // Update the commit mapping with the new commit ids.
    for (old_commit_id, new_commit_id) in result.replaced_commits.clone().iter() {
        commit_mapping.insert(*old_commit_id, *new_commit_id);
    }

    Ok(result.replaced_commits)
}

pub struct GetCommitDetails;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetCommitDetailsParameters {
    /// The commit id to get details for.
    #[schemars(description = "
    <description>
        The commit id of the commit to get details for.
    </description>

    <important_notes>
        The commit id should refer to a commit in the workspace.
    </important_notes>
    ")]
    pub commit_id: String,
}

impl Tool for GetCommitDetails {
    fn name(&self) -> String {
        "get_commit_details".to_string()
    }

    fn description(&self) -> String {
        "
        <description>
            Get details of a specific commit in the workspace.
        </description>

        <important_notes>
            This tool allows you to retrieve detailed information about a specific commit in the workspace.
            Use this tool to get the information about the files changed in the commit.
            You'll want to use this tool before moving file changes from one commit to another.
        </important_notes>
        "
        .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        let schema = schema_for!(GetCommitDetailsParameters);
        serde_json::to_value(&schema).unwrap_or_default()
    }

    fn call(
        self: Arc<Self>,
        parameters: serde_json::Value,
        ctx: &mut Context,
        _emitter: Arc<Emitter>,
        commit_mapping: &mut HashMap<gix::ObjectId, gix::ObjectId>,
    ) -> anyhow::Result<serde_json::Value> {
        let params: GetCommitDetailsParameters = serde_json::from_value(parameters)
            .map_err(|e| anyhow::anyhow!("Failed to parse input parameters: {}", e))?;

        let file_changes = commit_details(ctx, params, commit_mapping).to_json("commit_details");

        Ok(file_changes)
    }
}

pub fn commit_details(
    ctx: &mut Context,
    params: GetCommitDetailsParameters,
    commit_mapping: &HashMap<gix::ObjectId, gix::ObjectId>,
) -> anyhow::Result<Vec<FileChange>> {
    let repo = ctx.repo.get()?;
    let commit_id = gix::ObjectId::from_str(&params.commit_id)
        .map(|id| find_the_right_commit_id(id, commit_mapping))?;

    let changes =
        but_core::diff::ui::commit_changes_with_line_stats_by_worktree_dir(&repo, commit_id)?;
    let changes: Vec<but_core::TreeChange> = changes
        .changes
        .into_iter()
        .map(|change| change.into())
        .collect();

    let diff = unified_diff_for_changes(&repo, changes, ctx.settings().context_lines)?;
    let file_changes = get_file_changes(&diff, vec![]);

    Ok(file_changes)
}

pub struct GetBranchChanges;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetBranchChangesParameters {
    /// The branch name to get changes for.
    #[schemars(description = "
    <description>
        The name of the branch to get changes for.
    </description>

    <important_notes>
        The branch name should be a valid Git branch name present in the workspace.
    </important_notes>
    ")]
    pub branch_name: String,
}

impl Tool for GetBranchChanges {
    fn name(&self) -> String {
        "get_branch_changes".to_string()
    }

    fn description(&self) -> String {
        "
        <description>
            Get the list of file changes for a specific branch in the workspace.
        </description>

        <important_notes>
            This tool allows you to retrieve a list of file paths that have been changed on a specific branch.
            Call this tool before splitting a branch.
            Use this to inspect what files have been changed on a branch.
        </important_notes>
        "
        .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        let schema = schema_for!(GetBranchChangesParameters);
        serde_json::to_value(&schema).unwrap_or_default()
    }

    fn call(
        self: Arc<Self>,
        parameters: serde_json::Value,
        ctx: &mut Context,
        _emitter: Arc<Emitter>,
        _commit_mapping: &mut HashMap<gix::ObjectId, gix::ObjectId>,
    ) -> anyhow::Result<serde_json::Value> {
        let params: GetBranchChangesParameters = serde_json::from_value(parameters)
            .map_err(|e| anyhow::anyhow!("Failed to parse input parameters: {}", e))?;

        let file_changes = branch_changes(ctx, params).to_json("get_branch_changes");

        Ok(file_changes)
    }
}

pub fn branch_changes(
    ctx: &mut Context,
    params: GetBranchChangesParameters,
) -> anyhow::Result<Vec<FileChangeSimple>> {
    let stacks = stacks(ctx)?;
    let stack_id = stacks
        .iter()
        .find_map(|s| {
            let found = s.heads.iter().any(|h| h.name == params.branch_name);
            if found { s.id } else { None }
        })
        .ok_or_else(|| {
            anyhow::anyhow!("Branch '{}' not found in the workspace", params.branch_name)
        })?;

    let changes = changes_in_branch_inner(ctx, params.branch_name, Some(stack_id))?;
    let file_changes = changes
        .changes
        .into_iter()
        .map(|change| change.into())
        .collect();

    Ok(file_changes)
}

pub struct SquashCommits;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SquashCommitsParameters {
    /// The stack id containing the commits to squash.
    #[schemars(description = "
        <description>
            The stack id where the commits to squash are located.
        </description>

        <important_notes>
            The stack id should refer to an existing stack in the workspace.
        </important_notes>
        ")]
    pub stack_id: String,
    /// The list of commit ids to squash (in order).
    #[schemars(description = "
        <description>
            The list of commit ids to squash, in the order they should be squashed.
        </description>

        <important_notes>
            The commit ids should refer to commits in the specified stack.
            The commits should be in the order they were created, with the oldest commit first.
            All commit should be part of the same stack specified by `stack_id`.
            This should NOT include the commit to squash into.
        </important_notes>
        ")]
    pub source_commit_ids: Vec<String>,
    /// The commit to squash into.
    #[schemars(description = "
        <description>
            The commit id to squash the other commits into.
        </description>

        <important_notes>
            This should be the id of an existing commit in the stack.
            The commit should be present in the stack specified by `stack_id`.
        </important_notes>
        ")]
    pub destination_commit_id: String,
    /// The new commit title.
    #[schemars(description = "
        <description>
            The new commit message title for the squashed commit.
        </description>

        <important_notes>
            The commit message title should be concise and descriptive.
            Don't exceed 50 characters in length.
        </important_notes>
        ")]
    pub message_title: String,
    /// The new commit description.
    #[schemars(description = "
        <description>
            The new commit message body for the squashed commit.
        </description>

        <important_notes>
            The commit message body should provide context and details about the changes made.
            It should span multiple lines if necessary.
        </important_notes>
        ")]
    pub message_body: String,
}

impl Tool for SquashCommits {
    fn name(&self) -> String {
        "squash_commits".to_string()
    }

    fn description(&self) -> String {
        "
        <description>
            Squash multiple commits in a stack into a single commit.
        </description>

        <important_notes>
            This tool allows you to squash a sequence of commits in a stack into a single commit with a new message.
            Use this tool to clean up commit history before merging or sharing.
            Always squash the commits down, meaning newer commits into their parents.
            Remember that the commits listed in the project status are in reverse order, so the first commit in the array is the newest one.
        </important_notes>
        ".to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        let schema = schema_for!(SquashCommitsParameters);
        serde_json::to_value(&schema).unwrap_or_default()
    }

    fn call(
        self: Arc<Self>,
        parameters: serde_json::Value,
        ctx: &mut Context,
        emitter: Arc<Emitter>,
        commit_mapping: &mut HashMap<gix::ObjectId, gix::ObjectId>,
    ) -> anyhow::Result<serde_json::Value> {
        let params: SquashCommitsParameters = serde_json::from_value(parameters)
            .map_err(|e| anyhow::anyhow!("Failed to parse input parameters: {}", e))?;

        let value = squash_commits(ctx, emitter, params, commit_mapping).to_json("squash_commits");

        Ok(value)
    }
}

pub fn squash_commits(
    ctx: &mut Context,
    emitter: Arc<Emitter>,
    params: SquashCommitsParameters,
    commit_mapping: &mut HashMap<gix::ObjectId, gix::ObjectId>,
) -> Result<gix::ObjectId, anyhow::Error> {
    let destination_id = gix::ObjectId::from_str(&params.destination_commit_id)
        .map(|id| find_the_right_commit_id(id, commit_mapping))?
        .to_git2();
    let source_ids = params
        .source_commit_ids
        .iter()
        .map(|id| {
            gix::ObjectId::from_str(id).map(|oid| {
                let id = find_the_right_commit_id(oid, commit_mapping);
                id.to_git2()
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let stack_id = StackId::from_str(&params.stack_id)?;

    let message = format!(
        "{}\n\n{}",
        params.message_title.trim(),
        params.message_body.trim()
    );

    let squashed_commit =
        gitbutler_branch_actions::squash_commits(ctx, stack_id, source_ids, destination_id)?;

    let new_commit_id = gitbutler_branch_actions::update_commit_message(
        ctx,
        stack_id,
        squashed_commit,
        message.as_str(),
    )?;

    let project_id = ctx.legacy_project.id;
    let stack_update = StackUpdate {
        project_id,
        stack_id,
    };
    let (name, payload) = stack_update.emittable();
    (emitter)(&name, payload);

    // Update the commit mapping with the new commit id.
    commit_mapping.insert(destination_id.to_gix(), new_commit_id.to_gix());

    Ok(git2_to_gix_object_id(new_commit_id))
}

pub struct SplitBranch;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SplitBranchParameters {
    /// The name of the branch to split from.
    #[schemars(description = "
    <description>
        The name of the branch to split from.
    </description>

    <important_notes>
        This should be the name of an existing branch in the workspace.
    </important_notes>
    ")]
    pub source_branch_name: String,

    /// The name of the new branch to create with the split off files.
    #[schemars(description = "
    <description>
        The name of the new branch to create with the split-off files.
    </description>

    <important_notes>
        The branch name should be a valid Git branch name.
        It should not contain spaces or special characters.
        Keep it to maximum 5 words, and use hyphens to separate words.
        Don't use slashes or other special characters.
    </important_notes>
    ")]
    pub new_branch_name: String,

    /// The list of file paths to split off into the new branch.
    #[schemars(description = "
    <description>
        The list of file paths to split off into the new branch.
    </description>

    <important_notes>
        The file paths should be relative to the workspace root.
        Only the specified files will be moved to the new branch.
    </important_notes>
    ")]
    pub files_to_split_off: Vec<String>,
}

impl Tool for SplitBranch {
    fn name(&self) -> String {
        "split_branch".to_string()
    }

    fn description(&self) -> String {
        "
        <description>
            Split off selected files from an existing branch into a new branch.
        </description>

        <important_notes>
            This tool allows you to move a set of files from one branch to a new branch, effectively splitting the branch.
            This will copy the same commit history from the source branch to the new branch, so probably you'll want to amend the commit messages afterwards.
            Use this when you want to organize changes into separate branches.
        </important_notes>
        ".to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        let schema = schema_for!(SplitBranchParameters);
        serde_json::to_value(&schema).unwrap_or_default()
    }

    fn call(
        self: Arc<Self>,
        parameters: serde_json::Value,
        ctx: &mut Context,
        emitter: Arc<Emitter>,
        commit_mapping: &mut HashMap<gix::ObjectId, gix::ObjectId>,
    ) -> anyhow::Result<serde_json::Value> {
        let params: SplitBranchParameters = serde_json::from_value(parameters)
            .map_err(|e| anyhow::anyhow!("Failed to parse input parameters: {}", e))?;

        Ok(split_branch(ctx, emitter, params, commit_mapping).to_json("split_branch"))
    }
}

pub fn split_branch(
    ctx: &mut Context,
    emitter: Arc<Emitter>,
    params: SplitBranchParameters,
    commit_mapping: &mut HashMap<gix::ObjectId, gix::ObjectId>,
) -> Result<StackId, anyhow::Error> {
    let mut guard = ctx.exclusive_worktree_access();
    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());

    let stacks = stacks(ctx)?;
    let source_stack_id = stacks
        .iter()
        .find(|s| s.heads.iter().any(|b| b.name == params.source_branch_name))
        .and_then(|s| s.id)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Source branch '{}' not found in the workspace",
                params.source_branch_name
            )
        })?;

    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::SplitBranch),
        guard.write_permission(),
    );

    let (_, move_result) = but_workspace::legacy::split_branch(
        ctx,
        source_stack_id,
        params.source_branch_name,
        params.new_branch_name.clone(),
        &params.files_to_split_off,
        ctx.settings().context_lines,
    )?;

    update_workspace_commit(&vb_state, ctx, false)?;

    let refname = Refname::Local(LocalRefname::new(&params.new_branch_name, None));
    let branch_manager = ctx.branch_manager();

    let (stack_id, _, _) = branch_manager.create_virtual_branch_from_branch(
        &refname,
        None,
        None,
        guard.write_permission(),
    )?;

    // Emit an event to update the stack details in the UI.
    let project_id = ctx.legacy_project.id;
    let stack_update = StackUpdate {
        project_id,
        stack_id,
    };
    let (name, payload) = stack_update.emittable();
    (emitter)(&name, payload);

    // Update the commit mapping with the new commit ids.
    for (old_commit_id, new_commit_id) in move_result.replaced_commits.iter() {
        commit_mapping.insert(*old_commit_id, *new_commit_id);
    }

    Ok(stack_id)
}

pub struct SplitCommit;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SplitCommitParameters {
    /// The stack id containing the commit to split.
    #[schemars(description = "
    <description>
        The stack id containing the commit to split.
    </description>

    <important_notes>
        The stack id should refer to a stack in the workspace that contains the source commit.
    </important_notes>
    ")]
    pub source_stack_id: String,
    /// The commit id to split.
    #[schemars(description = "
    <description>
        The commit id of the commit to split.
    </description>

    <important_notes>
        The commit id should refer to a commit in the workspace.
        This is the commit whose changes will be split into multiple new commits.
        The commit id should be contained in the stack specified by `source_stack_id`.
    </important_notes>
    ")]
    pub source_commit_id: String,

    /// The definitions for each new commit shard.
    #[schemars(description = "
    <description>
        The definitions for each new commit shard.
        Each shard specifies the commit message and the list of files to include in that shard.
    </description>

    <important_notes>
        Each shard must have a unique set of files (no overlap).
        All files in the source commit must be assigned to a shard.
        The order of the shards determines the order of the resulting commits (first being the newest or 'child-most' commit and las being the oldest or 'parent-most').
    </important_notes>
    ")]
    pub shards: Vec<CommitShard>,
}

impl Tool for SplitCommit {
    fn name(&self) -> String {
        "split_commit".to_string()
    }

    fn description(&self) -> String {
        "
        <description>
            Split a single commit into multiple new commits, each with its own message and file set.
        </description>

        <important_notes>
            This tool allows you to break up a commit into several smaller commits, each defined by a shard.
            Each shard must have a unique set of files, and all files in the source commit must be assigned to a shard.
            The order of the shards determines the order of the resulting commits.
        </important_notes>
        "
        .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        let schema = schema_for!(SplitCommitParameters);
        serde_json::to_value(&schema).unwrap_or_default()
    }

    fn call(
        self: Arc<Self>,
        parameters: serde_json::Value,
        ctx: &mut Context,
        emitter: Arc<Emitter>,
        commit_mapping: &mut HashMap<gix::ObjectId, gix::ObjectId>,
    ) -> anyhow::Result<serde_json::Value> {
        let params = serde_json::from_value::<SplitCommitParameters>(parameters)
            .map_err(|e| anyhow::anyhow!("Failed to parse input parameters: {}", e))?;

        let value = split_commit(ctx, params, emitter, commit_mapping).to_json("split_commit");
        Ok(value)
    }
}
pub fn split_commit(
    ctx: &mut Context,
    params: SplitCommitParameters,
    emitter: Arc<Emitter>,
    commit_mapping: &mut HashMap<gix::ObjectId, gix::ObjectId>,
) -> Result<Vec<gix::ObjectId>, anyhow::Error> {
    let source_stack_id = StackId::from_str(&params.source_stack_id)?;
    let source_commit_id = gix::ObjectId::from_str(&params.source_commit_id)
        .map(|id| find_the_right_commit_id(id, commit_mapping))?;

    let pieces = params
        .shards
        .into_iter()
        .map(Into::into)
        .collect::<Vec<but_workspace::legacy::CommitFiles>>();

    let outcome = but_workspace::legacy::split_commit(
        ctx,
        source_stack_id,
        source_commit_id,
        &pieces,
        ctx.settings().context_lines,
    )?;

    let CommmitSplitOutcome {
        new_commits,
        move_changes_result,
    } = outcome;

    // Emit a stack update for the frontend.
    let project_id = ctx.legacy_project.id;
    let stack_update = StackUpdate {
        project_id,
        stack_id: source_stack_id,
    };
    let (name, payload) = stack_update.emittable();
    (emitter)(&name, payload);

    // Update the commit mapping with the new commit ids.
    for (old_commit_id, new_commit_id) in move_changes_result.replaced_commits.iter() {
        commit_mapping.insert(*old_commit_id, *new_commit_id);
    }

    Ok(new_commits)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommitShard {
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
        Don't exceed 50 characters in length.
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
    /// The list of file paths to be included in the commit.
    ///
    /// Each entry is a string representing the relative path to a file.
    #[schemars(description = "
    <description>
        The list of file paths to be included in the commit.
        Each entry is a string representing the relative path to a file.
    </description>

    <important_notes>
        The file paths should be files that exist in the the source commit.
        The file paths are unique to this commit shard, there can't be duplicates.
    </important_notes>
    ")]
    pub files: Vec<String>,
}

impl From<CommitShard> for but_workspace::legacy::CommitFiles {
    fn from(value: CommitShard) -> Self {
        let message = format!(
            "{}\n\n{}",
            value.message_title.trim(),
            value.message_body.trim()
        );

        but_workspace::legacy::CommitFiles {
            message,
            files: value.files,
        }
    }
}

fn ref_metadata_toml(project: &Project) -> anyhow::Result<VirtualBranchesTomlMetadata> {
    VirtualBranchesTomlMetadata::from_path(project.gb_dir().join("virtual_branches.toml"))
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RichHunk {
    /// The diff string.
    pub diff: String,
    /// The stack ID this hunk is assigned to, if any.
    pub assigned_to_stack: Option<but_core::ref_metadata::StackId>,
    /// The locks this hunk has, if any.
    pub dependency_locks: Vec<but_hunk_dependency::ui::HunkLock>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleCommit {
    /// The commit sha.
    #[serde(with = "but_serde::object_id")]
    pub id: gix::ObjectId,
    /// The commit message.
    pub message_title: String,
    /// The commit message body.
    pub message_body: String,
}

impl From<but_workspace::ui::Commit> for SimpleCommit {
    fn from(commit: but_workspace::ui::Commit) -> Self {
        let message_str = commit.message.to_string();
        let mut lines = message_str.lines();
        let message_title = lines.next().unwrap_or_default().to_string();
        let mut message_body = lines.collect::<Vec<_>>().join("\n");
        // Remove leading empty lines from the body
        while message_body.starts_with('\n') || message_body.starts_with("\r\n") {
            message_body = message_body
                .trim_start_matches('\n')
                .trim_start_matches("\r\n")
                .to_string();
        }
        SimpleCommit {
            id: commit.id,
            message_title,
            message_body,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleBranch {
    /// The name of the branch.
    pub name: String,
    /// The description of the branch.
    pub description: Option<String>,
    /// The commits in the branch.
    pub commits: Vec<SimpleCommit>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleStack {
    /// The stack ID.
    pub id: but_core::ref_metadata::StackId,
    /// The name of the stack.
    pub name: String,
    /// The branches in the stack.
    pub branches: Vec<SimpleBranch>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeSimple {
    /// The path of the file that has changed.
    pub path: String,
    /// The file change status
    pub status: String,
}

impl From<but_core::ui::TreeChange> for FileChangeSimple {
    fn from(change: but_core::ui::TreeChange) -> Self {
        FileChangeSimple {
            path: change.path.to_string(),
            status: match change.status {
                but_core::ui::TreeStatus::Addition { .. } => "added".to_string(),
                but_core::ui::TreeStatus::Deletion { .. } => "deleted".to_string(),
                but_core::ui::TreeStatus::Modification { .. } => "modified".to_string(),
                but_core::ui::TreeStatus::Rename { .. } => "renamed".to_string(),
            },
        }
    }
}

impl ToolResult for Result<Vec<FileChangeSimple>, anyhow::Error> {
    fn to_json(&self, action_identifier: &str) -> serde_json::Value {
        result_to_json(self, action_identifier, "Vec<FileChangeSimple>")
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    /// The path of the file that has changed.
    pub path: String,
    /// The file change status
    pub status: String,
    /// The hunk changes in the file.
    pub hunks: Vec<RichHunk>,
}

impl ToolResult for Result<Vec<FileChange>, anyhow::Error> {
    fn to_json(&self, action_identifier: &str) -> serde_json::Value {
        result_to_json(self, action_identifier, "Vec<FileChange>")
    }
}

/// Represents the status of a project, including applied stacks and file changes.
///
/// The shape of this struct is designed to be serializable and as simple as possible for use in LLM context.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStatus {
    /// List of stacks applied to the project's workspace
    pub stacks: Vec<SimpleStack>,
    /// Unified diff changes that could be committed.
    pub file_changes: Vec<FileChange>,
}

impl ToolResult for Result<ProjectStatus, anyhow::Error> {
    fn to_json(&self, action_identifier: &str) -> serde_json::Value {
        result_to_json(self, action_identifier, "ProjectStatus")
    }
}

pub fn get_project_status(
    ctx: &mut Context,
    filter_changes: Option<Vec<BString>>,
) -> anyhow::Result<ProjectStatus> {
    let stacks = stacks(ctx)?;
    let stacks = entries_to_simple_stacks(
        &stacks
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>, _>>()?,
        ctx,
    )?;

    let file_changes = get_filtered_changes(ctx, filter_changes)?;

    Ok(ProjectStatus {
        stacks,
        file_changes,
    })
}

pub fn get_filtered_changes(
    ctx: &mut Context,
    filter_changes: Option<Vec<BString>>,
) -> Result<Vec<FileChange>, anyhow::Error> {
    // TODO(db): once `&mut Context` is `&Context`, let it use `ctx.repo.get()`.
    let repo = ctx.open_isolated_repo()?;
    let worktree = but_core::diff::worktree_changes(&repo)?;
    let changes = if let Some(filter) = filter_changes {
        worktree
            .changes
            .into_iter()
            .filter(|change| filter.contains(&change.path))
            .collect::<Vec<_>>()
    } else {
        worktree.changes.clone()
    };
    let diff = unified_diff_for_changes(&repo, changes, ctx.settings().context_lines)?;
    let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
        ctx,
        false,
        None::<Vec<but_core::TreeChange>>,
        None,
    )
    .map_err(|err| serde_error::Error::new(&*err))?;
    let file_changes = get_file_changes(&diff, assignments.clone());
    Ok(file_changes)
}

fn entries_to_simple_stacks(
    entries: &[StackEntryNoOpt],
    ctx: &Context,
) -> anyhow::Result<Vec<SimpleStack>> {
    let mut stacks = vec![];
    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let repo = &*ctx.repo.get()?;
    for entry in entries {
        let stack = vb_state.get_stack(entry.id)?;
        let branches = stack.branches();
        let branches = branches.iter().filter(|b| !b.archived);
        let mut simple_branches = vec![];
        for branch in branches {
            let commits =
                but_workspace::legacy::local_and_remote_commits(ctx, repo, branch, &stack)?;

            if commits.is_empty() {
                continue;
            }

            let simple_commits = commits
                .into_iter()
                .map(SimpleCommit::from)
                .collect::<Vec<_>>();

            simple_branches.push(SimpleBranch {
                name: branch.name.to_string(),
                description: branch.description.clone(),
                commits: simple_commits,
            });
        }
        if simple_branches.is_empty() {
            continue;
        }

        stacks.push(SimpleStack {
            id: entry.id,
            name: entry.name().unwrap_or_default().to_string(),
            branches: simple_branches,
        });
    }
    Ok(stacks)
}

fn get_file_changes(
    changes: &[(TreeChange, UnifiedPatch)],
    assignments: Vec<but_hunk_assignment::HunkAssignment>,
) -> Vec<FileChange> {
    let mut file_changes = vec![];
    for (change, unified_diff) in changes.iter() {
        match unified_diff {
            but_core::UnifiedPatch::Patch { hunks, .. } => {
                let path = change.path.to_string();
                let status = match &change.status {
                    but_core::TreeStatus::Addition { .. } => "added".to_string(),
                    but_core::TreeStatus::Deletion { .. } => "deleted".to_string(),
                    but_core::TreeStatus::Modification { .. } => "modified".to_string(),
                    but_core::TreeStatus::Rename { previous_path, .. } => {
                        format!("renamed from {previous_path}")
                    }
                };

                let hunks = hunks
                    .iter()
                    .map(|hunk| {
                        let diff = hunk.diff.to_string();
                        let assignment = assignments
                            .iter()
                            .find(|a| {
                                a.path_bytes == change.path && a.hunk_header == Some(hunk.into())
                            })
                            .map(|a| (a.stack_id, a.hunk_locks.clone()));

                        let (assigned_to_stack, dependency_locks) =
                            if let Some((stack_id, locks)) = assignment {
                                let locks = locks.unwrap_or_default();
                                (stack_id, locks)
                            } else {
                                (None, vec![])
                            };

                        RichHunk {
                            diff,
                            assigned_to_stack,
                            dependency_locks,
                        }
                    })
                    .collect::<Vec<_>>();

                file_changes.push(FileChange {
                    path,
                    status,
                    hunks,
                });
            }
            _ => continue,
        }
    }

    file_changes
}

fn unified_diff_for_changes(
    repo: &gix::Repository,
    changes: Vec<but_core::TreeChange>,
    context_lines: u32,
) -> anyhow::Result<Vec<(but_core::TreeChange, but_core::UnifiedPatch)>> {
    changes
        .into_iter()
        .map(|tree_change| {
            tree_change
                .unified_patch(repo, context_lines)
                .map(|diff| (tree_change, diff.expect("no submodule")))
        })
        .collect::<Result<Vec<_>, _>>()
}

fn changes_in_branch_inner(
    ctx: &Context,
    branch_name: String,
    stack_id: Option<StackId>,
) -> anyhow::Result<but_core::ui::TreeChanges> {
    let state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let repo = ctx.repo.get()?;
    let (start_commit_id, base_commit_id) = if let Some(stack_id) = stack_id {
        commit_and_base_from_stack(ctx, &state, stack_id, branch_name.clone())
    } else {
        let start_commit_id = repo.find_reference(&branch_name)?.peel_to_commit()?.id;
        let target = state.get_default_target()?;
        let merge_base = ctx
            .git2_repo
            .get()?
            .merge_base(start_commit_id.to_git2(), target.sha)?;
        Ok((start_commit_id, merge_base.to_gix()))
    }?;

    but_core::diff::ui::changes_with_line_stats_in_range(&repo, start_commit_id, base_commit_id)
}

fn commit_and_base_from_stack(
    ctx: &Context,
    state: &VirtualBranchesHandle,
    stack_id: StackId,
    branch_name: String,
) -> anyhow::Result<(gix::ObjectId, gix::ObjectId)> {
    let stack = state.get_stack(stack_id)?;

    // Find the branch head and the one before it
    let heads = stack.heads(false);
    let (start, end) = heads
        .iter()
        .rev()
        .fold((None, None), |(start, end), branch| {
            if start.is_some() && end.is_none() {
                (start, Some(branch))
            } else if branch == &branch_name {
                (Some(branch), None)
            } else {
                (start, end)
            }
        });
    let repo = ctx.repo.get()?;

    // Find the head that matches the branch name - the commit contained is our commit_id
    let start_commit_id = repo
        .find_reference(start.with_context(|| format!("Branch {branch_name} not found"))?)?
        .peel_to_commit()?
        .id;

    // Now, find the preceding head in the stack. If it is not present, use the stack merge base
    let base_commit_id = match end {
        Some(end) => repo.find_reference(end)?.peel_to_commit()?.id,
        None => stack.merge_base(ctx)?,
    };
    Ok((start_commit_id, base_commit_id))
}

#[derive(Debug, serde::Serialize, serde::Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct AbsorbSpec {
    /// The title of the commit to use in the amended commit.
    #[schemars(description = "
    <description>
        The title of the commit to use in the amended commit.
    </description>

    <important_notes>
        The title should be concise and descriptive.
        Don't use more than 50 characters.
        It should be different from the original commit title only if needed.
    </important_notes>
    ")]
    pub commit_title: String,
    /// The description of the commit to use in the amended commit.
    #[schemars(description = "
    <description>
        The description of the commit to use in the amended commit.
    </description>

    <important_notes>
        The description should provide context and details about the changes made.
        It should span multiple lines if necessary.
        A good description focuses on describing the 'what' of the changes.
        Don't make assumption about the 'why', only describe the changes in the context of the branch (and other commits if any).
    </important_notes>
    ")]
    pub commit_description: String,
}

fn find_the_right_commit_id(
    commit_id: gix::ObjectId,
    commit_mapping: &HashMap<gix::ObjectId, gix::ObjectId>,
) -> gix::ObjectId {
    let mut visited_commits = HashSet::new();
    let mut commit_id = commit_id;
    while let Some(mapped_id) = commit_mapping.get(&commit_id) {
        if *mapped_id == commit_id {
            // If the mapped id is the same as the original, we can stop.
            break;
        }

        if visited_commits.contains(mapped_id) {
            // If we have already visited this commit, we are in a loop.
            break;
        }

        visited_commits.insert(commit_id);
        commit_id = *mapped_id;
    }
    commit_id
}

fn stacks(ctx: &Context) -> anyhow::Result<Vec<but_workspace::legacy::ui::StackEntry>> {
    let meta = ref_metadata_toml(&ctx.legacy_project)?;
    let repo = &*ctx.repo.get()?;
    but_workspace::legacy::stacks_v3(
        repo,
        &meta,
        but_workspace::legacy::StacksFilter::InWorkspace,
        None,
    )
}
