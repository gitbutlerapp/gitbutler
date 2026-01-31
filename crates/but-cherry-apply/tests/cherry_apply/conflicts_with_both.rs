use super::*;
use crate::util::test_ctx;

#[test]
fn status_is_causes_workspace_conflict() -> anyhow::Result<()> {
    let mut test_ctx = test_ctx("conflicts-with-both")?;

    let repo = test_ctx.ctx.repo.get()?;
    let commit_id = repo
        .rev_parse_single("refs/gitbutler/both-conflict")?
        .detach();

    drop(repo);
    let status = test_ctx.get_status(commit_id)?;

    assert_eq!(status, CherryApplyStatus::CausesWorkspaceConflict);

    Ok(())
}

#[test]
fn cannot_apply_to_foo_stack() -> anyhow::Result<()> {
    let mut test_ctx = test_ctx("conflicts-with-both")?;

    let repo = test_ctx.ctx.repo.get()?;
    let commit_id = repo
        .rev_parse_single("refs/gitbutler/both-conflict")?
        .detach();

    let foo_id = test_ctx
        .handle
        .list_stacks_in_workspace()?
        .iter()
        .find(|s| s.name() == "foo")
        .unwrap()
        .id;

    // Apply should fail
    drop(repo);
    let result = test_ctx.apply(commit_id, foo_id);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("causes workspace conflicts")
    );

    Ok(())
}

#[test]
fn cannot_apply_to_bar_stack() -> anyhow::Result<()> {
    let mut test_ctx = test_ctx("conflicts-with-both")?;

    let repo = test_ctx.ctx.repo.get()?;
    let commit_id = repo
        .rev_parse_single("refs/gitbutler/both-conflict")?
        .detach();

    let bar_id = test_ctx
        .handle
        .list_stacks_in_workspace()?
        .iter()
        .find(|s| s.name() == "bar")
        .unwrap()
        .id;

    // Apply should fail
    drop(repo);
    let result = test_ctx.apply(commit_id, bar_id);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("causes workspace conflicts")
    );

    Ok(())
}
