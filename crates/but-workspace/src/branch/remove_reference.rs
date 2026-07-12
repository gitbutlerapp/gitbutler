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

/// Remove the workspace reference `ref_name` (if it still exists),
/// possibly along with its `meta`-data.
/// The `workspace` is used to assure the `ref_name` is eligible for deletion in the first place.
/// It's not an error if `ref_name` can't be found.
/// Note that the `workspace` will be stale after deleting the reference successfully.
///
/// Return the updated substrate that reflects this change, or `None` if nothing changed.
/// Display callers materialize via
/// [`display_stacks`](but_graph::Workspace::display_stacks).
pub fn remove_reference(
    ref_name: &gix::refs::FullNameRef,
    repo: &gix::Repository,
    workspace: &but_graph::Workspace,
    meta: &mut impl RefMetadata,
    Options {
        avoid_anonymous_stacks,
        keep_metadata,
    }: Options,
) -> anyhow::Result<Option<but_graph::Workspace>> {
    // We assume the stack-idx can't change by deleting
    #[cfg(debug_assertions)]
    but_graph::declared::debug_assert_declared_branch_is_visible(
        workspace,
        ref_name,
        workspace.find_branch(ref_name).map(|(stack, _)| stack.id),
    );
    let Some((stack, _segment)) = workspace.find_branch(ref_name) else {
        return Ok(None);
    };

    if avoid_anonymous_stacks
        && (stack.segments.iter().any(|s| s.tip().is_some())
            && stack
                .segments
                .iter()
                .filter(|s| s.ref_name.is_some())
                .count()
                < 2)
    {
        bail!(
            "Refusing to delete last named segment '{}' as it would leave an anonymous segment",
            ref_name.shorten()
        );
    }

    let deleted_ref = if let Some(r) = repo.try_find_reference(ref_name)? {
        let safe = but_core::branch::SafeDelete::new(repo)?;
        let out = safe.delete_reference(&r)?;
        if let Some(paths) = out.checked_out_in_worktree_dirs {
            bail_precondition!(
                "Refusing to delete a branch that is checked out. Worktrees are: {paths:?}"
            );
        }
        true
    } else {
        false
    };

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
    let ws = workspace.rederive_with(repo, meta, Default::default())?;
    if avoid_anonymous_stacks {
        let Some(stack) = ws.stack_by_id(stack_id) else {
            // The whole stack is gone, so nothing that could be anonymous.
            return Ok(Some(ws));
        };
        if avoid_anonymous_stacks
            && let Some(commit) = stack
                .top()
                .and_then(|s| s.tip().filter(|_| s.ref_name.is_none()))
        {
            // The first named segment below the anonymous tip and its resting commit,
            // computed on this same view stack (not a display re-lookup).
            let (name_of_segment_below, target_id) = stack
                .segments
                .iter()
                .enumerate()
                .find_map(|(idx, s)| {
                    let rn = s.ref_name()?;
                    let resting = stack
                        .segments_at_or_below(idx)
                        .iter()
                        .find_map(|seg| seg.tip())
                        .or(stack.base)?;
                    Some((rn, resting))
                })
                .with_context(|| {
                    "BUG: should not try to delete branch if anon \
                    segments aren't allows and there is no named segment left"
                })?;

            repo.reference(
                name_of_segment_below,
                commit,
                PreviousValue::MustExistAndMatch(gix::refs::Target::Object(target_id)),
                "move segment reference up to avoid anonymous stack",
            )?;
            Ok(Some(ws.rederive_with(repo, meta, Default::default())?))
        } else {
            Ok(Some(ws))
        }
    } else {
        Ok(Some(ws))
    }
}
