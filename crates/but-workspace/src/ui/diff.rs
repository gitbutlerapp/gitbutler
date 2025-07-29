use but_core::ui;

/// Obtain the changes made to the tip of `branch` in `repo` compared to a base that is either extracted
/// from `workspace` or from the intersection of the `branch` with the workspace target ref, if present.
// TODO: It would be more consistent if the UI would use `but_core::diff::ui::changes_in_range()` directly,
//       that way computations of merge-bases/commits don't have to be aligned.
pub fn changes_in_branch(
    repo: &gix::Repository,
    workspace: &but_graph::projection::Workspace<'_>,
    branch: &gix::refs::FullNameRef,
) -> anyhow::Result<ui::TreeChanges> {
    let commits =
        if let Some((stack, segment)) = workspace.find_segment_and_stack_by_refname(branch) {
            let base = stack.base();
            segment.tip().zip(base)
        } else {
            let tip = repo.find_reference(branch)?.peel_to_commit()?.id;
            workspace.lower_bound.and_then(|lower_bound| {
                // This works because the lower-bound itself is the merge-base
                // between all applicable targets and the workspace branches.
                repo.merge_base(tip, lower_bound)
                    .ok()
                    .map(|base| (tip, base.detach()))
            })
        };

    let Some((tip, base)) = commits else {
        return Ok(ui::TreeChanges::default());
    };
    but_core::diff::ui::changes_in_range(repo, tip, base)
}
