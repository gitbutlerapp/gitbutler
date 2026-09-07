/// Options for altering how [remove_reference()](remove_reference()) works.
#[derive(Default, Debug, Clone, Copy)]
pub struct Options {
    /// If `true`, we will be sure that the top-most reference is always at the top of the stack,
    /// that is on the top-most commit of the topmost, otherwise anonymous, segment.
    ///
    /// If `false`, the deletion of a reference will not have other side-effects.
    pub avoid_anonymous_stacks: bool,
    /// If `true`, do not delete metadata, but leave it stale.
    /// This is useful if the rest of the system works better if reference with the same name
    /// will automatically pick up previously stale metadata.
    pub keep_metadata: bool,
}

use anyhow::{Context as _, bail};
use but_core::RefMetadata;
use but_error::bail_precondition;
use gix::refs::transaction::PreviousValue;

/// Delete `ref_name` and its local branch configuration, returning whether the ref existed.
///
/// Use this for local branch removal when the corresponding `branch.<name>` configuration
/// should also be removed. Missing refs are accepted and still have their configuration cleaned
/// up; the return value reflects whether the ref existed when locked for deletion,
/// not whether configuration changed.
/// A branch checked out in any worktree causes a precondition error. Configuration-cleanup
/// failures after ref deletion are logged and treated as success; use
/// [`gix::Repository::delete_local_branches()`] directly to handle those failures explicitly.
///
/// Use [`but_core::branch::SafeDelete`] for ref-only deletion, including non-branch refs,
/// or for a checkout preflight. It leaves branch configuration intact and reports a checked-out
/// ref through its outcome rather than an error. Its worktree information is captured at
/// construction, whereas this function checks worktrees on each call.
///
/// Unlike `SafeDelete`, which requires the ref's target to match the supplied reference,
/// this function deletes by name even if the target has changed since the caller inspected it.
/// Use [`remove_reference()`] when workspace eligibility, metadata cleanup, and rebuilding the
/// workspace are also required.
pub fn delete_local_branch(
    repo: &mut gix::Repository,
    ref_name: &gix::refs::FullNameRef,
) -> anyhow::Result<bool> {
    let deleted = match repo.delete_local_branches([ref_name.to_owned()]) {
        Ok(deleted) => deleted,
        Err(gix::repository::branch::delete::Error::Cleanup {
            deleted, source, ..
        }) => {
            tracing::warn!(
                ?source,
                ?ref_name,
                "branch was deleted but its local configuration remains"
            );
            deleted
        }
        Err(gix::repository::branch::delete::Error::CheckedOut { worktree_dirs, .. }) => {
            bail_precondition!(
                "Refusing to delete a branch that is checked out. Worktrees are: {worktree_dirs:?}"
            )
        }
        Err(err) => return Err(err.into()),
    };
    Ok(!deleted.is_empty())
}

/// Remove the workspace reference `ref_name` (if it still exists),
/// possibly along with its `meta`-data.
/// The `workspace` is used to assure the `ref_name` is eligible for deletion in the first place.
/// It's not an error if `ref_name` can't be found.
/// Note that the `workspace` will be stale after deleting the reference successfully.
///
/// Return the updated graph that reflects this change, or `None` if nothing changed.
pub fn remove_reference(
    ref_name: &gix::refs::FullNameRef,
    repo: &mut gix::Repository,
    workspace: &but_graph::Workspace,
    meta: &mut impl RefMetadata,
    Options {
        avoid_anonymous_stacks,
        keep_metadata,
    }: Options,
) -> anyhow::Result<Option<but_graph::Workspace>> {
    // We assume the stack-idx can't change by deleting
    let Some((stack, _segment)) = workspace.find_segment_and_stack_by_refname(ref_name) else {
        return Ok(None);
    };

    if avoid_anonymous_stacks
        && (stack
            .segments
            .iter()
            .map(|s| s.commits.len())
            .sum::<usize>()
            > 0
            && stack
                .segments
                .iter()
                .filter(|s| s.ref_info.is_some())
                .count()
                < 2)
    {
        bail!(
            "Refusing to delete last named segment '{}' as it would leave an anonymous segment",
            ref_name.shorten()
        );
    }

    let deleted_ref = delete_local_branch(repo, ref_name)?;

    let deleted_meta = if keep_metadata {
        false
    } else {
        meta.remove(ref_name)?
    };

    // Unlikely, hard to test, but can happen.
    if !deleted_ref && !deleted_meta {
        return Ok(None);
    }

    let stack_id = stack.id;
    let mut graph = workspace
        .graph
        .redo_traversal_with_overlay(repo, meta, Default::default())?;
    let ws = graph.into_workspace()?;
    if avoid_anonymous_stacks {
        let Some(stack) = ws.stacks.iter().find(|s| s.id == stack_id) else {
            // The whole stack is gone, so nothing that could be anonymous.
            return Ok(Some(ws));
        };
        if avoid_anonymous_stacks
            && let Some(commit) = stack
                .segments
                .first()
                .and_then(|s| s.commits.first().filter(|_| s.ref_info.is_none()))
        {
            let (name_of_segment_below, target_id) = stack
                .segments
                .iter()
                .find_map(|s| {
                    let rn = s.ref_name()?;
                    ws.tip_commit_by_segment_id(s.id).map(|c| (rn, c.id))
                })
                .with_context(|| {
                    "BUG: should not try to delete branch if anon \
                    segments aren't allows and there is no named segment left"
                })?;

            repo.reference(
                name_of_segment_below,
                commit.id,
                PreviousValue::MustExistAndMatch(gix::refs::Target::Object(target_id)),
                "move segment reference up to avoid anonymous stack",
            )?;
            graph = ws
                .graph
                .redo_traversal_with_overlay(repo, meta, Default::default())?;
            Ok(Some(graph.into_workspace()?))
        } else {
            Ok(Some(ws))
        }
    } else {
        Ok(Some(ws))
    }
}
