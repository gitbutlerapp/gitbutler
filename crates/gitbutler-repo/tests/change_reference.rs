use anyhow::Result;
use gitbutler_branch::VirtualBranchesHandle;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_repo::{
    create_change_reference, list_branch_references, push_change_reference,
    update_change_reference, LogUntil, RepositoryExt as _,
};
use tempfile::TempDir;

#[test]
fn create_success() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let reference = create_change_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/success".into(),
        test_ctx.commits.first().unwrap().change_id().unwrap(),
    )?;
    assert_eq!(reference.branch_id, test_ctx.branch.id);
    assert_eq!(reference.name, "refs/remotes/origin/success".into());
    assert_eq!(
        reference.change_id,
        test_ctx.commits.first().unwrap().change_id().unwrap()
    );
    Ok(())
}

#[test]
fn create_multiple() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let first = create_change_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/first".into(),
        test_ctx.commits.first().unwrap().change_id().unwrap(),
    )?;
    assert_eq!(first.branch_id, test_ctx.branch.id);
    assert_eq!(first.name, "refs/remotes/origin/first".into());
    assert_eq!(
        first.change_id,
        test_ctx.commits.first().unwrap().change_id().unwrap()
    );
    let last = create_change_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/last".into(),
        test_ctx.commits.last().unwrap().change_id().unwrap(),
    )?;
    assert_eq!(last.branch_id, test_ctx.branch.id);
    assert_eq!(last.name, "refs/remotes/origin/last".into());
    assert_eq!(
        last.change_id,
        test_ctx.commits.last().unwrap().change_id().unwrap()
    );
    Ok(())
}

#[test]
fn create_fails_with_non_remote_reference() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let result = create_change_reference(
        &ctx,
        test_ctx.branch.id,
        "foo".into(),
        test_ctx.commits.first().unwrap().change_id().unwrap(),
    );
    assert_eq!(
        result.unwrap_err().to_string(),
        "Failed to parse the provided reference",
    );
    Ok(())
}

#[test]
fn create_fails_when_branch_reference_with_name_exists() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    create_change_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/taken".into(),
        test_ctx.commits.first().unwrap().change_id().unwrap(),
    )?;
    let result = create_change_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/taken".into(),
        test_ctx.commits.last().unwrap().change_id().unwrap(),
    );
    assert_eq!(
        result.unwrap_err().to_string(),
        format!("A reference refs/remotes/origin/taken already exists",),
    );

    Ok(())
}

#[test]
fn create_fails_when_commit_already_referenced() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    create_change_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/one".into(),
        test_ctx.commits.first().unwrap().change_id().unwrap(),
    )?;
    let result = create_change_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/two".into(),
        test_ctx.commits.first().unwrap().change_id().unwrap(),
    );
    assert_eq!(
        result.unwrap_err().to_string(),
        format!(
            "A reference for change {} already exists",
            test_ctx.commits.first().unwrap().change_id().unwrap()
        ),
    );

    Ok(())
}

#[test]
fn create_fails_when_commit_in_anothe_branch() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let wrong_change = test_ctx.other_commits.first().unwrap().change_id().unwrap();
    let result = create_change_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/asdf".into(),
        wrong_change.clone(),
    );
    assert!(result.is_err());
    Ok(())
}

#[test]
fn create_fails_when_change_id_not_found() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let result = create_change_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/baz".into(),
        "does-not-exist".into(),
    );
    assert!(result.is_err());
    Ok(())
}

#[test]
fn list_success() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let first_ref = create_change_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/first".into(),
        test_ctx.commits.first().unwrap().change_id().unwrap(),
    )?;
    let second_ref = create_change_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/second".into(),
        test_ctx.commits.last().unwrap().change_id().unwrap(),
    )?;
    let result = list_branch_references(&ctx, test_ctx.branch.id)?;
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], first_ref);
    assert_eq!(result[1], second_ref);
    Ok(())
}

#[test]
fn update_success() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    create_change_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/first".into(),
        test_ctx.commits.first().unwrap().change_id().unwrap(),
    )?;
    let updated = update_change_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/first".into(),
        test_ctx.commits.last().unwrap().change_id().unwrap(),
    )?;
    assert_eq!(
        updated.change_id,
        test_ctx.commits.last().unwrap().change_id().unwrap()
    );
    let list = list_branch_references(&ctx, test_ctx.branch.id)?;
    assert_eq!(list.len(), 1);
    assert_eq!(
        list[0].change_id,
        test_ctx.commits.last().unwrap().change_id().unwrap()
    );
    Ok(())
}

#[test]
fn push_success() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let reference = create_change_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/first".into(),
        test_ctx.commits.first().unwrap().change_id().unwrap(),
    )?;
    let result = push_change_reference(&ctx, reference.branch_id, reference.name, false);
    assert!(result.is_ok());
    Ok(())
}

fn command_ctx(name: &str) -> Result<(CommandContext, TempDir)> {
    gitbutler_testsupport::writable::fixture("stacking.sh", name)
}

fn test_ctx(ctx: &CommandContext) -> Result<TestContext> {
    let handle = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let branches = handle.list_all_branches()?;
    let branch = branches.iter().find(|b| b.name == "virtual").unwrap();
    let other_branch = branches.iter().find(|b| b.name != "virtual").unwrap();
    let target = handle.get_default_target()?;
    let branch_commits = ctx
        .repository()
        .log(branch.head, LogUntil::Commit(target.sha))?;
    let other_commits = ctx
        .repository()
        .log(other_branch.head, LogUntil::Commit(target.sha))?;
    Ok(TestContext {
        branch: branch.clone(),
        commits: branch_commits,
        // other_branch: other_branch.clone(),
        other_commits,
    })
}
struct TestContext<'a> {
    branch: gitbutler_branch::Branch,
    commits: Vec<git2::Commit<'a>>,
    other_commits: Vec<git2::Commit<'a>>,
}
