use std::str::FromStr;
use std::sync::Arc;

use bstr::BString;
use but_core::{TreeChange, UnifiedDiff};
use but_graph::VirtualBranchesTomlMetadata;
use but_workspace::ui::StackEntry;
use gitbutler_command_context::CommandContext;
use gitbutler_oplog::{OplogExt, SnapshotExt};
use gitbutler_project::Project;
use gitbutler_stack::{PatchReferenceUpdate, VirtualBranchesHandle};
use schemars::{JsonSchema, schema_for};

use crate::emit::EmitStackUpdate;
use crate::tool::{Tool, ToolResult, Toolset, result_to_json};

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

/// Creates a toolset for amend operations.
pub fn amend_toolset<'a>(
    ctx: &'a mut CommandContext,
    app_handle: Option<&'a tauri::AppHandle>,
) -> anyhow::Result<Toolset<'a>> {
    let mut toolset = Toolset::new(ctx, app_handle);

    toolset.register_tool(Amend);

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
        This should be an update of the existin commit message body in order to accomodate the changes amended into it.
        If the description already matches the changes, you can pass in the same description.
        The commit message body should provide context and details about the changes made.
        It should span multiple lines if necessary.
        A good description focuses on describing the 'what' of the changes.
        Don't make assumption about the 'why', only describe the changes in the context of the branch (and other commits if any).
    </important_notes>
    ")]
    pub message_body: String,
    /// The branch name containing the commit to amend.
    #[schemars(description = "
    <description>
        The name of the branch containing the commit to amend.
    </description>

    <important_notes>
        The branch name should match an existing branch in the workspace.
    </important_notes>
    ")]
    pub branch_name: String,
    /// The list of files to include in the amended commit.
    #[schemars(description = "
        <description>
            The list of file paths to include in the amended commit.
        </description>

        <important_notes>
            The file paths should be relative to the workspace root.
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
        ctx: &mut CommandContext,
        app_handle: Option<&tauri::AppHandle>,
    ) -> anyhow::Result<serde_json::Value> {
        let params: AmendParameters = serde_json::from_value(parameters)
            .map_err(|e| anyhow::anyhow!("Failed to parse input parameters: {}", e))?;

        let value = amend_commit(ctx, app_handle, params).to_json("amend_commit");
        Ok(value)
    }
}

pub fn amend_commit(
    ctx: &mut CommandContext,
    app_handle: Option<&tauri::AppHandle>,
    params: AmendParameters,
) -> Result<but_workspace::commit_engine::ui::CreateCommitOutcome, anyhow::Error> {
    let repo = ctx.gix_repo()?;
    let project = ctx.project();
    let settings = ctx.app_settings();
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
        .find_map(|s| {
            let found = s.heads.iter().any(|h| h.name == params.branch_name);
            if found { Some(s.id) } else { None }
        })
        .ok_or_else(|| anyhow::anyhow!("Branch not found: {}", params.branch_name))?;

    let message = format!(
        "{}\n\n{}",
        params.message_title.trim(),
        params.message_body.trim()
    );

    let outcome = but_workspace::commit_engine::create_commit_and_update_refs_with_project(
        &repo,
        project,
        Some(stack_id),
        but_workspace::commit_engine::Destination::AmendCommit {
            commit_id: gix::ObjectId::from_str(&params.commit_id)?,
            new_message: Some(message),
        },
        None,
        file_changes,
        settings.context_lines,
        guard.write_permission(),
    );

    // If there's an app handle provided, emit an event to update the stack details in the UI.
    if let Some(app_handle) = app_handle {
        let project_id = ctx.project().id;
        app_handle.emit_stack_update(project_id, stack_id);
    }

    let outcome: but_workspace::commit_engine::ui::CreateCommitOutcome = outcome?.into();
    Ok(outcome)
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
        ctx: &mut CommandContext,
        _app_handle: Option<&tauri::AppHandle>,
    ) -> anyhow::Result<serde_json::Value> {
        let repo = ctx.gix_repo()?;
        let params: GetProjectStatusParameters = serde_json::from_value(parameters)
            .map_err(|e| anyhow::anyhow!("Failed to parse input parameters: {}", e))?;

        let paths = params
            .filter_changes
            .map(|f| f.into_iter().map(BString::from).collect::<Vec<BString>>());

        let value = get_project_status(ctx, &repo, paths).to_json("get_project_status");
        Ok(value)
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
    pub assigned_to_stack: Option<but_workspace::StackId>,
    /// The locks this hunk has, if any.
    pub dependency_locks: Vec<but_hunk_dependency::ui::HunkLock>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleCommit {
    /// The commit sha.
    #[serde(with = "gitbutler_serde::object_id")]
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
    pub id: but_workspace::StackId,
    /// The name of the stack.
    pub name: String,
    /// The branches in the stack.
    pub branches: Vec<SimpleBranch>,
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

/// Represents the status of a project, including applied stacks and file changes.
///
/// The shape of this struct is designed to be serializable and as simple as possible for use in LLM context.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStatus {
    /// List of stacks applied to the project's workspace
    stacks: Vec<SimpleStack>,
    /// Unified diff changes that could be committed.
    file_changes: Vec<FileChange>,
}

impl ToolResult for Result<ProjectStatus, anyhow::Error> {
    fn to_json(&self, action_identifier: &str) -> serde_json::Value {
        result_to_json(self, action_identifier, "ProjectStatus")
    }
}

pub fn get_project_status(
    ctx: &mut CommandContext,
    repo: &gix::Repository,
    filter_changes: Option<Vec<BString>>,
) -> anyhow::Result<ProjectStatus> {
    let stacks = stacks(ctx, repo)?;
    let stacks = entries_to_simple_stacks(&stacks, ctx, repo)?;

    let worktree = but_core::diff::worktree_changes(repo)?;
    let changes = if let Some(filter) = filter_changes {
        worktree
            .changes
            .into_iter()
            .filter(|change| filter.iter().any(|f| *f == change.path))
            .collect::<Vec<_>>()
    } else {
        worktree.changes.clone()
    };
    let diff = unified_diff_for_changes(repo, changes, ctx.app_settings().context_lines)?;
    // Get any assignments that may have been made, which also includes any hunk locks. Assignments should be updated according to locks where applicable.
    let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
        ctx,
        true,
        None::<Vec<but_core::TreeChange>>,
        None,
    )
    .map_err(|err| serde_error::Error::new(&*err))?;

    let file_changes = get_file_changes(&diff, assignments.clone());

    Ok(ProjectStatus {
        stacks,
        file_changes,
    })
}

fn entries_to_simple_stacks(
    entries: &[StackEntry],
    ctx: &mut CommandContext,
    repo: &gix::Repository,
) -> anyhow::Result<Vec<SimpleStack>> {
    let mut stacks = vec![];
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    for entry in entries {
        let stack = vb_state.get_stack(entry.id)?;
        let branches = stack.branches();
        let branches = branches.iter().filter(|b| !b.archived);
        let mut simple_branches = vec![];
        for branch in branches {
            let commits = but_workspace::local_and_remote_commits(ctx, repo, branch, &stack)?;

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
    changes: &[(TreeChange, UnifiedDiff)],
    assingments: Vec<but_hunk_assignment::HunkAssignment>,
) -> Vec<FileChange> {
    let mut file_changes = vec![];
    for (change, unified_diff) in changes.iter() {
        match unified_diff {
            but_core::UnifiedDiff::Patch { hunks, .. } => {
                let path = change.path.to_string();
                let status = match &change.status {
                    but_core::TreeStatus::Addition { .. } => "added".to_string(),
                    but_core::TreeStatus::Deletion { .. } => "deleted".to_string(),
                    but_core::TreeStatus::Modification { .. } => "modified".to_string(),
                    but_core::TreeStatus::Rename { previous_path, .. } => {
                        format!("renamed from {}", previous_path)
                    }
                };

                let hunks = hunks
                    .iter()
                    .map(|hunk| {
                        let diff = hunk.diff.to_string();
                        let assignment = assingments
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
) -> anyhow::Result<Vec<(but_core::TreeChange, but_core::UnifiedDiff)>> {
    changes
        .into_iter()
        .map(|tree_change| {
            tree_change
                .unified_diff(repo, context_lines)
                .map(|diff| (tree_change, diff.expect("no submodule")))
        })
        .collect::<Result<Vec<_>, _>>()
}
