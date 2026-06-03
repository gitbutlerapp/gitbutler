use anyhow::{Context, bail};
use but_core::RefMetadata;
use but_graph::workspace::{Stack, StackSegment};
use but_rebase::graph_rebase::{
    Editor, LookupStep, Selector, Step,
    mutate::{SegmentDelimiter, SelectorSet, SomeSelectors},
};

/// Payload containing information about how to disconnect a segment in the graph.
pub struct DisconnectParameters {
    /// The bounds of the segment to disconnect.
    pub(crate) delimiter: SegmentDelimiter<Selector, Selector>,
    /// The children of the child-most segment bound to disconnect.
    pub(crate) children_to_disconnect: SelectorSet,
    /// The parents of the parent-most segment bound to disconnect.
    pub(crate) parents_to_disconnect: SelectorSet,
}

/// Get the right disconnect parameters for the given subject segment and source stack.
///
/// This function determines which are the right parents and children to disconnect,
/// as well as the right segment delimiter to move.
pub fn get_disconnect_parameters<'ws, 'meta, M: RefMetadata>(
    editor: &Editor<'ws, 'meta, M>,
    workspace: &but_graph::Workspace,
    source_stack: &Stack,
    subject_segment: &StackSegment,
    workspace_head: gix::ObjectId,
) -> anyhow::Result<DisconnectParameters> {
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
        .map(|segment_idx| &workspace.graph[segment_idx]);

    let parents_to_disconnect = if let Some(stack_base_segment) = stack_base_segment {
        // Base segment is part of the source stack.
        select_segment(editor, stack_base_segment)?
    } else if let Some(graph_base_segment) = graph_base_segment {
        // Base segment is outside of workspace (probably target branch).
        select_segment(editor, graph_base_segment)?
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

        return Ok(DisconnectParameters {
            delimiter,
            children_to_disconnect,
            parents_to_disconnect,
        });
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

    Ok(DisconnectParameters {
        delimiter,
        children_to_disconnect,
        parents_to_disconnect,
    })
}

/// Determine which parent to disconnect from the subject commit.
///
/// Preference rules:
/// - Prefer a `Pick` parent first, which aligns with linear first-parent ancestry.
/// - If no commit parent edge is found, fall back to a `Reference` parent.
///
/// If no explicit parent candidate exists, return `SelectorSet::All` as a safe fallback.
pub fn determine_parent_selector<'ws, 'meta, M: RefMetadata>(
    editor: &Editor<'ws, 'meta, M>,
    subject_commit_selector: Selector,
) -> anyhow::Result<SelectorSet> {
    let mut parents = editor.direct_parents(subject_commit_selector)?;
    parents.sort_by_key(|(_, order)| *order);

    let preferred = parents
        .iter()
        .find(|(selector, _)| matches!(editor.lookup_step(*selector), Ok(Step::Pick(_))))
        .or_else(|| {
            parents.iter().find(|(selector, _)| {
                matches!(editor.lookup_step(*selector), Ok(Step::Reference { .. }))
            })
        })
        .map(|(selector, _)| *selector);

    match preferred {
        Some(selector) => {
            let selectors = SomeSelectors::new(vec![selector])?;
            Ok(SelectorSet::Some(selectors))
        }
        None => Ok(SelectorSet::All),
    }
}

/// Select a segment by its ref name if available, otherwise fall back to its tip commit.
fn select_segment<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    segment: &impl SegmentLike,
) -> anyhow::Result<SelectorSet> {
    let selector = if let Some(ref_name) = segment.ref_name() {
        editor.select_reference(ref_name)?
    } else if let Some(tip) = segment.tip() {
        editor.select_commit(tip)?
    } else {
        bail!("Base segment has neither a ref name nor any commits.");
    };
    let selectors = SomeSelectors::new(vec![selector])?;
    Ok(SelectorSet::Some(selectors))
}

trait SegmentLike {
    fn ref_name(&self) -> Option<&gix::refs::FullNameRef>;
    fn tip(&self) -> Option<gix::ObjectId>;
}

impl SegmentLike for StackSegment {
    fn ref_name(&self) -> Option<&gix::refs::FullNameRef> {
        self.ref_name()
    }
    fn tip(&self) -> Option<gix::ObjectId> {
        self.tip()
    }
}

impl SegmentLike for but_graph::Segment {
    fn ref_name(&self) -> Option<&gix::refs::FullNameRef> {
        self.ref_name()
    }
    fn tip(&self) -> Option<gix::ObjectId> {
        self.tip()
    }
}
