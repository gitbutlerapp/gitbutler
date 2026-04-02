use but_graph::{SegmentIndex, projection::Workspace};

/// Find the owning graph segment for `commit_id` in `workspace`.
///
/// This uses the stack segment's `commits_by_segment` offsets to map a projected
/// commit back to its source graph segment.
pub(crate) fn find_commit_segment_index(
    workspace: &Workspace,
    commit_id: gix::ObjectId,
) -> Option<SegmentIndex> {
    let (_, stack_segment, _) = workspace.find_commit_and_containers(commit_id)?;
    let commit_offset = stack_segment
        .commits
        .iter()
        .position(|c| c.id == commit_id)?;

    let mut owning_segment = stack_segment.id;
    for (segment_id, offset) in &stack_segment.commits_by_segment {
        if *offset > commit_offset {
            break;
        }
        owning_segment = *segment_id;
    }

    Some(owning_segment)
}
