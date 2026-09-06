use but_core::RefMetadata;
use but_rebase::graph_rebase::RebasedEditor;

/// Outcome of moving branches between or out of stacks.
///
/// Returned by [function::move_branch()].
#[derive(Debug)]
pub struct Outcome<'meta, M: RefMetadata> {
    /// A successful rebase result for continuing operations.
    pub rebase: RebasedEditor<'meta, M>,
    /// The updated workspace metadata that accompanies the move operation.
    /// It should replace the actual workspace metadata to configure moved 'virtual' branches segments, if `Some()`.
    pub ws_meta: Option<but_core::ref_metadata::Workspace>,
    /// In single-branch (ad-hoc) mode, set to the reference that should become the new tip after the
    /// reorder. This can be the subject when it moves above the current tip, or the branch now above
    /// it when the checked-out tip moves down. `HEAD` is *not* moved by the operation; the caller is
    /// responsible for checking this out so the whole reordered stack stays projected (mirroring
    /// [`create_reference`](crate::branch::create_reference())). `None` when the tip is unchanged.
    pub new_tip: Option<gix::refs::FullName>,
    /// In single-branch (ad-hoc) mode, the reordered tip-to-base branch chain that the caller should
    /// persist with [`RefMetadata::set_branch_stack_order`].
    /// It is returned rather than written here so callers can apply it only for real runs and skip
    /// persistence for dry-run previews. `None` outside single-branch mode.
    pub branch_stack_order: Option<Vec<gix::refs::FullName>>,
}

/// What remains after [`Outcome::apply()`]: the pieces callers still need once the move is
/// on disk and the workspace refreshed.
pub struct Applied<'meta, M: RefMetadata> {
    /// The metadata entry, released by materialization.
    pub meta: &'meta mut M,
    /// Every commit rewritten by the move, old id to new id.
    pub commit_mappings: std::collections::BTreeMap<gix::ObjectId, gix::ObjectId>,
    /// See [`Outcome::new_tip`]: the reference the caller should check out in single-branch
    /// mode, `None` when the tip is unchanged. Applying does NOT move `HEAD`.
    pub new_tip: Option<gix::refs::FullName>,
}

impl<'meta, M: RefMetadata> Outcome<'meta, M> {
    /// Apply the move for real, in one step: materialize the rebase to disk, persist the
    /// branch order and workspace metadata it decided, and refresh `ws` from the result.
    ///
    /// Metadata and refs move together here — persisting one without the other leaves a
    /// projection that disagrees with disk. Dry runs preview via
    /// [`RebasedEditor::overlay_with`] instead and never call
    /// this.
    pub fn apply(
        self,
        ws: &mut but_graph::Workspace,
        repo: &gix::Repository,
        db: &mut but_db::DbHandle,
    ) -> anyhow::Result<Applied<'meta, M>> {
        let Outcome {
            rebase,
            ws_meta,
            new_tip,
            branch_stack_order,
        } = self;
        let commit_mappings = rebase.commit_mappings();
        let (graph, meta) = rebase.materialize()?;
        if let Some(order) = branch_stack_order.as_deref() {
            meta.set_branch_stack_order(order)?;
        }
        ws.refresh_from_commit_graph(graph, repo, meta, db)?;
        if let Some((ws_meta, ref_name)) = ws_meta.zip(ws.ref_name()) {
            let mut md = meta.workspace(ref_name)?;
            *md = ws_meta;
            meta.set_workspace(&md)?;
        }
        Ok(Applied {
            commit_mappings,
            meta,
            new_tip,
        })
    }
}

pub(super) mod function {

    use but_core::RefMetadata;
    use but_core::ref_metadata::StackId;
    use but_graph::workspace::{Segment, SegmentStack};
    use but_rebase::graph_rebase::anchor::{Connect, Cut, Range};
    use but_rebase::graph_rebase::mutate::{InsertSide, Reconnect};

    use super::Outcome;
    use anyhow::Context;
    use anyhow::bail;
    use but_graph::workspace::WorkspaceKind;
    use but_rebase::graph_rebase::Editor;
    use but_rebase::graph_rebase::EditorIndex;
    use but_rebase::graph_rebase::RebasedEditor;
    use gix::refs::FullNameRef;

    /// What a segment move lifts: the segment's editor-graph `range` (its reference down to
    /// its bottom commit — the reference alone when the branch is empty), and which children
    /// sever at its top seam — the one thing the projection knows stands directly above:
    /// the workspace commit, the child segment's bottom commit, or that segment's reference
    /// when it is empty (this names where a shared reference group splits). The base below
    /// is the editor's own rule ([`Editor::move_range`](but_rebase::graph_rebase::Editor::move_range)).
    struct MoveBounds {
        /// The bounds of the segment to move.
        range: Range,
        /// Which children of the range's child-most bound sever.
        children: Cut,
    }

    /// The [`MoveBounds`] of `subject_segment` within `source_stack`.
    fn move_bounds<'meta, M: RefMetadata>(
        editor: &Editor<'meta, M>,
        source_stack: &SegmentStack,
        subject_segment: &Segment,
        workspace_head: Option<gix::ObjectId>,
    ) -> anyhow::Result<MoveBounds> {
        let index_of_segment = source_stack
            .segments
            .iter()
            .position(|segment| {
                segment.ref_name == subject_segment.ref_name
                    && segment.tip() == subject_segment.tip()
            })
            .context("BUG: Unable to find subject segment on source stack.")?;

        let subject_segment_ref_name = subject_segment
            .ref_name()
            .context("Subject segment doesn't have a ref name.")?;
        let child: EditorIndex = editor
            .select_reference(subject_segment_ref_name)
            .context("Failed to find subject reference in graph.")?
            .into();
        let parent: EditorIndex = match subject_segment.commits.last() {
            Some(bottom) => editor
                .select_commit(*bottom)
                .context("Failed to find last commit in subject segment in graph.")?
                .into(),
            // An empty branch is just its reference.
            None => child,
        };
        let range = Range { child, parent };

        if index_of_segment == 0 {
            // Managed workspaces have a workspace commit above the top-most segment. Ad-hoc
            // workspaces do not have such a child, so there is no child seam to sever there.
            let children = workspace_head
                .map(|workspace_head| -> anyhow::Result<Cut> {
                    let workspace_head_entry = editor
                        .select_commit(workspace_head)
                        .context("Failed to find workspace head in graph.")?;
                    Ok(Cut::only([workspace_head_entry]))
                })
                .transpose()?
                .unwrap_or(Cut::Nothing);

            return Ok(MoveBounds { range, children });
        }

        // Segment on top of the subject segment in the stack.
        let child_segment = source_stack.segments.get(index_of_segment - 1).context(
            "BUG: Unable to find child segment of subject segment but expected it to exist.",
        )?;

        // The seam is the child segment's bottom commit — or its reference when that segment
        // is empty, naming where the shared reference group splits.
        let child_entry: EditorIndex = match child_segment.commits.last() {
            Some(bottom) => editor
                .select_commit(*bottom)
                .context("Failed to find last commit of child segment in graph.")
                .map(EditorIndex::from),
            None => {
                let child_segment_ref_name = child_segment
                    .ref_name()
                    .context("Child segment doesn't have a ref name.")?;
                editor
                    .select_reference(child_segment_ref_name)
                    .context("Failed to find child segment reference in graph.")
                    .map(EditorIndex::from)
            }
        }?;
        let children = Cut::only([child_entry]);

        Ok(MoveBounds { range, children })
    }

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
    /// `stack_id_override` - Optionally, the ID to use for the newly created stack.
    ///     Mainly used for testing purposes.
    ///
    /// Returns the in memory update [outcome](Outcome) that can then used for materialisation.
    pub fn tear_off_branch<'meta, M: RefMetadata>(
        editor: Editor<'meta, M>,
        current_workspace: &but_graph::Workspace,
        subject_branch_name: &FullNameRef,
        stack_id_override: Option<StackId>,
    ) -> anyhow::Result<Outcome<'meta, M>> {
        let successful_rebase = editor.rebase()?;
        let workspace =
            crate::workspace::overlayed_workspace(current_workspace, &successful_rebase)?;
        let mut editor = successful_rebase.into_editor();
        let Some(source) = workspace.find_branch(subject_branch_name) else {
            bail!(
                "Couldn't find branch to move in workspace with reference name: {subject_branch_name}"
            );
        };

        // We're currently stopping the move branch operations imperatively at this stage, in order to
        // reduce the scope of this first iteration of moving the branches.
        // TODO: Enable and test that we can move branches in any kind of workspace.
        match workspace.kind() {
            WorkspaceKind::Managed { .. } => {}
            WorkspaceKind::ManagedMissingWorkspaceCommit { .. } => {
                bail!("Moving branches currently need a workspace commit")
            }
            WorkspaceKind::AdHoc => {
                bail!("Moving branches in non-managed workspaces is not supported");
            }
        };

        let mut ws_meta = workspace.metadata.clone();

        let (source_stack, subject_segment) = source;

        if source_stack.segments.len() == 1 {
            // There's only one branch in the source stack. Nothing to do.
            return Ok(Outcome {
                rebase: editor.rebase()?,
                ws_meta,
                new_tip: None,
                branch_stack_order: None,
            });
        }

        let Some(workspace_head) = workspace.tip_commit_id() else {
            bail!("Couldn't find workspace head.")
        };
        let head_entry = editor
            .select_commit(workspace_head)
            .context("Failed to find the workspace head in the graph.")?;

        let Some(lower_bound_ref) = workspace.lower_bound_ref_name() else {
            bail!("Tearing off a branch requires a workspace common base");
        };

        let target_entry = editor
            .select_reference(lower_bound_ref)
            .context("Failed to find target reference in graph.")?;

        let MoveBounds {
            range: subject_delimiter,
            children,
        } = move_bounds(&editor, source_stack, subject_segment, Some(workspace_head))?;
        let subject_top = subject_delimiter.child;
        editor.move_range(
            subject_delimiter,
            children,
            target_entry,
            but_rebase::graph_rebase::mutate::InsertSide::Above,
            Connect::only([head_entry]),
            Reconnect::Heal,
        )?;
        // The torn-off branch becomes the first lane: the merge's parent order is the lane
        // order, and a branch just pulled out of a stack lands at the front.
        editor.detach(head_entry, subject_top)?;
        editor.insert_parent(head_entry, subject_top, 0)?;

        // Update the workspace meta in order to create a new stack containing the
        // torn-off branch.
        if let Some(ws_meta) = ws_meta.as_mut() {
            ws_meta.remove_segment(subject_branch_name);
            ws_meta.add_or_insert_new_stack_if_not_present(
                subject_branch_name,
                None,
                but_core::ref_metadata::WorkspaceCommitRelation::Merged,
                |_| stack_id_override.unwrap_or_else(StackId::generate),
            );
        };

        Ok(Outcome {
            rebase: editor.rebase()?,
            ws_meta,
            new_tip: None,
            branch_stack_order: None,
        })
    }

    /// Move a branch between stacks in the `workspace`.
    ///
    /// `editor` is assumed to have been generated from the given `workspace`
    /// and therefore aligned.
    ///
    /// `workspace` - Used for getting the surrounding context of the branch being moved.
    ///     In the future, we should not rely on the projection and do it fully on the graph.
    ///
    /// `subject_branch_name` is the full reference name of the branch to move.
    ///
    /// `target_branch_name` is the full reference name of the branch to move the subject
    /// branch on top of.
    ///
    /// Returns an [outcome](Outcome) for potential materialisation.
    pub fn move_branch<'meta, M: RefMetadata>(
        editor: Editor<'meta, M>,
        current_workspace: &but_graph::Workspace,
        subject_branch_name: &FullNameRef,
        target_branch_name: &FullNameRef,
    ) -> anyhow::Result<Outcome<'meta, M>> {
        if subject_branch_name == target_branch_name {
            bail!("Cannot move branch {subject_branch_name} onto itself");
        }

        let successful_rebase = editor.rebase()?;
        let workspace =
            crate::workspace::overlayed_workspace(current_workspace, &successful_rebase)?;

        let (source, destination) =
            retrieve_branches_and_containers(&workspace, subject_branch_name, target_branch_name)?;

        // Each kind of workspace has a very different notion of what "moving a branch" means, so we
        // dispatch into a dedicated handler for each one.
        match workspace.kind() {
            WorkspaceKind::AdHoc => move_branch_in_single_branch_mode(
                successful_rebase,
                workspace.ref_name_owned(),
                source,
                destination,
                subject_branch_name,
                target_branch_name,
            ),
            WorkspaceKind::ManagedMissingWorkspaceCommit { .. } => {
                bail!("Moving branches currently need a workspace commit")
            }
            WorkspaceKind::Managed { .. } => move_branch_in_managed_workspace(
                successful_rebase,
                workspace,
                source,
                destination,
                subject_branch_name,
                target_branch_name,
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
    fn move_branch_in_single_branch_mode<'meta, M: RefMetadata>(
        mut successful_rebase: RebasedEditor<'meta, M>,
        entrypoint: Option<gix::refs::FullName>,
        source: WorkspaceSegmentContext,
        destination: WorkspaceSegmentContext,
        subject_branch_name: &FullNameRef,
        target_branch_name: &FullNameRef,
    ) -> anyhow::Result<Outcome<'meta, M>> {
        let (source_stack, subject_segment) = &source;
        let (destination_stack, _) = &destination;
        // A branch that owns commits can only be reordered within its current stack in
        // single-branch mode. Moving it across stacks would change commit ownership and needs a
        // real rebase.
        if subject_segment.tip().is_some() && !same_stack(source_stack, destination_stack) {
            bail!("Moving a non-empty branch in single-branch mode is not yet supported");
        }
        // Reordering same-target empty refs only changes which empty segment is displayed first —
        // metadata alone covers it. Everything else needs the editor: the crossed refs (and
        // possibly commits) must move to match the new order, or the projection would disagree
        // with what was persisted.
        let subject_shares_target_commit = successful_rebase
            .reference_target(subject_branch_name)?
            == successful_rebase.reference_target(target_branch_name)?;
        let move_requires_graph_update =
            subject_segment.tip().is_some() || !subject_shares_target_commit;
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
                None => match meta.branch_stack_order(target_branch_name)? {
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
        let new_order =
            reorder_branch_in_stack_order(existing_order, target_branch_name, subject_branch_name);

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
            // The subject's commits relocate only when the move crosses another commit owner.
            // Crossing empties alone moves no commit: the subject's range already rests where
            // it lands, and only the empty refs need re-pointing (the reconcile pass below).
            let crossed = crossed_segments(source_stack, subject_branch_name, target_branch_name)?;
            let needs_commit_surgery = subject_segment.tip().is_some()
                && crossed.iter().any(|segment| segment.tip().is_some());

            let mut editor = successful_rebase.into_editor();

            if needs_commit_surgery {
                let target_entry = editor
                    .select_reference(target_branch_name)
                    .context("Failed to find target reference in graph.")?;
                let MoveBounds {
                    range: subject_delimiter,
                    children,
                } = move_bounds(&editor, source_stack, subject_segment, None)?;
                editor.move_range(
                    subject_delimiter,
                    children,
                    target_entry,
                    InsertSide::Above,
                    Connect::Splice,
                    Reconnect::Heal,
                )?;
            }

            // Reconcile pass: re-anchor every empty ref of this stack directly above its
            // below-neighbor per the new order, bottom to top. This is the one mechanism all
            // empty-crossing shapes reduce to — empties dropping below a lifted subject,
            // lifting above a lowered one, an empty subject relocating itself, and empties
            // that rode a surgically moved commit — so refs always land where the persisted
            // order says.
            let mut anchor_ref: Option<gix::refs::FullName> = None;
            for name in new_order.iter().rev() {
                let is_empty_segment = segment_index_in(source_stack, name.as_ref())
                    .and_then(|index| source_stack.segments.get(index))
                    .map(|segment| segment.tip().is_none());
                if is_empty_segment != Some(true) {
                    // A commit owner, or a segment this stack doesn't know: a fixed anchor.
                    anchor_ref = Some(name.clone());
                    continue;
                }
                let anchor = anchor_ref.as_ref().with_context(|| {
                    format!(
                        "Unsupported reorder: empty branch '{}' would sit below the bottom branch",
                        name.as_bstr()
                    )
                })?;
                let anchor = editor.select_reference(anchor.as_ref())?;
                editor.move_reference(
                    editor.select_reference(name.as_ref())?,
                    anchor,
                    InsertSide::Above,
                )?;
                anchor_ref = Some(name.clone());
            }

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
    fn move_branch_in_managed_workspace<'meta, M: RefMetadata>(
        successful_rebase: RebasedEditor<'meta, M>,
        workspace: but_graph::Workspace,
        source: WorkspaceSegmentContext,
        destination: WorkspaceSegmentContext,
        subject_branch_name: &FullNameRef,
        target_branch_name: &FullNameRef,
    ) -> anyhow::Result<Outcome<'meta, M>> {
        let Some(workspace_head) = workspace.tip_commit_id() else {
            bail!("Couldn't find workspace head.")
        };

        let mut ws_meta = workspace.metadata.clone();

        let (source_stack, subject_segment) = source;
        let (destination_stack, target_segment) = destination;
        // Same-commit empty refs within ONE stack reorder purely in metadata. Empties on
        // different commits are a real move (the subject's ref must re-point), and a
        // same-commit move ACROSS stacks must still merge the subject into the target's
        // chain in the editor — a metadata-only edit leaves the subject's old chain in
        // the layout, and the next materialize builds a stale workspace parent from it.
        let subject_shares_target_commit = successful_rebase
            .reference_target(subject_branch_name)?
            == successful_rebase.reference_target(target_branch_name)?;
        if subject_segment.tip().is_none()
            && target_segment.tip().is_none()
            && subject_shares_target_commit
            && same_stack(&source_stack, &destination_stack)
            && ws_meta.is_some()
        {
            if let Some(ws_meta) = ws_meta.as_mut() {
                move_branch_in_metadata(ws_meta, subject_branch_name, target_branch_name);
            }
            return Ok(Outcome {
                rebase: successful_rebase,
                ws_meta,
                new_tip: None,
                branch_stack_order: None,
            });
        }

        // A commit-owning subject crossing only empties within its stack relocates nothing:
        // its range already rests where it lands. Running the pick surgery anyway is the
        // degenerate self-move — it disconnects the range from the workspace commit and never
        // reconnects it, expelling the subject's commits from the workspace. Re-anchor the
        // crossed empty refs around the unchanged range instead.
        if subject_segment.tip().is_some() && same_stack(&source_stack, &destination_stack) {
            let crossed = crossed_segments(&source_stack, subject_branch_name, target_branch_name)?;
            let target_index = segment_index_in(&source_stack, target_branch_name)
                .context("BUG: target segment missing from the source stack")?;
            let subject_index = segment_index_in(&source_stack, subject_branch_name)
                .context("BUG: subject segment missing from its source stack")?;
            if crossed.iter().all(|segment| segment.tip().is_none()) {
                let mut editor = successful_rebase.into_editor();
                if target_index < subject_index {
                    // The subject lifts above the crossed empties: they drop below its
                    // bottom-most commit, keeping their order.
                    let mut anchor: EditorIndex = editor
                        .select_commit(
                            subject_segment
                                .commits
                                .last()
                                .copied()
                                .context("BUG: non-empty subject has no commits")?,
                        )?
                        .into();
                    for segment in crossed {
                        let empty_ref = segment
                            .ref_name()
                            .context("Empty segment doesn't have a ref")?;
                        editor.move_reference(
                            editor.select_reference(empty_ref)?,
                            anchor,
                            InsertSide::Below,
                        )?;
                        anchor = editor.select_reference(empty_ref)?.into();
                    }
                } else {
                    // The subject drops below the crossed empties: they lift above its tip
                    // ref, keeping their order.
                    let mut anchor_ref = subject_branch_name.to_owned();
                    for segment in crossed.iter().rev() {
                        let empty_ref = segment
                            .ref_name()
                            .context("Empty segment doesn't have a ref")?;
                        let anchor = editor.select_reference(anchor_ref.as_ref())?;
                        editor.move_reference(
                            editor.select_reference(empty_ref)?,
                            anchor,
                            InsertSide::Above,
                        )?;
                        anchor_ref = empty_ref.to_owned();
                    }
                }
                if let Some(ws_meta) = ws_meta.as_mut() {
                    move_branch_in_metadata(ws_meta, subject_branch_name, target_branch_name);
                }
                return Ok(Outcome {
                    rebase: editor.rebase()?,
                    ws_meta,
                    new_tip: None,
                    branch_stack_order: None,
                });
            }
        }

        let mut editor = successful_rebase.into_editor();
        let target_segment_ref_name = target_segment
            .ref_name()
            .context("Target segment doesn't have a ref")?;
        let target_entry = editor
            .select_reference(target_segment_ref_name)
            .context("Failed to find target reference in graph.")?;

        let MoveBounds {
            range: subject_delimiter,
            children,
        } = move_bounds(
            &editor,
            &source_stack,
            &subject_segment,
            Some(workspace_head),
        )?;
        let reconnect = if source_stack.segments.len() == 1 {
            Reconnect::Skip
        } else {
            Reconnect::Heal
        };
        editor.move_range(
            subject_delimiter,
            children,
            target_entry,
            but_rebase::graph_rebase::mutate::InsertSide::Above,
            Connect::Splice,
            reconnect,
        )?;

        // Keep workspace metadata aligned with the graph move outcome for all move cases.
        // We remove the subject branch from its current location and reinsert it above the target.
        if let Some(ws_meta) = ws_meta.as_mut() {
            move_branch_in_metadata(ws_meta, subject_branch_name, target_branch_name);
        };

        Ok(Outcome {
            rebase: editor.rebase()?,
            ws_meta,
            new_tip: None,
            branch_stack_order: None,
        })
    }

    /// The position of the segment named `name` within `stack`, if any.
    fn segment_index_in(
        stack: &but_graph::workspace::SegmentStack,
        name: &FullNameRef,
    ) -> Option<usize> {
        stack
            .segments
            .iter()
            .position(|segment| segment.ref_name() == Some(name))
    }

    /// The segments a reorder of `subject` onto `target` crosses within `stack`, in stack
    /// order: always exclusive of `subject`; `target` is included when the subject lifts
    /// above it, and excluded when the subject drops to sit on top of it. Shared by both
    /// move modes.
    fn crossed_segments<'a>(
        stack: &'a but_graph::workspace::SegmentStack,
        subject_branch_name: &FullNameRef,
        target_branch_name: &FullNameRef,
    ) -> anyhow::Result<&'a [but_graph::workspace::Segment]> {
        let subject_index = segment_index_in(stack, subject_branch_name)
            .context("BUG: subject segment missing from its source stack")?;
        let target_index = segment_index_in(stack, target_branch_name)
            .context("BUG: target segment missing from the source stack")?;
        Ok(if target_index < subject_index {
            stack
                .segments
                .get(target_index..subject_index)
                .unwrap_or(&[])
        } else {
            stack
                .segments
                .get(subject_index + 1..target_index)
                .unwrap_or(&[])
        })
    }

    /// A segment and its container stack.
    type WorkspaceSegmentContext = (
        but_graph::workspace::SegmentStack,
        but_graph::workspace::Segment,
    );

    type WorkspaceSegmentContextRef<'a> = (
        &'a but_graph::workspace::SegmentStack,
        &'a but_graph::workspace::Segment,
    );

    fn own_context<'a>(ctx: WorkspaceSegmentContextRef<'a>) -> WorkspaceSegmentContext {
        (ctx.0.to_owned(), ctx.1.to_owned())
    }

    fn same_stack(
        left: &but_graph::workspace::SegmentStack,
        right: &but_graph::workspace::SegmentStack,
    ) -> bool {
        left.segments.len() == right.segments.len()
            && left
                .segments
                .iter()
                .zip(&right.segments)
                .all(|(left, right)| left.ref_name() == right.ref_name())
    }

    fn stack_branch_order(stack: &but_graph::workspace::SegmentStack) -> Vec<gix::refs::FullName> {
        stack.segment_names().map(ToOwned::to_owned).collect()
    }

    fn reordered_entrypoint(
        entrypoint: Option<&FullNameRef>,
        stack: &but_graph::workspace::SegmentStack,
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

    /// Determine the surrounding context of the subject and target branches.
    ///
    /// Currently, this looks into the workspace projection in order to determine **where to take the branch from and to**.
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
    /// the stack, and insert it on top of another. In graph terms, this means that we:
    /// - Disconnect the reference node from the base segment (the branch under the subject or the target base)
    /// - Disconnect the last commit node of the child segment (the branch over the subject or the workspace commit)
    /// - Nothing else. Other parentage and children are kept, since this is what we care about in a GB workspace world.
    ///
    /// ### What the future holds
    /// In the future, where we're not afraid of complex graphs, we've figured out UX and data wrangling,
    /// the concept of a segment might not hold, and hence we'll have to figure out a better way of determining
    /// what to cut (e.g. letting the clients decide what to cut).
    fn retrieve_branches_and_containers(
        workspace: &but_graph::Workspace,
        subject_branch_name: &FullNameRef,
        target_branch_name: &FullNameRef,
    ) -> anyhow::Result<(WorkspaceSegmentContext, WorkspaceSegmentContext)> {
        let Some(source) = workspace.find_branch(subject_branch_name) else {
            bail!(
                "Couldn't find branch to move in workspace with reference name: {subject_branch_name}"
            );
        };

        let Some(destination) = workspace.find_branch(target_branch_name) else {
            bail!(
                "Couldn't find target branch to move in workspace with reference name: {target_branch_name}"
            );
        };
        Ok((own_context(source), own_context(destination)))
    }

    /// Reorder `subject` to sit directly on top of `target` in the tip-to-base ad-hoc `order`.
    ///
    /// Mirrors the [`Position::Above`](crate::branch::create_reference::Position) case of
    /// `create_reference`'s `insert_into_branch_stack_order`: `subject` is removed and re-inserted
    /// at `target`'s position, pushing `target` (and everything below it) down.
    ///
    /// If `target` isn't tracked yet (stale or empty metadata) it is appended first, so that a move
    /// where *both* branches are missing adds them both - `subject` on top of `target` - instead of
    /// silently clobbering the rest of the ordering down to just `subject`.
    fn reorder_branch_in_stack_order(
        mut order: Vec<gix::refs::FullName>,
        target_branch_name: &FullNameRef,
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
        order.insert(target_idx, subject_branch_name.to_owned());
        order
    }

    fn move_branch_in_metadata(
        ws_meta: &mut but_core::ref_metadata::Workspace,
        subject_branch_name: &FullNameRef,
        target_branch_name: &FullNameRef,
    ) {
        ws_meta.remove_segment(subject_branch_name);
        if ws_meta
            .insert_new_segment_above_anchor_if_not_present(subject_branch_name, target_branch_name)
            .is_none()
        {
            // If metadata doesn't know the target anchor (stale metadata),
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
        use super::reorder_branch_in_stack_order;

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
                r("refs/heads/b").as_ref(),
            );
            // `b` moves directly above `main`, `a` shifts down.
            assert_eq!(
                names(&new),
                ["refs/heads/b", "refs/heads/main", "refs/heads/a"]
            );
        }

        #[test]
        fn adds_subject_above_target_when_only_target_is_present() {
            let order = vec![r("refs/heads/main")];
            let new = reorder_branch_in_stack_order(
                order,
                r("refs/heads/main").as_ref(),
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
                r("refs/heads/subject").as_ref(),
            );
            assert_eq!(names(&new), ["refs/heads/subject", "refs/heads/target"]);
        }
    }
}
