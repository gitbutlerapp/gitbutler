use anyhow::{Context, bail};
use but_core::RefMetadata;
use but_graph::projection::{Stack, StackSegment};
use but_rebase::graph_rebase::{
    Editor, Selector, SuccessfulRebase,
    mutate::{SegmentDelimiter, SelectorSet, SomeSelectors},
};

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
}

pub(super) mod function {

    use but_core::RefMetadata;
    use but_core::ref_metadata::StackId;
    use but_rebase::graph_rebase::mutate::SomeSelectors;

    use super::get_disconnect_parameters;

    use super::Outcome;
    use anyhow::Context;
    use anyhow::bail;
    use but_graph::projection::WorkspaceKind;
    use but_rebase::graph_rebase::Editor;
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
    /// `stack_id_override` - Optionally, the ID to use for the newly created stack.
    ///     Mainly used for testing purposes.
    ///
    /// Returns the in memory update [outcome](Outcome) that can then used for materialisation.
    pub fn tear_off_branch<'ws, 'meta, M: RefMetadata>(
        mut editor: Editor<'ws, 'meta, M>,
        subject_branch_name: &FullNameRef,
        stack_id_override: Option<StackId>,
    ) -> anyhow::Result<Outcome<'ws, 'meta, M>> {
        let Some(source) = editor
            .workspace
            .find_segment_and_stack_by_refname(subject_branch_name)
        else {
            bail!(
                "Couldn't find branch to move in workspace with reference name: {subject_branch_name}"
            );
        };

        // We're currently stopping the move branch operations imperatively at this stage, in order to
        // reduce the scope of this first iteration of moving the branches.
        // TODO: Enable and test that we can move branches in any kind of workspace.
        match &editor.workspace.kind {
            WorkspaceKind::Managed { .. } => {}
            WorkspaceKind::ManagedMissingWorkspaceCommit { .. } => {
                bail!("Moving branches currently need a workspace commit")
            }
            WorkspaceKind::AdHoc => {
                bail!("Moving branches in non-managed workspaces is not supported");
            }
        };

        let mut ws_meta = editor.workspace.metadata.clone();

        let (source_stack, subject_segment) = source;

        if source_stack.segments.len() == 1 {
            // There's only one branch in the source stack. Nothing to do.
            return Ok(Outcome {
                rebase: editor.rebase()?,
                ws_meta,
            });
        }

        let Some(workspace_head) = editor.workspace.tip_commit().map(|commit| commit.id) else {
            bail!("Couldn't find workspace head.")
        };

        let head_selector = editor
            .select_commit(workspace_head)
            .context("Failed to find the workspace head in the graph.")?;

        let Some(lower_bound_ref) = editor
            .workspace
            .lower_bound_segment_id
            .map(|segment_id| &editor.workspace.graph[segment_id])
            .and_then(|segment| segment.ref_name())
        else {
            bail!("Tearing off a branch requires a workspace common base");
        };

        let target_selector = editor
            .select_reference(lower_bound_ref)
            .context("Failed to find target reference in graph.")?;

        let (subject_delimiter, children_to_disconnect, parents_to_disconnect) =
            get_disconnect_parameters(&editor, source_stack, subject_segment, workspace_head)?;

        editor.disconnect_segment_from(
            subject_delimiter.clone(),
            children_to_disconnect,
            parents_to_disconnect,
            false,
        )?;

        let selectors = SomeSelectors::new(vec![head_selector])?;

        editor.insert_segment_into(
            target_selector,
            subject_delimiter,
            but_rebase::graph_rebase::mutate::InsertSide::Above,
            Some(selectors),
        )?;

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
    pub fn move_branch<'ws, 'meta, M: RefMetadata>(
        mut editor: Editor<'ws, 'meta, M>,
        subject_branch_name: &FullNameRef,
        target_branch_name: &FullNameRef,
    ) -> anyhow::Result<Outcome<'ws, 'meta, M>> {
        let (source, destination) = retrieve_branches_and_containers(
            editor.workspace,
            subject_branch_name,
            target_branch_name,
        )?;

        let Some(workspace_head) = editor.workspace.tip_commit().map(|commit| commit.id) else {
            bail!("Couldn't find workspace head.")
        };

        // We're currently stopping the move branch operations imperatively at this stage, in order to
        // reduce the scope of this first iteration of moving the branches.
        // TODO: Enable and test that we can move branches in any kind of workspace.
        match &editor.workspace.kind {
            WorkspaceKind::Managed { .. } => {}
            WorkspaceKind::ManagedMissingWorkspaceCommit { .. } => {
                bail!("Moving branches currently need a workspace commit")
            }
            WorkspaceKind::AdHoc => {
                bail!("Moving branches in non-managed workspaces is not supported");
            }
        };

        let mut ws_meta = editor.workspace.metadata.clone();

        let (source_stack, subject_segment) = source;
        let (_, target_segment) = destination;
        let target_segment_ref_name = target_segment
            .ref_name()
            .context("Target segment doesn't have a ref")?;
        let target_selector = editor
            .select_reference(target_segment_ref_name)
            .context("Failed to find target reference in graph.")?;

        let (subject_delimiter, children_to_disconnect, parents_to_disconnect) =
            get_disconnect_parameters(&editor, &source_stack, &subject_segment, workspace_head)?;

        let skip_reconnect_step = source_stack.segments.len() == 1;
        editor.disconnect_segment_from(
            subject_delimiter.clone(),
            children_to_disconnect,
            parents_to_disconnect,
            skip_reconnect_step,
        )?;
        editor.insert_segment(
            target_selector,
            subject_delimiter,
            but_rebase::graph_rebase::mutate::InsertSide::Above,
        )?;

        // Update the workspace meta if any of the branches we're handling is empty.
        // This is needed in order to disambiguate the intended operation.
        if let Some(ws_meta) = ws_meta.as_mut()
            && (subject_segment.commits.is_empty() || target_segment.commits.is_empty())
        {
            ws_meta.remove_segment(subject_branch_name);
            ws_meta.insert_new_segment_above_anchor_if_not_present(
                subject_branch_name,
                target_branch_name,
            );
        };

        Ok(Outcome {
            rebase: editor.rebase()?,
            ws_meta,
        })
    }

    /// A segment and its container stack.
    type WorkspaceSegmentContext = (
        but_graph::projection::Stack,
        but_graph::projection::StackSegment,
    );

    type WorkspaceSegmentContextRef<'a> = (
        &'a but_graph::projection::Stack,
        &'a but_graph::projection::StackSegment,
    );

    fn own_context<'a>(ctx: WorkspaceSegmentContextRef<'a>) -> WorkspaceSegmentContext {
        (ctx.0.to_owned(), ctx.1.to_owned())
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
        workspace: &but_graph::projection::Workspace,
        subject_branch_name: &FullNameRef,
        target_branch_name: &FullNameRef,
    ) -> anyhow::Result<(WorkspaceSegmentContext, WorkspaceSegmentContext)> {
        let Some(source) = workspace.find_segment_and_stack_by_refname(subject_branch_name) else {
            bail!(
                "Couldn't find branch to move in workspace with reference name: {subject_branch_name}"
            );
        };

        let Some(destination) = workspace.find_segment_and_stack_by_refname(target_branch_name)
        else {
            bail!(
                "Couldn't find target branch to move in workspace with reference name: {target_branch_name}"
            );
        };
        Ok((own_context(source), own_context(destination)))
    }
}

/// Get the right disconnect parameters for the given subject segment and source stack.
///
/// This function determines which are the right parents and children to disconnect,
/// as well as the right segment delimiter to move.
fn get_disconnect_parameters<'ws, 'meta, M: RefMetadata>(
    editor: &Editor<'ws, 'meta, M>,
    source_stack: &Stack,
    subject_segment: &StackSegment,
    workspace_head: gix::ObjectId,
) -> anyhow::Result<(
    SegmentDelimiter<Selector, Selector>,
    SelectorSet,
    SelectorSet,
)> {
    let index_of_segment = source_stack
        .segments
        .iter()
        .position(|segment| segment.id == subject_segment.id)
        .context("BUG: Unable to find subject segment on source stack.")?;

    let subject_segment_ref_name = subject_segment
        .ref_name()
        .context("Subject segment doesn't have a ref name.")?;
    let delimiter_child = editor
        .select_reference(subject_segment_ref_name)
        .context("Failed to find subject reference in graph.")?;
    let delimiter_parent = match subject_segment.commits.last() {
        Some(last_commit) => editor
            .select_commit(last_commit.id)
            .context("Failed to find last commit in subject segment in graph.")?,
        None => {
            // Subject segment is empty, move only the reference
            delimiter_child
        }
    };

    // The delimiter for the segment we want to move, is the reference selector
    // as the child, and the last commit inside the branch as the parent.
    // If the branch is empty, we take the reference selector as the parent as well.
    let delimiter = SegmentDelimiter {
        child: delimiter_child,
        parent: delimiter_parent,
    };

    // The parent segment in the stack if any.
    // This will be `None` if the branch we want to move is at the bottom of the stack.
    let stack_base_segment = subject_segment.base_segment_id.and_then(|base_segment_id| {
        source_stack
            .segments
            .iter()
            .find(|segment| segment.id == base_segment_id)
    });

    // The parent segment in the graph.
    // If the `stack_base_segment` is `None` but there's a `base_segment_id` defined, it means we'll find it in the
    // graph data, and it's probably the target branch, which is not included in the workspace.
    let graph_base_segment = subject_segment
        .base_segment_id
        .map(|segment_idx| &editor.workspace.graph[segment_idx]);

    let parents_to_disconnect = if let Some(stack_base_segment) = stack_base_segment {
        // Base segment is part of the source stack.
        let base_segment_ref_name = stack_base_segment
            .ref_name()
            .context("Base segment doesn't have a ref name.")?;
        let reference_selector = editor.select_reference(base_segment_ref_name)?;
        let selectors = SomeSelectors::new(vec![reference_selector])?;
        SelectorSet::Some(selectors)
    } else if let Some(graph_base_segment) = graph_base_segment {
        // Base segment is outside of workspace (probably target branch).
        let ref_name = graph_base_segment
            .ref_name()
            .context("Graph base segment doesn't have a ref name.")?;
        let reference_selector = editor.select_reference(ref_name)?;
        let selectors = SomeSelectors::new(vec![reference_selector])?;
        SelectorSet::Some(selectors)
    } else if subject_segment.base_segment_id.is_some() {
        // Base segment could not be found, but there is an ID defined. Error out.
        bail!(
            "Failed to find the base segment of the subject we want to move, even if it seems to be defined"
        );
    } else {
        // Nothing found. Remove all parents.
        SelectorSet::All
    };

    if index_of_segment == 0 {
        // This is the top-most segment in the stack, so the parent is the workspace commit.
        let workspace_head_selector = editor
            .select_commit(workspace_head)
            .context("Failed to find workspace head in graph.")?;
        let selectors = SomeSelectors::new(vec![workspace_head_selector])?;
        let children_to_disconnect = SelectorSet::Some(selectors);

        return Ok((delimiter, children_to_disconnect, parents_to_disconnect));
    }

    // Segment on top of the subject segment in the stack.
    let child_segment = source_stack.segments.get(index_of_segment - 1).context(
        "BUG: Unable to find child segment of subject segment but expected it to exist.",
    )?;

    // If branch stacked on top of the branch we want to move is empty, we only need to disconnect
    // the reference from it.
    // Otherwise, disconnect the last commit on the segment.
    let child_selector = match child_segment.commits.last() {
        Some(last_commit) => editor
            .select_commit(last_commit.id)
            .context("Failed to find last commit of child segment in graph."),
        None => {
            // The segment on top of the subject segment is empty. Select the reference.
            let child_segment_ref_name = child_segment
                .ref_name()
                .context("Child segment doesn't have a ref name.")?;
            editor
                .select_reference(child_segment_ref_name)
                .context("Failed to find child segment reference in graph.")
        }
    }?;
    let selectors = SomeSelectors::new(vec![child_selector])?;
    let children_to_disconnect = SelectorSet::Some(selectors);

    Ok((delimiter, children_to_disconnect, parents_to_disconnect))
}
