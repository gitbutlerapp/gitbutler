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

use anyhow::Context as _;
use but_core::RefMetadata;
use but_error::bail_precondition;
use gix::refs::transaction::PreviousValue;

use crate::workspace::find_segment_and_stack;

/// Remove the workspace reference `ref_name` (if it still exists),
/// possibly along with its `meta`-data.
/// The `workspace` is used to assure the `ref_name` is eligible for deletion in the first place.
/// It's not an error if `ref_name` can't be found.
/// Note that the `workspace` will be stale after deleting the reference successfully.
///
/// Return the updated graph that reflects this change, or `None` if nothing changed.
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
    let project_meta = workspace.graph.project_meta().clone();
    let (mut entrypoint_id, mut entrypoint_ref) = match workspace.graph.entrypoint() {
        but_graph::NodeGraphEntrypoint::Node(index) => {
            let Some(node) = workspace.graph.nodes().get(*index) else {
                unreachable!("born graph entrypoints are valid node indices")
            };
            match node.kind() {
                but_graph::NodeKind::Commit { id } => {
                    (*id, workspace.graph.entrypoint_ref().map(ToOwned::to_owned))
                }
                _ => unreachable!("born graph entrypoints are commits"),
            }
        }
        but_graph::NodeGraphEntrypoint::Unborn(_) => return Ok(None),
    };
    // We assume the stack-idx can't change by deleting
    let Some((stack, _segment)) = find_segment_and_stack(workspace, ref_name) else {
        return Ok(None);
    };

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
    if entrypoint_ref
        .as_ref()
        .is_some_and(|name| name.as_ref() == ref_name)
    {
        entrypoint_ref = None;
    }

    let stack_id = stack.id;
    let mut graph = but_graph::Graph::from_repo(
        repo,
        meta,
        project_meta.clone(),
        but_graph::init::Overlay::default().with_entrypoint(entrypoint_id, entrypoint_ref.clone()),
    )?;
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
                    let ref_info = s.ref_info.as_ref()?;
                    ref_info
                        .commit_id
                        .map(|commit_id| (ref_info.ref_name.as_ref(), commit_id))
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
            if entrypoint_ref
                .as_ref()
                .is_some_and(|name| name.as_ref() == name_of_segment_below)
            {
                entrypoint_id = commit.id;
            }
            graph = but_graph::Graph::from_repo(
                repo,
                meta,
                project_meta,
                but_graph::init::Overlay::default().with_entrypoint(entrypoint_id, entrypoint_ref),
            )?;
            Ok(Some(graph.into_workspace()?))
        } else {
            Ok(Some(ws))
        }
    } else {
        Ok(Some(ws))
    }
}
