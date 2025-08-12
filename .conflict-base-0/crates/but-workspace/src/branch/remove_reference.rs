/// Options for altering how [remove_reference()](function::remove_reference()) works.
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

pub(crate) mod function {
    use super::Options;
    use anyhow::{Context, bail};
    use but_core::RefMetadata;
    use but_graph::Graph;
    use gix::refs::transaction::PreviousValue;

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
        workspace: &but_graph::projection::Workspace<'_>,
        meta: &mut impl RefMetadata,
        Options {
            avoid_anonymous_stacks,
            keep_metadata,
        }: Options,
    ) -> anyhow::Result<Option<Graph>> {
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
            r.delete()?;
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
        let mut graph =
            workspace
                .graph
                .redo_traversal_with_overlay(repo, meta, Default::default())?;
        if avoid_anonymous_stacks {
            let workspace = graph.to_workspace()?;
            let Some(stack) = workspace.stacks.iter().find(|s| s.id == stack_id) else {
                // The whole stack is gone, so nothing that could be anonymous.
                return Ok(Some(graph));
            };
            if avoid_anonymous_stacks {
                if let Some(commit) = stack
                    .segments
                    .first()
                    .and_then(|s| s.commits.first().filter(|_| s.ref_name.is_none()))
                {
                    let (name_of_segment_below, target_id) = stack
                        .segments
                        .iter()
                        .find_map(|s| {
                            let rn = s.ref_name.as_ref()?;
                            workspace.graph.tip_skip_empty(s.id).map(|c| (rn, c.id))
                        })
                        .with_context(|| {
                            "BUG: should not try to delete branch if anon \
                    segments aren't allows and there is no named segment left"
                        })?;

                    repo.reference(
                        name_of_segment_below.as_ref(),
                        commit.id,
                        PreviousValue::MustExistAndMatch(gix::refs::Target::Object(target_id)),
                        "move segment reference up to avoid anonymous stack",
                    )?;
                    graph = graph.redo_traversal_with_overlay(repo, meta, Default::default())?;
                }
            }
        }

        Ok(Some(graph))
    }
}
