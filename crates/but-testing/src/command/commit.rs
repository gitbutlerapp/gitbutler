use std::path::Path;

use anyhow::bail;
use but_core::{DiffSpec, TreeChange};
use but_workspace::{
    commit_engine::StackSegmentId,
    legacy::commit_engine::{ReferenceFrame, create_commit_and_update_refs},
};
use gitbutler_project::Project;
use gitbutler_stack::{VirtualBranchesHandle, VirtualBranchesState};

use crate::command::{
    debug_print, discard_change::IndicesOrHeaders, indices_or_headers_to_hunk_headers,
    path_to_rela_path,
};

#[expect(clippy::too_many_arguments)]
pub fn commit(
    repo: gix::Repository,
    project: Option<Project>,
    message: Option<&str>,
    amend: bool,
    parent_revspec: Option<&str>,
    stack_segment_ref: Option<&str>,
    workspace_tip: Option<&str>,
    current_rela_path: Option<&Path>,
    previous_rela_path: Option<&Path>,
    headers: Option<&[u32]>,
    diff_spec: Option<Vec<DiffSpec>>,
    use_json: bool,
) -> anyhow::Result<()> {
    if message.is_none() && !amend {
        bail!("Need a message when creating a new commit");
    }

    let parent_id = resolve_parent_id(&repo, parent_revspec)?;

    let changes = resolve_changes(
        &repo,
        current_rela_path,
        previous_rela_path,
        headers,
        diff_spec,
    )?;

    if let Some(project) = project.as_ref() {
        commit_with_project(
            &repo,
            project,
            message,
            amend,
            parent_id,
            stack_segment_ref,
            changes,
            use_json,
        )?;
    } else {
        commit_without_project(
            &repo,
            message,
            amend,
            parent_id,
            stack_segment_ref,
            workspace_tip,
            changes,
            use_json,
        )?;
    }
    Ok(())
}

/// Determines the parent commit ID based on the provided `parent_revspec`.
fn resolve_parent_id(
    repo: &gix::Repository,
    parent_revspec: Option<&str>,
) -> anyhow::Result<Option<gix::ObjectId>> {
    parent_revspec
        .map(|revspec| repo.rev_parse_single(revspec).map_err(anyhow::Error::from))
        .map(|id| id.map(|id| id.detach()))
        .transpose()
}

/// Determines the changes to be committed based on the provided parameters.
fn resolve_changes(
    repo: &gix::Repository,
    current_rela_path: Option<&Path>,
    previous_rela_path: Option<&Path>,
    headers: Option<&[u32]>,
    diff_spec: Option<Vec<DiffSpec>>,
) -> anyhow::Result<Vec<DiffSpec>> {
    Ok(
        match (current_rela_path, previous_rela_path, headers, diff_spec) {
            (None, None, None, Some(diff_spec)) => diff_spec,
            (None, None, None, None) => {
                to_whole_file_diffspec(but_core::diff::worktree_changes(repo)?.changes)
            }
            (Some(current_path), previous_path, Some(headers), None) => {
                let path_bytes = path_to_rela_path(current_path)?;
                let previous_path_bytes = previous_path.map(path_to_rela_path).transpose()?;
                let hunk_headers = indices_or_headers_to_hunk_headers(
                    repo,
                    Some(IndicesOrHeaders::Headers(headers)),
                    &path_bytes,
                    previous_path_bytes.as_ref(),
                )?;

                vec![DiffSpec {
                    previous_path: previous_path_bytes,
                    path: path_bytes,
                    hunk_headers,
                }]
            }
            _ => unreachable!("BUG: specifying this shouldn't be possible"),
        },
    )
}

#[expect(clippy::too_many_arguments)]
fn commit_with_project(
    repo: &gix::Repository,
    project: &Project,
    message: Option<&str>,
    amend: bool,
    parent_id: Option<gix::ObjectId>,
    stack_segment_ref: Option<&str>,
    changes: Vec<DiffSpec>,
    use_json: bool,
) -> anyhow::Result<()> {
    let destination = if amend {
        let parent_id = parent_id.unwrap_or(repo.head_id()?.detach());
        but_workspace::commit_engine::Destination::AmendCommit {
            commit_id: parent_id,
            new_message: message.map(ToOwned::to_owned),
        }
    } else {
        let (stack_segment, parent_commit_id) =
            get_stack_segment_info(repo, stack_segment_ref, parent_id, project)?;

        but_workspace::commit_engine::Destination::NewCommit {
            parent_commit_id,
            message: message.unwrap_or_default().to_owned(),
            stack_segment,
        }
    };
    let mut guard = but_core::sync::exclusive_worktree_access(project.git_dir());
    let outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs_with_project(
        repo,
        project,
        None,
        destination,
        changes,
        0, /* context-lines */
        guard.write_permission(),
    )?;

    if use_json {
        let outcome = but_workspace::commit_engine::ui::CreateCommitOutcome::from(outcome);
        let json = serde_json::to_string_pretty(&outcome)?;
        println!("{json}");
    } else {
        debug_print(outcome)?;
    }

    Ok(())
}

#[expect(clippy::too_many_arguments)]
fn commit_without_project(
    repo: &gix::Repository,
    message: Option<&str>,
    amend: bool,
    parent_id: Option<gix::ObjectId>,
    stack_segment_ref: Option<&str>,
    workspace_tip: Option<&str>,
    changes: Vec<DiffSpec>,
    use_json: bool,
) -> anyhow::Result<()> {
    let destination = if amend {
        let parent_id = parent_id.unwrap_or(repo.head_id()?.detach());
        but_workspace::commit_engine::Destination::AmendCommit {
            commit_id: parent_id,
            new_message: message.map(ToOwned::to_owned),
        }
    } else {
        but_workspace::commit_engine::Destination::NewCommit {
            parent_commit_id: parent_id,
            message: message.unwrap_or_default().to_owned(),
            stack_segment: None,
        }
    };

    let outcome = create_commit_and_update_refs(
        repo,
        ReferenceFrame {
            workspace_tip: workspace_tip
                .map(|spec| repo.rev_parse_single(spec))
                .transpose()?
                .map(|id| id.detach()),
            branch_tip: Some(
                stack_segment_ref
                    .map(|name| repo.find_reference(name).map(|r| r.id().detach()))
                    .transpose()?
                    .unwrap_or(repo.head_id()?.detach()),
            ),
        },
        &mut VirtualBranchesState::default(),
        destination,
        changes,
        0,
    )?;

    if use_json {
        let outcome = but_workspace::commit_engine::ui::CreateCommitOutcome::from(outcome);
        let json = serde_json::to_string_pretty(&outcome)?;
        println!("{json}");
    } else {
        debug_print(outcome)?;
    }

    Ok(())
}

/// Determines the target stack segment (branch) and the target parent commit ID based on the provided `stack_segment_ref` and `parent_id`.
///
/// If a branch is provided is provided:
/// - Normalizes the reference and attempts to find the corresponding stack segment in the workspace.
/// - Constructs a `StackSegmentId` if a matching stack is found.
/// - Determines the parent commit ID:
///   - Uses the provided `parent_id` if defined.
///   - Otherwise, tries to find the branch reference in the repository and peels it to a commit to get its ID.
///
/// If a branch is not provided:
/// - Returns `None` for the stack segment and uses the provided `parent_id`.
///
/// Returns a tuple containing:
/// - An `Option<StackSegmentId>` representing the target stack segment (if found).
/// - An `Option<CommitId>` representing the target parent commit ID (if found).
fn get_stack_segment_info(
    repo: &gix::Repository,
    stack_segment_ref: Option<&str>,
    parent_id: Option<gix::ObjectId>,
    project: &Project,
) -> Result<(Option<StackSegmentId>, Option<gix::ObjectId>), anyhow::Error> {
    let (stack_segment, parent_commit_id) = if let Some(stack_segment_ref) = stack_segment_ref {
        let full_name = normalize_stack_segment_ref(stack_segment_ref)?;
        let stack_segment = VirtualBranchesHandle::new(project.gb_dir())
            .list_stacks_in_workspace()?
            .iter()
            .find(|s| s.heads(false).contains(&stack_segment_ref.to_string()))
            .map(|s| s.id)
            .map(|id| StackSegmentId {
                segment_ref: full_name,
                stack_id: id,
            });

        let parent_commit_id = match parent_id {
            Some(id) => Some(id),
            None => {
                let reference = repo
                    .try_find_reference(stack_segment_ref)
                    .map_err(anyhow::Error::from)?;
                if let Some(mut r) = reference {
                    Some(r.peel_to_commit().map_err(anyhow::Error::from)?.id)
                } else {
                    None
                }
            }
        };

        (stack_segment, parent_commit_id)
    } else {
        (None, parent_id)
    };
    Ok((stack_segment, parent_commit_id))
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

fn to_whole_file_diffspec(changes: Vec<TreeChange>) -> Vec<DiffSpec> {
    changes
        .into_iter()
        .map(|change| DiffSpec {
            previous_path: change.previous_path().map(ToOwned::to_owned),
            path: change.path,
            hunk_headers: Vec::new(),
        })
        .collect()
}
