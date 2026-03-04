use anyhow::Context;
use but_graph::projection::{Stack, StackSegment};
use but_rebase::graph_rebase::{
    Editor, Selector, SuccessfulRebase,
    mutate::{SegmentDelimiter, SelectorSet, SomeSelectors},
};

/// Outcome of moving branches between stacks.
///
/// Returned by [function::move_branch()].
pub struct Outcome {
    /// A successful rebase result for continuing operations.
    pub rebase: SuccessfulRebase,
}

pub(super) mod function {
    use super::get_disconnect_parameters;

    use super::Outcome;
    use anyhow::Context;
    use anyhow::bail;
    use but_rebase::graph_rebase::Editor;
    use gix::refs::FullNameRef;

    /// Move a branch between stacks in the workspace.
    ///
    /// `subject_branch_name` is the full reference name of the branch to move.
    ///
    /// `target_branch_name` is the full reference name of the branch to move the subject
    /// branch on top of.
    ///
    /// Returns:
    /// - Successful rebase
    pub fn move_branch(
        workspace: &but_graph::projection::Workspace,
        mut editor: Editor,
        subject_branch_name: &FullNameRef,
        target_branch_name: &FullNameRef,
    ) -> anyhow::Result<Outcome> {
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

        let Some(workspace_head) = workspace.tip_commit().map(|commit| commit.id) else {
            bail!("Couldn't find workspace head.")
        };

        let (source_stack, subject_segment) = source;
        let (_, target_segment) = destination;
        let target_segment_ref_name = target_segment
            .ref_name()
            .context("Target segment doesn't have a ref")?;
        let target_selector = editor
            .select_reference(target_segment_ref_name)
            .context("Failed to find target reference in graph.")?;

        let (subject_delimiter, children_to_disconnect, parents_to_disconnect) =
            get_disconnect_parameters(
                workspace,
                &editor,
                source_stack,
                subject_segment,
                workspace_head,
            )?;

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

        Ok(Outcome {
            rebase: editor.rebase()?,
        })
    }
}

/// Get the right disconnect parameters for the given subject segment and source stack.
///
/// This function determines which are the right parents and children to disconnect,
/// as well as the right segment delimiter to move.
fn get_disconnect_parameters(
    workspace: &but_graph::projection::Workspace,
    editor: &Editor,
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
        .enumerate()
        .find_map(|(index, segment)| {
            if segment.id != subject_segment.id {
                None
            } else {
                Some(index)
            }
        })
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

    let delimiter = SegmentDelimiter {
        child: delimiter_child,
        parent: delimiter_parent,
    };

    let stack_base_segment = subject_segment.base_segment_id.and_then(|base_segment_id| {
        source_stack
            .segments
            .iter()
            .find(|segment| segment.id == base_segment_id)
    });

    let graph_base_segment = subject_segment
        .base_segment_id
        .and_then(|segment_idx| workspace.graph.find_segment(segment_idx));

    let parents_to_disconnect = match (stack_base_segment, graph_base_segment) {
        (Some(stack_base_segment), _) => {
            // Base segment is part of the source stack.
            let base_segment_ref_name = stack_base_segment
                .ref_name()
                .context("Base segment doesn't have a ref name.")?;
            let reference_selector = editor.select_reference(base_segment_ref_name)?;
            let selectors = SomeSelectors::new(vec![reference_selector])?;
            SelectorSet::Some(selectors)
        }
        (None, Some(graph_base_segment)) => {
            // Base segment is outside of workspace (probably target branch).
            let ref_name = graph_base_segment
                .ref_name()
                .context("Graph base segment doesn't have a ref name.")?;
            let reference_selector = editor.select_reference(ref_name)?;
            let selectors = SomeSelectors::new(vec![reference_selector])?;
            SelectorSet::Some(selectors)
        }
        (None, None) => {
            // No parents could be determined. Detach all.
            SelectorSet::All
        }
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
