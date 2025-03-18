use crate::command::debug_print;
use anyhow::bail;
use but_core::TreeChange;
use but_workspace::commit_engine::{
    DiffSpec, ReferenceFrame, StackSegmentId, create_commit_and_update_refs,
};
use gitbutler_project::Project;
use gitbutler_stack::{VirtualBranchesHandle, VirtualBranchesState};

pub fn commit(
    repo: gix::Repository,
    project: Option<Project>,
    message: Option<&str>,
    amend: bool,
    parent_revspec: Option<&str>,
    stack_segment_ref: Option<&str>,
) -> anyhow::Result<()> {
    if message.is_none() && !amend {
        bail!("Need a message when creating a new commit");
    }
    let parent_id = parent_revspec
        .map(|revspec| repo.rev_parse_single(revspec).map_err(anyhow::Error::from))
        .unwrap_or_else(|| Ok(repo.head_id()?))?
        .detach();

    let changes = to_whole_file_diffspec(but_core::diff::worktree_changes(&repo)?.changes);
    if let Some(project) = project.as_ref() {
        let destination = if amend {
            if message.is_some() {
                bail!("Messages aren't used when amending");
            }
            but_workspace::commit_engine::Destination::AmendCommit(parent_id)
        } else {
            let stack_segment = if let Some(stack_segment_ref) = stack_segment_ref {
                let full_name = gix::refs::FullName::try_from(stack_segment_ref)?;
                VirtualBranchesHandle::new(project.gb_dir())
                    .list_stacks_in_workspace()?
                    .iter()
                    .find(|s| s.heads().contains(&stack_segment_ref.to_string()))
                    .map(|s| s.id)
                    .map(|id| StackSegmentId {
                        segment_ref: full_name,
                        stack_id: id,
                    })
            } else {
                None
            };
            but_workspace::commit_engine::Destination::NewCommit {
                parent_commit_id: Some(parent_id),
                message: message.unwrap_or_default().to_owned(),
                stack_segment,
            }
        };
        let mut guard = project.exclusive_worktree_access();
        debug_print(
            but_workspace::commit_engine::create_commit_and_update_refs_with_project(
                &repo,
                project,
                None,
                destination,
                None,
                changes,
                0, /* context-lines */
                guard.write_permission(),
            )?,
        )?;
    } else {
        let destination = if amend {
            if message.is_some() {
                bail!("Messages aren't used when amending");
            }
            but_workspace::commit_engine::Destination::AmendCommit(parent_id)
        } else {
            but_workspace::commit_engine::Destination::NewCommit {
                parent_commit_id: Some(parent_id),
                message: message.unwrap_or_default().to_owned(),
                stack_segment: None,
            }
        };
        debug_print(create_commit_and_update_refs(
            &repo,
            ReferenceFrame {
                workspace_tip: None,
                branch_tip: repo.head_id()?.detach().into(),
            },
            &mut VirtualBranchesState::default(),
            destination,
            None,
            changes,
            0,
        )?)?;
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
