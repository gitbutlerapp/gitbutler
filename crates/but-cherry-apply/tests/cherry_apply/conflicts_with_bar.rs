use super::*;
use crate::util::test_ctx;

#[test]
fn status_is_locked_to_bar() -> anyhow::Result<()> {
    let test_ctx = test_ctx("conflicts-with-bar")?;

    let repo = test_ctx.ctx.repo.get()?;
    let commit_id = repo
        .rev_parse_single("refs/gitbutler/bar-conflict")?
        .detach();

    let status = test_ctx.get_status(commit_id)?;

    let bar_id = test_ctx
        .handle
        .list_stacks_in_workspace()?
        .iter()
        .find(|s| s.name == "bar")
        .unwrap()
        .id;

    assert_eq!(status, CherryApplyStatus::LockedToStack(bar_id));

    Ok(())
}

#[test]
fn can_only_apply_to_bar_stack() -> anyhow::Result<()> {
    let test_ctx = test_ctx("conflicts-with-bar")?;

    let repo = test_ctx.ctx.repo.get()?;
    let commit_id = repo
        .rev_parse_single("refs/gitbutler/bar-conflict")?
        .detach();

    let bar_id = test_ctx
        .handle
        .list_stacks_in_workspace()?
        .iter()
        .find(|s| s.name == "bar")
        .unwrap()
        .id;

    // Apply to bar should succeed
    test_ctx.apply(commit_id, bar_id)?;

    // Verify the commit is now in the bar stack by checking for its message
    let meta = VirtualBranchesTomlMetadata::from_path(
        test_ctx
            .ctx
            .legacy_project
            .gb_dir()
            .join("virtual_branches.toml"),
    )?;
    let details = stack_details_v3(Some(bar_id), &repo, &meta)?;

    let has_commit = details
        .branch_details
        .iter()
        .flat_map(|branch| &branch.commits)
        .any(|commit| {
            commit
                .message
                .to_string()
                .contains("Conflicting change to bar.txt")
        });

    assert!(
        has_commit,
        "Expected to find cherry-picked commit in bar stack"
    );

    Ok(())
}
