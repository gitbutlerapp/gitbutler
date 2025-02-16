use crate::command::debug_print;
use anyhow::bail;
use but_core::TreeChange;
use but_workspace::commit_engine::DiffSpec;
use gitbutler_oxidize::OidExt;
use gitbutler_project::Project;
use gitbutler_stack::{VirtualBranchesHandle, VirtualBranchesState};
use gix::prelude::ObjectIdExt;
use gix::revision::walk::Sorting;

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
    let mut vbs = None;
    let mut frame = None;
    let mut project = if let Some(project) = project {
        let guard = project.exclusive_worktree_access();
        vbs = Some(VirtualBranchesHandle::new(project.gb_dir()).read_file()?);
        let reference_frame = project_to_reference_frame(&repo, vbs.as_mut().unwrap(), parent_id)?;
        // This might be the default set earlier, but we never want to push on top of the workspace commit.
        if repo.head_id().ok().map(|id| id.detach()) == Some(parent_id) {
            parent_id = reference_frame
                .branch_tip
                .expect("set as we need the parent to be part of a stack");
        }
        frame = Some(reference_frame);
        Some((project, guard))
    } else {
        None
    };
    debug_print(
        but_workspace::commit_engine::create_commit_and_update_refs_with_project(
            &repo,
            project
                .as_mut()
                .zip(frame)
                .map(|((_project, guard), frame)| (frame, guard.write_permission())),
            if amend {
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

    if let Some((vbs, (project, _guard))) = vbs.zip(project) {
        VirtualBranchesHandle::new(project.gb_dir()).write_file(&vbs)?;
    }
    Ok(())
}

/// Find the tip of the stack that will contain the `parent_id`, and the workspace merge commit as well.
fn project_to_reference_frame<'a>(
    repo: &gix::Repository,
    vb: &'a mut VirtualBranchesState,
    parent_id: gix::ObjectId,
) -> anyhow::Result<but_workspace::commit_engine::ReferenceFrame<'a>> {
    let head_id = repo.head_id()?;
    let workspace_commit = head_id.object()?.into_commit().decode()?.to_owned();
    if workspace_commit.parents.len() < 2 {
        return Ok(but_workspace::commit_engine::ReferenceFrame {
            workspace_tip: Some(head_id.detach()),
            // The workspace commit is never the tip
            branch_tip: Some(workspace_commit.parents[0]),
            vb,
        });
    }

    let cache = repo.commit_graph_if_enabled()?;
    let mut graph = repo.revision_graph(cache.as_ref());
    let default_target_tip = vb
        .default_target
        .as_ref()
        .map(|target| -> anyhow::Result<_> {
            let r = repo.find_reference(&target.branch.to_string())?;
            Ok(r.try_id())
        })
        .and_then(Result::ok)
        .flatten();

    let merge_base = if default_target_tip.is_none() {
        Some(repo.merge_base_octopus(workspace_commit.parents)?)
    } else {
        None
    };
    for stack in vb.branches.values() {
        let stack_tip = stack.head.to_gix();
        if stack_tip
            .attach(repo)
            .ancestors()
            .with_boundary(match default_target_tip {
                Some(target_tip) => {
                    Some(repo.merge_base_with_graph(stack_tip, target_tip, &mut graph)?)
                }
                None => merge_base,
            })
            .sorting(Sorting::BreadthFirst)
            .all()?
            .filter_map(Result::ok)
            .any(|info| info.id == parent_id)
        {
            return Ok(but_workspace::commit_engine::ReferenceFrame {
                workspace_tip: Some(head_id.detach()),
                branch_tip: Some(stack_tip),
                vb,
            });
        }
    }
    bail!("Could not find stack that includes parent-id at {parent_id}")
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
