//! Move a commit within or across branches and stacks.

pub(crate) mod function {
    use anyhow::{Context, bail};
    use but_rebase::graph_rebase::{
        Editor, SuccessfulRebase, ToCommitSelector, ToSelector,
        mutate::{InsertSide, SegmentDelimiter, SelectorSet, SomeSelectors},
    };

    /// Move a commit.
    ///
    /// `editor` is assumed to have been generated from the given `workspace`
    /// and therefore aligned.
    ///
    /// `workspace` - Used for getting the surrounding context of the commit being moved.
    ///     In the future, we should not rely on the projection and do it fully on the graph.
    ///
    /// `subject_commit` - The commit to be moved.
    ///
    /// `anchor` - A git graph node selector to move the subject commit relative to.
    ///
    /// `side` - The side relative to the anchor at which to insert the subject commit.
    ///
    /// The subject commit will be detached from the source segment, and inserted relative
    /// to a given anchor (branch or commit).
    pub fn move_commit(
        mut editor: Editor,
        workspace: &but_graph::projection::Workspace,
        subject_commit: impl ToCommitSelector,
        anchor: impl ToSelector,
        side: InsertSide,
    ) -> anyhow::Result<SuccessfulRebase> {
        let (subject_commit_selector, subject_commit) =
            editor.find_selectable_commit(subject_commit)?;

        let subject = retrieve_commit_and_containers(workspace, &subject_commit)?;

        let (source_stack, source_segment, _) = subject;

        let commit_delimiter = SegmentDelimiter {
            child: subject_commit_selector,
            parent: subject_commit_selector,
        };

        // Step 1: Determine the parents and children to disconnect.
        let index_of_subject_commit = source_segment
            .commits
            .iter()
            .position(|commit| commit.id == subject_commit.id)
            .context("BUG: Subject commit is not in the source segment.")?;

        let child_to_disconnect =
            determine_child_selector(&editor, source_segment, index_of_subject_commit)?;

        let parent_to_disconnect = determine_parent_selector(
            workspace,
            &editor,
            source_stack,
            source_segment,
            index_of_subject_commit,
        )?;

        // Step 2: Disconnect
        editor.disconnect_segment_from(
            commit_delimiter.clone(),
            child_to_disconnect,
            parent_to_disconnect,
            false,
        )?;

        // Step 3: Insert
        editor.insert_segment(anchor, commit_delimiter, side)?;
        editor.rebase()
    }

    /// Determine the surrounding context of the commit to be moved
    ///
    /// Currently, this looks into the workspace projection in order to determine **where to take the commit from**.
    ///
    /// ### The issue
    /// It's impossible to know for sure what is the exact intention of 'moving a commit' inside a complex git graph.
    /// A commit, can have N children and M parents. 'Moving' it somewhere else can imply:
    /// - Disconnecting all parents and children, and inserting it somewhere else.
    /// - Disconnecting the first parent and all children, and then inserting.
    /// - Disconnecting *some* parents and *some* children, and then inserting it.
    ///
    /// ### The GitButler assumption
    /// In the context of a GitButler workspace (as of this writing), we want to disconnect the commit from the linear
    /// segments and move them to another position in the same or other segment. That way, any other parents and
    /// children that are not part of the source segment are kept.
    ///
    /// ### What the future holds
    /// In the future, where we're not afraid of complex graphs, we've figured out UX and data wrangling,
    /// the concept of a segment might not hold, and hence we'll have to figure out a better way of determining
    /// what to cut (e.g. letting the clients decide what to cut).
    fn retrieve_commit_and_containers<'a>(
        workspace: &'a but_graph::projection::Workspace,
        subject_commit: &but_core::CommitOwned,
    ) -> anyhow::Result<(
        &'a but_graph::projection::Stack,
        &'a but_graph::projection::StackSegment,
        &'a but_graph::projection::StackCommit,
    )> {
        let Some(subject) = workspace.find_commit_and_containers(subject_commit.id) else {
            bail!("Failed to find the commit to move in the workspace.");
        };
        Ok(subject)
    }

    /// Determine which children to disconnect, based on the position of the subject commit in the segment.
    fn determine_child_selector(
        editor: &Editor,
        source_segment: &but_graph::projection::StackSegment,
        index_of_subject_commit: usize,
    ) -> Result<SelectorSet, anyhow::Error> {
        let child_to_disconnect = if index_of_subject_commit == 0 {
            // The commit is at the top of the branch.
            // We just need to disconnect the ref from it.
            let ref_name = source_segment
                .ref_name()
                .context("Source segment doesn't have a reference name.")?;
            let reference_selector = editor.select_reference(ref_name)?;
            let selectors = SomeSelectors::new(vec![reference_selector])?;
            SelectorSet::Some(selectors)
        } else if let Some(child_of_subject) =
            source_segment.commits.get(index_of_subject_commit - 1)
        {
            let child_commit_selector = editor.select_commit(child_of_subject.id)?;
            let selectors = SomeSelectors::new(vec![child_commit_selector])?;
            SelectorSet::Some(selectors)
        } else {
            bail!(
                "BUG: Subject commit is not the first child in segment but also can't find its child."
            );
        };
        Ok(child_to_disconnect)
    }

    /// Determine which parents to disconnect, based on the position of the subject commit in the segment
    /// and the position of the source segment in the source stack.
    fn determine_parent_selector(
        workspace: &but_graph::projection::Workspace,
        editor: &Editor,
        source_stack: &but_graph::projection::Stack,
        source_segment: &but_graph::projection::StackSegment,
        index_of_subject_commit: usize,
    ) -> Result<SelectorSet, anyhow::Error> {
        let parent_to_disconnect = if index_of_subject_commit == source_segment.commits.len() - 1 {
            // Subject commit is the last one, then we need to disconnect the parent ref.
            // If this is `None` but the `base_segment_id` is defined, it means that the parent segment lives
            // outside the workspace and it's probably the target branch.
            let stack_base_segment_ref_name =
                source_segment.base_segment_id.and_then(|base_segment_id| {
                    source_stack.segments.iter().find_map(|segment| {
                        if segment.id == base_segment_id {
                            segment.ref_name()
                        } else {
                            None
                        }
                    })
                });

            // Look for the base segment in the graph data, as a fallback if there's no stack segment found.
            let graph_base_segment_ref_name = source_segment
                .base_segment_id
                .map(|base_segment_id| &workspace.graph[base_segment_id])
                .and_then(|segment| segment.ref_name());

            match stack_base_segment_ref_name.or(graph_base_segment_ref_name) {
                Some(ref_name) => {
                    let reference_selector = editor.select_reference(ref_name)?;
                    let selectors = SomeSelectors::new(vec![reference_selector])?;
                    SelectorSet::Some(selectors)
                }
                None => {
                    // No explicit base segment/ref available (e.g., root commit or traversal stopped early).
                    // Fall back to disconnecting from all parents; downstream code can handle this.
                    SelectorSet::All
                }
            }
        } else if let Some(parent_of_subject) =
            source_segment.commits.get(index_of_subject_commit + 1)
        {
            let parent_commit_selector = editor.select_commit(parent_of_subject.id)?;
            let selectors = SomeSelectors::new(vec![parent_commit_selector])?;
            SelectorSet::Some(selectors)
        } else {
            bail!(
                "BUG: Subject commit is not the last commit in the segment but can't find its parent either."
            )
        };
        Ok(parent_to_disconnect)
    }
}
