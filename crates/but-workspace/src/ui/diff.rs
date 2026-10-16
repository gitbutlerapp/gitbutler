use but_core::ui;

/// Obtain the changes made to the tip of `branch` in `repo` compared to a base that is either extracted
/// from `workspace` or from the merge base of `branch` and the selected workspace cutoff, if present.
// TODO: It would be more consistent if the UI would use `but_core::diff::ui::changes_in_range()` directly,
//       that way computations of merge-bases/commits don't have to be aligned.
pub fn changes_in_branch(
    repo: &gix::Repository,
    workspace: &but_graph::Workspace,
    branch: &gix::refs::FullNameRef,
) -> anyhow::Result<ui::TreeChanges> {
    let commits = if let Some(segment) = workspace.find_segment_by_refname(branch) {
        let base = segment.base;
        segment.tip().zip(base)
    } else {
        // TODO: this should be (kept) in sync with branch-listing!
        let tip = repo.find_reference(branch)?.peel_to_commit()?.id;
        workspace.target_commit.as_ref().and_then(|target| {
            // NOTE: Can't do merge-base computation in graph as `branch` might not be contained in it.
            repo.merge_base(tip, target.commit_id)
                .ok()
                .flatten()
                .map(|base| (tip, base.detach()))
        })
    };

    let Some((tip, base)) = commits else {
        return Ok(ui::TreeChanges::default());
    };
    but_core::diff::ui::changes_with_line_stats_in_range(repo, tip, base)
}
