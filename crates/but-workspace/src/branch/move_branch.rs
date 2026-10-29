pub(super) mod function {

    use but_core::RefMetadata;
    use but_rebase::graph_rebase::mutate::SomeSelectors;

    use crate::graph_manipulation::DisconnectParameters;
    use crate::graph_manipulation::get_disconnect_parameters;
    use crate::graph_manipulation::traverse_nodes;

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
    /// Returns the in memory rebase that can then used for materialisation.
    pub fn tear_off_branch<'ws, 'meta, M: RefMetadata>(
        editor: Editor<'ws, 'meta, M>,
        subject_branch_name: &FullNameRef,
    ) -> anyhow::Result<SuccessfulRebase<'ws, 'meta, M>> {
        let successful_rebase = editor.rebase()?;
        let workspace = successful_rebase.overlayed_graph()?.into_workspace()?;
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
            return editor.rebase();
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
            let target = workspace
                .target_commit
                .as_ref()
                .context("Tearing off a branch requires a workspace cutoff")?;
            editor.select_commit(target.commit_id)?
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

        if let Some(workspace_head) = workspace_head {
            let head_selector = editor.select_commit(workspace_head)?;
            let selectors = SomeSelectors::new(Vec::from([head_selector]))?;
            editor.insert_segment_into(
                target_selector,
                subject_delimiter,
                but_rebase::graph_rebase::mutate::InsertSide::Above,
                Some(selectors),
                but_rebase::graph_rebase::mutate::ParentReparentingOrder::Prepend,
            )?;
        } else {
            // There is no workspace commit to connect to the torn-off branch. Attach only
            // its first parent at the target, leaving the remaining stack independent.
            editor.add_edge(subject_delimiter.parent, target_selector, 0)?;
        }

        editor.rebase()
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
    /// Returns the in memory rebase for potential materialisation.
    pub fn move_branch<'ws, 'meta, M: RefMetadata>(
        editor: Editor<'ws, 'meta, M>,
        subject_branch_name: &FullNameRef,
        relative_to: RelativeTo,
        side: InsertSide,
    ) -> anyhow::Result<SuccessfulRebase<'ws, 'meta, M>> {
        let successful_rebase = editor.rebase()?;
        let workspace = successful_rebase.overlayed_graph()?.into_workspace()?;

        let Some(source) = workspace.find_segment_and_lane_by_refname(subject_branch_name) else {
            bail!(
                "Couldn't find branch to move in workspace with reference name: {subject_branch_name}"
            );
        };
        let anchor = Anchor::resolve(&workspace, &relative_to)?;
        if anchor.segment.id == source.1.id {
            bail!("Cannot move branch {subject_branch_name} onto itself");
        }

        // Each kind of workspace has a very different notion of what "moving a branch" means, so we
        // dispatch into a dedicated handler for each one.
        match &workspace.kind {
            WorkspaceKind::AdHoc => move_branch_in_single_branch_mode(
                successful_rebase,
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
                subject_branch_name,
                relative_to,
                side,
            ),
        }
    }

    /// Move a branch in a single-branch (ad-hoc) workspace, where `HEAD` is on a plain local branch.
    ///
    /// `HEAD` follows the top of the reordered stack, so the whole stack stays projected.
    fn move_branch_in_single_branch_mode<'ws, 'meta, M: RefMetadata>(
        mut successful_rebase: SuccessfulRebase<'ws, 'meta, M>,
        source: SegmentInLane<'_>,
        anchor: Anchor<'_>,
        subject_branch_name: &FullNameRef,
        relative_to: RelativeTo,
        side: InsertSide,
    ) -> anyhow::Result<SuccessfulRebase<'ws, 'meta, M>> {
        let ((Lane::Stack(source_stack), subject_segment), Lane::Stack(destination_stack)) =
            (source, anchor.lane)
        else {
            bail!(
                "Moving a branch between a worktree and the workspace in single-branch mode is not yet supported"
            );
        };
        // A branch that owns commits can only be reordered within its current stack in
        // single-branch mode. Moving it across stacks would change commit ownership and needs a
        // real rebase.
        if !subject_segment.commits.is_empty() && !same_stack(source_stack, destination_stack) {
            bail!("Moving a non-empty branch in single-branch mode is not yet supported");
        }
        let (_repo, meta) = successful_rebase.repo_and_meta_mut();
        if !meta.can_persist_branch_stack_order() {
            bail!(
                "Cannot reorder '{subject_branch_name}' in single-branch mode without branch order metadata"
            );
        }

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
        if is_tip_of(Lane::Stack(source_stack), subject_segment)
            && let Some(branch_below) = source_stack
                .segments
                .get(1)
                .and_then(|below| below.ref_name())
        {
            let branch_below = editor.select_reference(branch_below)?;
            editor.set_head_checkout(branch_below)?;
        }
        editor.insert_segment(target_selector, subject_delimiter, side)?;

        editor.rebase()
    }

    /// Move a branch within a managed workspace (one backed by a workspace commit).
    fn move_branch_in_managed_workspace<'ws, 'meta, M: RefMetadata>(
        successful_rebase: SuccessfulRebase<'ws, 'meta, M>,
        workspace: &but_graph::Workspace,
        source: SegmentInLane<'_>,
        subject_branch_name: &FullNameRef,
        relative_to: RelativeTo,
        side: InsertSide,
    ) -> anyhow::Result<SuccessfulRebase<'ws, 'meta, M>> {
        let Some(workspace_head) = workspace.tip_commit().map(|commit| commit.id) else {
            bail!("Couldn't find workspace head.")
        };

        let (source_lane, subject_segment) = source;

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

        editor.rebase()
    }

    /// A segment and the lane holding it.
    type SegmentInLane<'a> = (Lane<'a>, &'a StackSegment);

    fn is_tip_of(lane: Lane<'_>, segment: &StackSegment) -> bool {
        lane.segments()
            .first()
            .is_some_and(|tip| tip.id == segment.id)
    }

    /// The segment a moved branch is placed next to, and the lane holding it.
    struct Anchor<'a> {
        lane: Lane<'a>,
        segment: &'a StackSegment,
    }

    impl<'a> Anchor<'a> {
        fn resolve(
            workspace: &'a but_graph::Workspace,
            relative_to: &RelativeTo,
        ) -> anyhow::Result<Self> {
            let (lane, segment) = match relative_to {
                RelativeTo::Reference(name) => workspace
                    .find_segment_and_lane_by_refname(name.as_ref())
                    .with_context(|| {
                        format!(
                            "Couldn't find target branch to move in workspace with reference name: {name}"
                        )
                    })?,
                RelativeTo::Commit(id) => workspace
                    .find_segment_and_lane_by_commit_id(*id)
                    .with_context(|| format!("Commit {id} isn't part of the workspace"))?,
            };
            Ok(Anchor { lane, segment })
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
}
