use std::path::Path;

use anyhow::bail;
use but_core::{DiffSpec, TreeChange};
use but_workspace::legacy::commit_engine::{ReferenceFrame, create_commit_and_update_refs};
use gitbutler_stack::VirtualBranchesState;

use crate::command::{
    debug_print, discard_change::IndicesOrHeaders, indices_or_headers_to_hunk_headers,
    path_to_rela_path,
};

#[expect(clippy::too_many_arguments)]
pub fn commit(
    repo: gix::Repository,
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

    commit_without_project(
        &repo,
        message,
        amend,
        parent_id,
        stack_segment_ref,
        workspace_tip,
        changes,
        use_json,
    )
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
