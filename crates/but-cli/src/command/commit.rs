use crate::command::debug_print;
use anyhow::bail;
use but_core::TreeChange;
use but_workspace::commit_engine::reference_frame::InferenceMode;
use but_workspace::commit_engine::{DiffSpec, ReferenceFrame};
use gitbutler_project::Project;
use gitbutler_stack::VirtualBranchesHandle;

pub fn commit(
    repo: gix::Repository,
    project: Option<Project>,
    message: Option<&str>,
    amend: bool,
    parent_revspec: Option<&str>,
) -> anyhow::Result<()> {
    if message.is_none() && !amend {
        bail!("Need a message when creating a new commit");
    }
    let mut parent_id = parent_revspec
        .map(|revspec| repo.rev_parse_single(revspec).map_err(anyhow::Error::from))
        .unwrap_or_else(|| Ok(repo.head_id()?))?
        .detach();
    #[allow(unused_assignments)]
    let mut frame = None;
    let mut project_and_vb = if let Some(project) = project {
        let guard = project.exclusive_worktree_access();
        let vbs = VirtualBranchesHandle::new(project.gb_dir()).read_file()?;
        let reference_frame =
            ReferenceFrame::infer(&repo, &vbs, InferenceMode::CommitIdInStack(parent_id))?;
        // This might be the default set earlier, but we never want to push on top of the workspace commit.
        if repo.head_id().ok().map(|id| id.detach()) == Some(parent_id) {
            parent_id = reference_frame
                .branch_tip
                .expect("set as we need the parent to be part of a stack");
        }
        frame = Some(reference_frame);
        Some((project, vbs, guard))
    } else {
        None
    };
    debug_print(
        but_workspace::commit_engine::create_commit_and_update_refs_with_project(
            &repo,
            project_and_vb
                .as_mut()
                .zip(frame)
                .map(|((_project, vbs, guard), frame)| (frame, vbs, guard.write_permission())),
            if amend {
                if message.is_some() {
                    bail!("Messages aren't used when amending");
                }
                but_workspace::commit_engine::Destination::AmendCommit(parent_id)
            } else {
                but_workspace::commit_engine::Destination::NewCommit {
                    parent_commit_id: Some(parent_id),
                    message: message.unwrap_or_default().to_owned(),
                }
            },
            None,
            to_whole_file_diffspec(but_core::diff::worktree_changes(&repo)?.changes),
            0, /* context-lines */
        )?,
    )?;

    if let Some((project, vbs, _guard)) = project_and_vb {
        VirtualBranchesHandle::new(project.gb_dir()).write_file(&vbs)?;
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
