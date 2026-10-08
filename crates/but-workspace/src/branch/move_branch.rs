use but_core::RefMetadata;
use but_rebase::graph_rebase::SuccessfulRebase;

/// Outcome of moving branches between or out of stacks.
///
/// Returned by [function::move_branch()].
#[derive(Debug)]
pub struct Outcome<'ws, 'meta, M: RefMetadata> {
    /// A successful rebase result for continuing operations.
    pub rebase: SuccessfulRebase<'ws, 'meta, M>,
    /// The updated workspace metadata that accompanies the move operation.
    /// It should replace the actual workspace metadata to configure moved 'virtual' branches segments, if `Some()`.
    pub ws_meta: Option<but_core::ref_metadata::Workspace>,
    /// In single-branch (ad-hoc) mode, set to the reference that should become the new tip after the
    /// reorder. This can be the subject when it moves above the current tip, or the branch now above
    /// it when the checked-out tip moves down. Materializing moves `HEAD` there only when the
    /// subject was inserted directly above the checked-out branch, so the caller is responsible for
    /// checking this out to keep the whole reordered stack projected (mirroring
    /// [`create_reference`](crate::branch::create_reference())). `None` when the tip is unchanged.
    pub new_tip: Option<gix::refs::FullName>,
    /// In single-branch (ad-hoc) mode, the reordered tip-to-base branch chain that the caller should
    /// persist with [`RefMetadata::set_branch_stack_order`].
    /// It is returned rather than written here so callers can apply it only for real runs and skip
    /// persistence for dry-run previews. `None` outside single-branch mode.
    pub branch_stack_order: Option<Vec<gix::refs::FullName>>,
}

pub(super) mod function {

    use but_core::RefMetadata;
    use but_core::ref_metadata::StackId;
    use but_rebase::graph_rebase::mutate::SomeSelectors;

    use crate::graph_manipulation::DisconnectParameters;
    use crate::graph_manipulation::get_disconnect_parameters;
    use crate::graph_manipulation::traverse_nodes;

    use super::Outcome;
    use anyhow::Context;
    use anyhow::bail;
    use but_graph::workspace::{Lane, StackSegment, WorkspaceKind};
    use but_rebase::graph_rebase::Editor;
    use but_rebase::graph_rebase::SuccessfulRebase;
    use but_rebase::graph_rebase::ToSelector;
    use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
    use gix::refs::FullNameRef;

    /// Remove a branch out of a stack, creating a new stack out of it, in memory.
    ///
    /// `editor` is assumed to have been generated from the given `workspace`
    /// and therefore aligned.
    ///
    /// `workspace` - Used for getting the surrounding context of the branch being torn off.
    ///     In the future, we should not rely on the projection and do it fully on the graph.
    ///
    /// `subject_branch_name` - The branch to take out of a stack.
    ///
    /// Returns the in memory update [outcome](Outcome) that can then used for materialisation.
    pub fn tear_off_branch<'ws, 'meta, M: RefMetadata>(
        editor: Editor<'ws, 'meta, M>,
        subject_branch_name: &FullNameRef,
    ) -> anyhow::Result<Outcome<'ws, 'meta, M>> {
        let mut successful_rebase = editor.rebase()?;
        let workspace = successful_rebase.overlayed_graph()?.into_workspace()?;
        let existing_order = if matches!(workspace.kind, WorkspaceKind::AdHoc) {
            let (_, meta) = successful_rebase.repo_and_meta_mut();
            meta.branch_stack_order(subject_branch_name)?
        } else {
            None
        };
        let mut editor = successful_rebase.into_editor();
        let Some(source) = workspace.find_segment_and_stack_by_refname(subject_branch_name) else {
            bail!(
                "Couldn't find branch to move in workspace with reference name: {subject_branch_name}"
            );
        };

        let managed = match &workspace.kind {
            WorkspaceKind::Managed { .. } => true,
            WorkspaceKind::ManagedMissingWorkspaceCommit { .. } => {
                bail!("Moving branches currently need a workspace commit")
            }
            WorkspaceKind::AdHoc => false,
        };

        let (source_stack, subject_segment) = source;

        if source_stack.segments.len() == 1 {
            // There's only one branch in the source stack. Nothing to do.
            return Ok(Outcome {
                rebase: editor.rebase()?,
                ws_meta: None,
                new_tip: None,
                branch_stack_order: None,
            });
        }

        let workspace_head = if managed {
            Some(
                workspace
                    .tip_commit()
                    .context("Couldn't find workspace head.")?
                    .id,
            )
        } else {
            None
        };
        let target_selector = if managed {
            let lower_bound_ref = workspace
                .lower_bound_segment_id
                .map(|segment_id| &workspace.graph[segment_id])
                .and_then(|segment| segment.ref_name())
                .context("Tearing off a branch requires a workspace common base")?;
            editor
                .select_reference(lower_bound_ref)
                .context("Failed to find target reference in graph.")?
        } else {
            editor.select_commit(workspace.graph.project_meta.target_commit_id_or_err()?)?
        };

        let DisconnectParameters {
            delimiter: subject_delimiter,
            children_to_disconnect,
            parents_to_disconnect,
        } = get_disconnect_parameters(
            &editor,
            &source_stack.segments,
            subject_segment,
            workspace_head,
        )?;

        editor.disconnect_segment_from(
            subject_delimiter.clone(),
            children_to_disconnect,
            parents_to_disconnect,
            false,
        )?;

        let branch_stack_order = if let Some(workspace_head) = workspace_head {
            let head_selector = editor.select_commit(workspace_head)?;
            let selectors = SomeSelectors::new(Vec::from([head_selector]))?;
            editor.insert_segment_into(
                target_selector,
                subject_delimiter,
                but_rebase::graph_rebase::mutate::InsertSide::Above,
                Some(selectors),
                but_rebase::graph_rebase::mutate::ParentReparentingOrder::Prepend,
            )?;
            None
        } else {
            // There is no workspace commit to connect to the torn-off branch. Attach only
            // its first parent at the target, leaving the remaining stack independent.
            editor.add_edge(subject_delimiter.parent, target_selector, 0)?;
            Some(
                existing_order
                    .unwrap_or_else(|| stack_branch_order(source_stack))
                    .into_iter()
                    .filter(|name| name.as_ref() != subject_branch_name)
                    .collect(),
            )
        };

        Ok(Outcome {
            rebase: editor.rebase()?,
            ws_meta: None,
            new_tip: None,
            branch_stack_order,
        })
    }

    /// Move a branch to `side` of `relative_to`, within its lane or into another one.
    ///
    /// A lane is a stack of the workspace or the history a linked worktree owns.
    ///
    /// `editor` is assumed to have been generated from the workspace the branch lives in.
    ///
    /// `subject_branch_name` is the full reference name of the branch to move.
    ///
    /// `relative_to` and `side` name where it goes, as they do for commits:
    /// - above a reference, the branch sits directly on top of that branch.
    /// - below a reference, it takes over the commits of that branch, which stays as empty
    ///   branch on top of it.
    /// - above or below a commit, it splits the branch owning that commit there, taking over
    ///   everything beneath it.
    ///
    /// Moving the branch a linked worktree has checked out leaves that worktree on what the
    /// branch was based on: the branch below it, or else that commit with a detached `HEAD`.
    /// A branch placed directly above what a linked worktree has checked out, be it a branch or
    /// the commit of a detached `HEAD`, becomes what that worktree has checked out.
    ///
    /// Currently, this looks into the workspace projection in order to determine **where to take the branch from**.
    ///
    /// ### The issue
    /// It's impossible to know for sure what is the exact intention of 'moving a branch' inside a complex git graph.
    /// Any commit, can have N children and M parents. 'Moving' it somewhere else can imply:
    /// - Disconnecting all parents and children, and inserting it somewhere else.
    /// - Disconnecting the first parent and all children, and then inserting.
    /// - Disconnecting *some* parents and *some* children, and then inserting it.
    ///
    /// This condition holds for every commit in a branch.
    ///
    /// ### The GitButler assumption
    /// In the context of a GitButler workspace (as of this writing), we want to disconnect the branch (segment) from
    /// its lane, and insert it into another. In graph terms, this means that we:
    /// - Disconnect the reference node from the base segment (the branch under the subject or the target base)
    /// - Disconnect the last commit node of the child segment (the branch over the subject or the workspace commit)
    /// - Nothing else. Other parentage and children are kept, since this is what we care about in a GB workspace world.
    ///
    /// ### What the future holds
    /// In the future, where we're not afraid of complex graphs, we've figured out UX and data wrangling,
    /// the concept of a segment might not hold, and hence we'll have to figure out a better way of determining
    /// what to cut (e.g. letting the clients decide what to cut).
    ///
    /// Returns an [outcome](Outcome) for potential materialisation.
    pub fn move_branch<'ws, 'meta, M: RefMetadata>(
        editor: Editor<'ws, 'meta, M>,
        subject_branch_name: &FullNameRef,
        relative_to: RelativeTo,
        side: InsertSide,
    ) -> anyhow::Result<Outcome<'ws, 'meta, M>> {
        let successful_rebase = editor.rebase()?;
        let workspace = successful_rebase.overlayed_graph()?.into_workspace()?;

        let Some(source) = workspace.find_segment_and_lane_by_refname(subject_branch_name) else {
            bail!(
                "Couldn't find branch to move in workspace with reference name: {subject_branch_name}"
            );
        };
        let anchor = Anchor::resolve(&workspace, &relative_to, side)?;
        if anchor.segment.id == source.1.id {
            bail!("Cannot move branch {subject_branch_name} onto itself");
        }

        // Each kind of workspace has a very different notion of what "moving a branch" means, so we
        // dispatch into a dedicated handler for each one.
        match &workspace.kind {
            WorkspaceKind::AdHoc => move_branch_in_single_branch_mode(
                successful_rebase,
                &workspace,
                source,
                anchor,
                subject_branch_name,
                relative_to,
                side,
            ),
            WorkspaceKind::ManagedMissingWorkspaceCommit { .. } => {
                bail!("Moving branches currently need a workspace commit")
            }
            WorkspaceKind::Managed { .. } => move_branch_in_managed_workspace(
                successful_rebase,
                &workspace,
                source,
                anchor,
                subject_branch_name,
                relative_to,
                side,
            ),
        }
    }

    /// Move a branch in a single-branch (ad-hoc) workspace, where `HEAD` is on a plain local branch.
    ///
    /// In single-branch (ad-hoc) mode there is no workspace commit, and the tip-to-base order of
    /// branches lives in the `branch_order` metadata table rather than in workspace metadata. Empty
    /// branches can therefore move through metadata alone when their refs already share a target,
    /// while branches with commits or empty branches crossing commits also require a graph rewrite.
    /// The reordered chain is returned in [`Outcome::branch_stack_order`] for the caller to persist
    /// (via [`RefMetadata::set_branch_stack_order`]) rather than being written here, so callers can
    /// skip persistence for dry-run previews.
    fn move_branch_in_single_branch_mode<'ws, 'meta, M: RefMetadata>(
        mut successful_rebase: SuccessfulRebase<'ws, 'meta, M>,
        workspace: &but_graph::Workspace,
        source: SegmentInLane<'_>,
        anchor: Anchor<'_>,
        subject_branch_name: &FullNameRef,
        relative_to: RelativeTo,
        side: InsertSide,
    ) -> anyhow::Result<Outcome<'ws, 'meta, M>> {
        let ((Lane::Stack(source_stack), subject_segment), Lane::Stack(destination_stack)) =
            (source, anchor.lane)
        else {
            bail!(
                "Moving a branch between a worktree and the workspace in single-branch mode is not yet supported"
            );
        };
        let anchor_branch_name = anchor
            .segment
            .ref_name()
            .context("Target segment doesn't have a ref")?;
        let entrypoint = workspace.ref_name().map(ToOwned::to_owned);
        // A branch that owns commits can only be reordered within its current stack in
        // single-branch mode. Moving it across stacks would change commit ownership and needs a
        // real rebase.
        if !subject_segment.commits.is_empty() && !same_stack(source_stack, destination_stack) {
            bail!("Moving a non-empty branch in single-branch mode is not yet supported");
        }
        // Reordering same-target empty refs only changes which empty segment is displayed first.
        // If their targets differ, however, the subject crosses commit-owning segments and its ref
        // must move with it or those commits would be projected as belonging to the empty branch.
        let move_requires_graph_update = !subject_segment.commits.is_empty()
            || match &relative_to {
                RelativeTo::Reference(name) => {
                    successful_rebase.reference_target(subject_branch_name)?
                        != successful_rebase.reference_target(name.as_ref())?
                }
                RelativeTo::Commit(_) => true,
            };
        let existing_order = {
            let (_repo, meta) = successful_rebase.repo_and_meta_mut();
            if !meta.can_persist_branch_stack_order() {
                bail!(
                    "Cannot reorder '{subject_branch_name}' in single-branch mode without branch order metadata"
                );
            }
            // Reorder against the existing chain. A movable subject is always part of `branch_order`
            // (that's what makes it a projected segment), so the first lookup normally succeeds. The
            // target and entrypoint lookups are defensive fallbacks so that, should the projection ever
            // surface a segment that isn't tracked yet, we extend the real chain instead of clobbering
            // it down to just the moved refs.
            match meta.branch_stack_order(subject_branch_name)? {
                Some(order) => order,
                None => match meta.branch_stack_order(anchor_branch_name)? {
                    Some(order) => order,
                    None => entrypoint
                        .as_ref()
                        .map(|entrypoint| meta.branch_stack_order(entrypoint.as_ref()))
                        .transpose()?
                        .flatten()
                        .unwrap_or_else(|| stack_branch_order(source_stack)),
                },
            }
        };
        let previous_order = existing_order.clone();
        let new_order = reorder_branch_in_stack_order(
            existing_order,
            anchor_branch_name,
            anchor.side,
            subject_branch_name,
        );

        // Keep HEAD at the top of the reordered portion of the stack. This is the subject when it
        // moves above the current entrypoint, or the branch that moves above the subject when the
        // checked-out top branch moves down.
        let new_tip = reordered_entrypoint(
            entrypoint.as_ref().map(|name| name.as_ref()),
            source_stack,
            &new_order,
        );

        if new_order == previous_order {
            return Ok(Outcome {
                rebase: successful_rebase,
                ws_meta: None,
                new_tip,
                branch_stack_order: Some(new_order),
            });
        }

        if move_requires_graph_update {
            let mut editor = successful_rebase.into_editor();
            let target_selector = relative_to
                .to_selector(&editor)
                .context("Failed to find target in graph.")?;

            let DisconnectParameters {
                delimiter: subject_delimiter,
                children_to_disconnect,
                parents_to_disconnect,
            } = get_disconnect_parameters(&editor, &source_stack.segments, subject_segment, None)?;

            editor.disconnect_segment_from(
                subject_delimiter.clone(),
                children_to_disconnect,
                parents_to_disconnect,
                false,
            )?;
            editor.insert_segment(target_selector, subject_delimiter, side)?;

            return Ok(Outcome {
                rebase: editor.rebase()?,
                ws_meta: None,
                new_tip,
                branch_stack_order: Some(new_order),
            });
        }

        Ok(Outcome {
            rebase: successful_rebase,
            ws_meta: None,
            new_tip,
            branch_stack_order: Some(new_order),
        })
    }

    /// Move a branch within a managed workspace (one backed by a workspace commit).
    fn move_branch_in_managed_workspace<'ws, 'meta, M: RefMetadata>(
        successful_rebase: SuccessfulRebase<'ws, 'meta, M>,
        workspace: &but_graph::Workspace,
        source: SegmentInLane<'_>,
        anchor: Anchor<'_>,
        subject_branch_name: &FullNameRef,
        relative_to: RelativeTo,
        side: InsertSide,
    ) -> anyhow::Result<Outcome<'ws, 'meta, M>> {
        let Some(workspace_head) = workspace.tip_commit().map(|commit| commit.id) else {
            bail!("Couldn't find workspace head.")
        };

        let mut ws_meta = workspace.metadata.clone();

        let (source_lane, subject_segment) = source;
        if let Lane::Worktree(worktree) = anchor.lane
            && subject_segment.commits.is_empty()
        {
            bail!(
                "Cannot place empty branch '{}' in worktree '{}': branches can't be ordered in worktrees yet",
                subject_branch_name.shorten(),
                worktree.name
            );
        }
        if matches!(
            (source_lane, anchor.lane, &relative_to),
            (Lane::Stack(_), Lane::Stack(_), RelativeTo::Reference(_))
        ) && subject_segment.commits.is_empty()
            && anchor.segment.commits.is_empty()
            && ws_meta.is_some()
        {
            if let Some(ws_meta) = ws_meta.as_mut() {
                move_branch_in_metadata(ws_meta, subject_branch_name, &anchor);
            }
            return Ok(Outcome {
                rebase: successful_rebase,
                ws_meta,
                new_tip: None,
                branch_stack_order: None,
            });
        }

        let mut editor = successful_rebase.into_editor();
        let target_selector = relative_to
            .to_selector(&editor)
            .context("Failed to find target in graph.")?;

        let DisconnectParameters {
            delimiter: subject_delimiter,
            children_to_disconnect,
            parents_to_disconnect,
        } = get_disconnect_parameters(
            &editor,
            source_lane.segments(),
            subject_segment,
            match source_lane {
                Lane::Stack(_) => Some(workspace_head),
                Lane::Worktree(_) => None,
            },
        )?;

        let skip_reconnect_step = source_lane.segments().len() == 1;
        editor.disconnect_segment_from(
            subject_delimiter.clone(),
            children_to_disconnect,
            parents_to_disconnect,
            skip_reconnect_step,
        )?;
        if let Lane::Worktree(worktree) = source_lane
            && is_tip_of(source_lane, subject_segment)
        {
            let based_on = match worktree.segments.get(1).and_then(|below| below.ref_name()) {
                Some(branch_below) => editor.select_reference(branch_below)?,
                None => editor.select_commit(subject_segment.base.with_context(|| {
                    format!(
                        "Cannot move '{}': worktree '{}' would have nothing left to check out",
                        subject_branch_name.shorten(),
                        worktree.name
                    )
                })?)?,
            };
            editor.set_worktree_checkout(gix::bstr::BStr::new(&worktree.name), based_on)?;
        }
        if traverse_nodes(&editor, target_selector)?.contains(&subject_delimiter.parent) {
            bail!(
                "Cannot move '{}' {}, which builds on it",
                subject_branch_name.shorten(),
                Location(&relative_to, side),
            );
        }
        editor.insert_segment(target_selector, subject_delimiter, side)?;

        // Keep workspace metadata aligned with the graph move outcome: we remove the subject
        // branch from its current location and reinsert it next to the anchor.
        // A branch that left for a worktree leaves its stack when the move is materialized.
        let ws_meta = match anchor.lane {
            Lane::Stack(_) => ws_meta.map(|mut ws_meta| {
                move_branch_in_metadata(&mut ws_meta, subject_branch_name, &anchor);
                ws_meta
            }),
            Lane::Worktree(_) => None,
        };

        Ok(Outcome {
            rebase: editor.rebase()?,
            ws_meta,
            new_tip: None,
            branch_stack_order: None,
        })
    }

    /// A segment and the lane holding it.
    type SegmentInLane<'a> = (Lane<'a>, &'a StackSegment);

    fn is_tip_of(lane: Lane<'_>, segment: &StackSegment) -> bool {
        lane.segments()
            .first()
            .is_some_and(|tip| tip.id == segment.id)
    }

    /// The segment a moved branch is placed next to, and the side of it the branch ends up on
    /// among the segments of its lane.
    ///
    /// On either side of a commit, the branch sits below what remains of that commit's segment.
    struct Anchor<'a> {
        lane: Lane<'a>,
        segment: &'a StackSegment,
        side: InsertSide,
    }

    impl<'a> Anchor<'a> {
        fn resolve(
            workspace: &'a but_graph::Workspace,
            relative_to: &RelativeTo,
            side: InsertSide,
        ) -> anyhow::Result<Self> {
            match relative_to {
                RelativeTo::Reference(name) => {
                    let (lane, segment) = workspace
                        .find_segment_and_lane_by_refname(name.as_ref())
                        .with_context(|| {
                            format!(
                                "Couldn't find target branch to move in workspace with reference name: {name}"
                            )
                        })?;
                    Ok(Anchor {
                        lane,
                        segment,
                        side,
                    })
                }
                RelativeTo::Commit(id) => {
                    let (lane, segment) = workspace
                        .find_segment_and_lane_by_commit_id(*id)
                        .with_context(|| format!("Commit {id} isn't part of the workspace"))?;
                    Ok(Anchor {
                        lane,
                        segment,
                        side: InsertSide::Below,
                    })
                }
            }
        }
    }

    /// A `side` of `relative_to`, for showing to the user.
    struct Location<'a>(&'a RelativeTo, InsertSide);

    impl std::fmt::Display for Location<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let side = match self.1 {
                InsertSide::Above => "above",
                InsertSide::Below => "below",
            };
            match self.0 {
                RelativeTo::Reference(name) => write!(f, "{side} '{}'", name.shorten()),
                RelativeTo::Commit(id) => write!(f, "{side} commit {}", id.to_hex_with_len(7)),
            }
        }
    }

    fn same_stack(left: &but_graph::workspace::Stack, right: &but_graph::workspace::Stack) -> bool {
        left.segments.len() == right.segments.len()
            && left
                .segments
                .iter()
                .zip(&right.segments)
                .all(|(left, right)| left.id == right.id)
    }

    fn stack_branch_order(stack: &but_graph::workspace::Stack) -> Vec<gix::refs::FullName> {
        stack
            .segments
            .iter()
            .filter_map(|segment| segment.ref_name().map(ToOwned::to_owned))
            .collect()
    }

    fn reordered_entrypoint(
        entrypoint: Option<&FullNameRef>,
        stack: &but_graph::workspace::Stack,
        new_order: &[gix::refs::FullName],
    ) -> Option<gix::refs::FullName> {
        let entrypoint = entrypoint?;
        let new_entrypoint = new_order.iter().find(|candidate| {
            stack
                .segments
                .iter()
                .any(|segment| segment.ref_name() == Some(candidate.as_ref()))
        })?;
        (new_entrypoint.as_ref() != entrypoint).then(|| new_entrypoint.clone())
    }

    /// Reorder `subject` to sit directly on `side` of `target` in the tip-to-base ad-hoc `order`.
    ///
    /// Mirrors `create_reference`'s `insert_into_branch_stack_order`: `subject` is removed and
    /// re-inserted at `target`'s slot, pushing `target` (and everything below it) down, or right
    /// after it.
    ///
    /// If `target` isn't tracked yet (stale or empty metadata) it is appended first, so that a move
    /// where *both* branches are missing adds them both - `subject` next to `target` - instead of
    /// silently clobbering the rest of the ordering down to just `subject`.
    fn reorder_branch_in_stack_order(
        mut order: Vec<gix::refs::FullName>,
        target_branch_name: &FullNameRef,
        side: InsertSide,
        subject_branch_name: &FullNameRef,
    ) -> Vec<gix::refs::FullName> {
        order.retain(|branch| branch.as_ref() != subject_branch_name);
        let target_idx = match order
            .iter()
            .position(|branch| branch.as_ref() == target_branch_name)
        {
            Some(idx) => idx,
            None => {
                order.push(target_branch_name.to_owned());
                order.len() - 1
            }
        };
        order.insert(
            match side {
                InsertSide::Above => target_idx,
                InsertSide::Below => target_idx + 1,
            },
            subject_branch_name.to_owned(),
        );
        order
    }

    fn move_branch_in_metadata(
        ws_meta: &mut but_core::ref_metadata::Workspace,
        subject_branch_name: &FullNameRef,
        anchor: &Anchor<'_>,
    ) {
        ws_meta.remove_segment(subject_branch_name);
        let inserted = anchor
            .segment
            .ref_name()
            .and_then(|anchor_branch_name| match anchor.side {
                InsertSide::Above => ws_meta.insert_new_segment_above_anchor_if_not_present(
                    subject_branch_name,
                    anchor_branch_name,
                ),
                InsertSide::Below => ws_meta.insert_new_segment_below_anchor_if_not_present(
                    subject_branch_name,
                    anchor_branch_name,
                ),
            });
        if inserted.is_none() {
            // If metadata doesn't know the anchor (stale metadata),
            // keep the moved branch represented as a stack tip.
            ws_meta.add_or_insert_new_stack_if_not_present(
                subject_branch_name,
                None,
                but_core::ref_metadata::WorkspaceCommitRelation::Merged,
                |_| StackId::generate(),
            );
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{InsertSide, reorder_branch_in_stack_order};

        fn r(name: &str) -> gix::refs::FullName {
            gix::refs::FullName::try_from(name).expect("valid ref name")
        }

        fn names(order: &[gix::refs::FullName]) -> Vec<String> {
            order.iter().map(|n| n.to_string()).collect()
        }

        #[test]
        fn moves_subject_on_top_of_target_when_both_present() {
            let order = vec![r("refs/heads/main"), r("refs/heads/a"), r("refs/heads/b")];
            let new = reorder_branch_in_stack_order(
                order,
                r("refs/heads/main").as_ref(),
                InsertSide::Above,
                r("refs/heads/b").as_ref(),
            );
            // `b` moves directly above `main`, `a` shifts down.
            assert_eq!(
                names(&new),
                ["refs/heads/b", "refs/heads/main", "refs/heads/a"]
            );
        }

        #[test]
        fn moves_subject_right_below_target() {
            let order = vec![r("refs/heads/a"), r("refs/heads/b"), r("refs/heads/main")];
            let new = reorder_branch_in_stack_order(
                order,
                r("refs/heads/b").as_ref(),
                InsertSide::Below,
                r("refs/heads/a").as_ref(),
            );
            assert_eq!(
                names(&new),
                ["refs/heads/b", "refs/heads/a", "refs/heads/main"]
            );
        }

        #[test]
        fn adds_subject_above_target_when_only_target_is_present() {
            let order = vec![r("refs/heads/main")];
            let new = reorder_branch_in_stack_order(
                order,
                r("refs/heads/main").as_ref(),
                InsertSide::Above,
                r("refs/heads/new").as_ref(),
            );
            assert_eq!(names(&new), ["refs/heads/new", "refs/heads/main"]);
        }

        #[test]
        fn adds_both_in_order_when_neither_is_present() {
            // Stale/empty metadata: neither branch is tracked yet. Both are added, subject on top of
            // target, without dropping any pre-existing ordering.
            let order = vec![r("refs/heads/main")];
            let new = reorder_branch_in_stack_order(
                order,
                r("refs/heads/target").as_ref(),
                InsertSide::Above,
                r("refs/heads/subject").as_ref(),
            );
            assert_eq!(
                names(&new),
                ["refs/heads/main", "refs/heads/subject", "refs/heads/target"]
            );
        }

        #[test]
        fn adds_both_in_order_from_empty_metadata() {
            let new = reorder_branch_in_stack_order(
                Vec::new(),
                r("refs/heads/target").as_ref(),
                InsertSide::Above,
                r("refs/heads/subject").as_ref(),
            );
            assert_eq!(names(&new), ["refs/heads/subject", "refs/heads/target"]);
        }
    }
}
