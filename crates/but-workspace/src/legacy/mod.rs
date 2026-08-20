use but_ctx::Context;

pub mod head;
pub use head::{
    merge_worktree_with_workspace, remerged_workspace_commit_v2, remerged_workspace_tree_v2,
};

/// Various types for the frontend.
pub mod ui;

pub mod push;
pub use push::workspace_branch_and_ancestors_push;

/// Return a list of commits on the target branch
/// Starts either from the target branch or from the provided commit id, up to the limit provided.
///
/// Returns the commits in reverse order, i.e., from the most recent to the oldest.
/// The `Commit` type is the same as that of the other workspace endpoints - for that reason,
/// the fields `has_conflicts` and `state` are somewhat meaningless.
pub fn log_target_first_parent(
    ctx: &Context,
    last_commit_id: Option<gix::ObjectId>,
    limit: usize,
) -> anyhow::Result<Vec<crate::ui::Commit>> {
    let repo = ctx.repo.get()?;
    let traversal_root_id = match last_commit_id {
        Some(id) => {
            let commit = repo.find_commit(id)?;
            commit.parent_ids().next()
        }
        None => {
            let project_meta = ctx.project_meta()?;
            Some(
                repo.find_reference(project_meta.target_ref_or_err()?.as_ref())?
                    .peel_to_commit()?
                    .id(),
            )
        }
    };
    let traversal_root_id = match traversal_root_id {
        Some(id) => id,
        None => return Ok(vec![]),
    };

    let mut commits: Vec<crate::ui::Commit> = vec![];
    for commit_info in traversal_root_id.ancestors().first_parent_only().all()? {
        if commits.len() == limit {
            break;
        }
        // In shallow repositories, the traversal may hit a commit whose parent
        // objects are not present locally. Stop rather than propagating the error.
        let info = match commit_info {
            Ok(info) => info,
            Err(_) => break,
        };
        let commit = match info.id().object() {
            Ok(obj) => obj.into_commit(),
            Err(_) => break,
        };
        commits.push(commit.try_into()?);
    }
    Ok(commits)
}
