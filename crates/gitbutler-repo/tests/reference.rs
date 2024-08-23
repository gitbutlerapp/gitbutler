use anyhow::Result;
use gitbutler_branch::VirtualBranchesHandle;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_repo::{
    create_branch_reference, credentials::Helper, list_branch_references, list_commit_references,
    push_branch_reference, update_branch_reference, LogUntil, RepoActionsExt,
};
use tempfile::TempDir;

#[test]
fn create_success() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let reference = create_branch_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/success".into(),
        test_ctx.commits.first().unwrap().id(),
    )?;
    assert_eq!(reference.branch_id, test_ctx.branch.id);
    assert_eq!(reference.upstream, "refs/remotes/origin/success".into());
    assert_eq!(reference.commit_id, test_ctx.commits.first().unwrap().id());
    assert_eq!(
        reference.change_id,
        test_ctx.commits.first().unwrap().change_id()
    );
    Ok(())
}

#[test]
fn create_multiple() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let first = create_branch_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/first".into(),
        test_ctx.commits.first().unwrap().id(),
    )?;
    assert_eq!(first.branch_id, test_ctx.branch.id);
    assert_eq!(first.upstream, "refs/remotes/origin/first".into());
    assert_eq!(first.commit_id, test_ctx.commits.first().unwrap().id());
    assert_eq!(
        first.change_id,
        test_ctx.commits.first().unwrap().change_id()
    );
    let last = create_branch_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/last".into(),
        test_ctx.commits.last().unwrap().id(),
    )?;
    assert_eq!(last.branch_id, test_ctx.branch.id);
    assert_eq!(last.upstream, "refs/remotes/origin/last".into());
    assert_eq!(last.commit_id, test_ctx.commits.last().unwrap().id());
    assert_eq!(last.change_id, test_ctx.commits.last().unwrap().change_id());
    Ok(())
}

#[test]
fn create_fails_with_non_remote_reference() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let result = create_branch_reference(
        &ctx,
        test_ctx.branch.id,
        "foo".into(),
        test_ctx.commits.first().unwrap().id(),
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
    create_branch_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/taken".into(),
        test_ctx.commits.first().unwrap().id(),
    )?;
    let result = create_branch_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/taken".into(),
        test_ctx.commits.last().unwrap().id(),
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
    create_branch_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/one".into(),
        test_ctx.commits.first().unwrap().id(),
    )?;
    let result = create_branch_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/two".into(),
        test_ctx.commits.first().unwrap().id(),
    );
    assert_eq!(
        result.unwrap_err().to_string(),
        format!(
            "A reference for commit {} already exists",
            test_ctx.commits.first().unwrap().id()
        ),
    );

    Ok(())
}

#[test]
fn create_fails_when_commit_in_anothe_branch() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let wrong_commit = test_ctx.other_commits.first().unwrap().id();
    let result = create_branch_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/asdf".into(),
        wrong_commit,
    );
    assert_eq!(
        result.unwrap_err().to_string(),
        format!(
            "The commit {} is not between the branch base and the branch head",
            wrong_commit
        ),
    );
    Ok(())
}

#[test]
fn create_fails_when_commit_is_the_base() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let result = create_branch_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/baz".into(),
        test_ctx.branch_base,
    );
    assert_eq!(
        result.unwrap_err().to_string(),
        format!(
            "The commit {} is not between the branch base and the branch head",
            test_ctx.branch_base
        ),
    );
    Ok(())
}

#[test]
fn list_success() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let first_ref = create_branch_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/first".into(),
        test_ctx.commits.first().unwrap().id(),
    )?;
    let second_ref = create_branch_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/second".into(),
        test_ctx.commits.last().unwrap().id(),
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
    create_branch_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/first".into(),
        test_ctx.commits.first().unwrap().id(),
    )?;
    let updated = update_branch_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/first".into(),
        test_ctx.commits.last().unwrap().id(),
    )?;
    assert_eq!(updated.commit_id, test_ctx.commits.last().unwrap().id());
    assert_eq!(
        updated.change_id,
        test_ctx.commits.last().unwrap().change_id()
    );
    let list = list_branch_references(&ctx, test_ctx.branch.id)?;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].commit_id, test_ctx.commits.last().unwrap().id());
    Ok(())
}

#[test]
fn push_success() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let reference = create_branch_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/first".into(),
        test_ctx.commits.first().unwrap().id(),
    )?;
    let result = push_branch_reference(
        &ctx,
        reference.branch_id,
        reference.upstream,
        false,
        &Helper::default(),
    );
    assert!(result.is_ok());
    Ok(())
}

#[test]
fn list_by_commits_success() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let first = create_branch_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/first".into(),
        test_ctx.commits.first().unwrap().id(),
    )?;
    let second = create_branch_reference(
        &ctx,
        test_ctx.branch.id,
        "refs/remotes/origin/second".into(),
        test_ctx.commits.last().unwrap().id(),
    )?;
    let third = create_branch_reference(
        &ctx,
        test_ctx.other_branch.id,
        "refs/remotes/origin/third".into(),
        test_ctx.other_commits.first().unwrap().id(),
    )?;
    let commits = vec![
        test_ctx.commits.first().unwrap().id(),
        test_ctx.commits.get(1).unwrap().id(),
        test_ctx.commits.last().unwrap().id(),
        test_ctx.other_commits.first().unwrap().id(),
    ];
    let result = list_commit_references(&ctx, commits.clone())?;
    assert_eq!(result.len(), 4);
    assert_eq!(result.get(&commits[0]).unwrap().clone().unwrap(), first);
    assert_eq!(result.get(&commits[1]).unwrap().clone(), None);
    assert_eq!(result.get(&commits[2]).unwrap().clone().unwrap(), second);
    assert_eq!(result.get(&commits[3]).unwrap().clone().unwrap(), third);
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
    let branch_base = ctx.repository().merge_base(target.sha, branch.head)?;
    let branch_commits = ctx.log(branch.head, LogUntil::Commit(target.sha))?;
    let other_commits = ctx.log(other_branch.head, LogUntil::Commit(target.sha))?;
    Ok(TestContext {
        branch: branch.clone(),
        branch_base,
        commits: branch_commits,
        other_branch: other_branch.clone(),
        other_commits,
    })
}
struct TestContext<'a> {
    branch: gitbutler_branch::Branch,
    branch_base: git2::Oid,
    commits: Vec<git2::Commit<'a>>,
    other_branch: gitbutler_branch::Branch,
    other_commits: Vec<git2::Commit<'a>>,
}
