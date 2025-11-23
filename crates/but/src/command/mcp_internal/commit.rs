use std::path::Path;

use bstr::BString;
use but_workspace::commit_engine::StackSegmentId;
use rmcp::schemars;
use serde::{Deserialize, Serialize};

use crate::command::mcp_internal::project;

/// Commit changes to the repository.
pub fn commit(
    project_dir: &Path,
    commit_message: String,
    diff_spec: Vec<DiffSpec>,
    parent_id: Option<String>,
    branch_name: String,
) -> anyhow::Result<but_workspace::commit_engine::ui::CreateCommitOutcome> {
    let changes: Vec<but_core::DiffSpec> = diff_spec.into_iter().map(Into::into).collect();
    let (repo, project) =
        project::repo_and_maybe_project(project_dir, project::RepositoryOpenMode::Merge)?;

    let project = project.ok_or_else(|| {
        anyhow::anyhow!(
            "No project found in the specified directory: {}",
            project_dir.display()
        )
    })?;

    let branch_full_name = normalize_stack_segment_ref(&branch_name)?;
    let parent_commit_id = parent_id
        .map(|id| resolve_parent_id(&repo, &id))
        .transpose()?;

    let stack_segment = gitbutler_stack::VirtualBranchesHandle::new(project.gb_dir())
        .list_stacks_in_workspace()?
        .iter()
        .find(|s| s.heads(false).contains(&branch_name))
        .map(|s| s.id)
        .map(|id| StackSegmentId {
            segment_ref: branch_full_name,
            stack_id: id,
        });

    let parent_commit_id = match parent_commit_id {
        Some(id) => Some(id),
        None => {
            let reference = repo
                .try_find_reference(&branch_name)
                .map_err(anyhow::Error::from)?;
            if let Some(mut r) = reference {
                Some(r.peel_to_commit().map_err(anyhow::Error::from)?.id)
            } else {
                None
            }
        }
    };

    let destination = but_workspace::commit_engine::Destination::NewCommit {
        parent_commit_id,
        message: commit_message,
        stack_segment,
    };

    let mut guard = but_core::sync::exclusive_worktree_access(project.git_dir());
    let outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs_with_project(
        &repo,
        &project,
        None,
        destination,
        changes,
        0, /* context-lines */
        guard.write_permission(),
    )?;

    Ok(outcome.into())
}

/// Amend an existing commit in the repository.
pub fn amend(
    project_dir: &Path,
    commit_message: String,
    diff_spec: Vec<DiffSpec>,
    commit_id: String,
    branch_name: String,
) -> anyhow::Result<but_workspace::commit_engine::ui::CreateCommitOutcome> {
    let changes: Vec<but_core::DiffSpec> = diff_spec.into_iter().map(Into::into).collect();
    let (repo, project) =
        project::repo_and_maybe_project(project_dir, project::RepositoryOpenMode::Merge)?;

    let project = project.ok_or_else(|| {
        anyhow::anyhow!(
            "No project found in the specified directory: {}",
            project_dir.display()
        )
    })?;

    let commit_id = resolve_parent_id(&repo, &commit_id)?;

    let stack_id = gitbutler_stack::VirtualBranchesHandle::new(project.gb_dir())
        .list_stacks_in_workspace()?
        .iter()
        .find(|s| s.heads(false).contains(&branch_name))
        .map(|s| s.id);

    let destination = but_workspace::commit_engine::Destination::AmendCommit {
        commit_id,
        new_message: Some(commit_message),
    };

    let mut guard = but_core::sync::exclusive_worktree_access(project.git_dir());
    let outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs_with_project(
        &repo,
        &project,
        stack_id,
        destination,
        changes,
        0, /* context-lines */
        guard.write_permission(),
    )?;

    Ok(outcome.into())
}

/// Determines the parent commit ID based on the provided `parent_revspec`.
fn resolve_parent_id(repo: &gix::Repository, parent_id: &str) -> anyhow::Result<gix::ObjectId> {
    repo.rev_parse_single(parent_id)
        .map_err(anyhow::Error::from)
        .map(|id| id.detach())
}

fn normalize_stack_segment_ref(
    stack_segment_ref: &str,
) -> Result<gix::refs::FullName, gix::refs::name::Error> {
    let full_name = if stack_segment_ref.starts_with("refs/heads/") {
        stack_segment_ref.to_string()
    } else {
        format!("refs/heads/{stack_segment_ref}")
    };
    gix::refs::FullName::try_from(full_name)
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DiffSpec {
    /// The previous location of the entry, the source of a rename if there was one.
    #[schemars(description = "The previous path of the file, if it was renamed")]
    pub previous_path: Option<String>,
    /// The worktree-relative path to the worktree file with the content to commit.
    ///
    /// If `hunks` is empty, this means the current content of the file should be committed.
    #[schemars(description = "The path of the file to commit")]
    pub path: String,
}

impl From<DiffSpec> for but_core::DiffSpec {
    fn from(spec: DiffSpec) -> Self {
        but_core::DiffSpec {
            previous_path: spec.previous_path.map(BString::from),
            path: BString::from(spec.path),
            hunk_headers: vec![],
        }
    }
}
