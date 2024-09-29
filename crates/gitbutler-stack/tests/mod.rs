use anyhow::Result;
use gitbutler_branch::VirtualBranchesHandle;
use gitbutler_command_context::CommandContext;
use gitbutler_patch_reference::{CommitOrChangeId, PatchReference};
use gitbutler_repo::{LogUntil, RepositoryExt as _};
use gitbutler_stack::Stack;
use tempfile::TempDir;

#[test]
fn init_success() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let mut branch = test_ctx.branch;
    assert!(!branch.initialized());
    assert_eq!(branch.heads.len(), 0);
    let result = branch.init(&ctx);
    assert!(result.is_ok());
    assert!(branch.initialized());
    assert_eq!(branch.heads.len(), 1);
    assert_eq!(branch.heads[0].name, "virtual"); // matches the stack name
    assert_eq!(
        branch.heads[0].target,
        CommitOrChangeId::CommitId(branch.head.to_string())
    );
    // Assert persisted
    assert_eq!(branch, test_ctx.handle.get_branch(branch.id)?);
    Ok(())
}

#[test]
fn init_already_initialized_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let mut branch = test_ctx.branch;
    let result = branch.init(&ctx);
    assert!(result.is_ok());
    let result = branch.init(&ctx);
    assert!(result.is_err());
    Ok(())
}

#[test]
fn add_branch_success() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let reference = PatchReference {
        name: "asdf".into(),
        target: CommitOrChangeId::CommitId(test_ctx.commits[0].id().to_string()),
    };
    let result = test_ctx.branch.add_branch(&ctx, reference);
    assert!(result.is_ok());
    assert_eq!(test_ctx.branch.heads.len(), 2);
    // Assert persisted
    assert_eq!(
        test_ctx.branch,
        test_ctx.handle.get_branch(test_ctx.branch.id)?
    );
    Ok(())
}

#[test]
fn add_branch_uninitialized_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let reference = PatchReference {
        name: "asdf".into(),
        target: CommitOrChangeId::CommitId(test_ctx.commits[0].id().to_string()),
    };
    let result = test_ctx.branch.add_branch(&ctx, reference);
    assert_eq!(
        result.err().unwrap().to_string(),
        "Stack has not been initialized"
    );
    Ok(())
}

#[test]
fn add_branch_invalid_name_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let reference = PatchReference {
        name: "name with spaces".into(),
        target: CommitOrChangeId::CommitId(test_ctx.commits[0].id().to_string()),
    };
    let result = test_ctx.branch.add_branch(&ctx, reference);
    assert_eq!(result.err().unwrap().to_string(), "Invalid branch name");
    Ok(())
}

#[test]
fn add_branch_duplicate_name_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let reference = PatchReference {
        name: "asdf".into(),
        target: CommitOrChangeId::CommitId(test_ctx.commits[0].id().to_string()),
    };
    let result = test_ctx.branch.add_branch(&ctx, reference.clone());
    assert!(result.is_ok());
    let result = test_ctx.branch.add_branch(&ctx, reference);
    assert_eq!(
        result.err().unwrap().to_string(),
        "A patch reference with the name asdf exists"
    );
    Ok(())
}

#[test]
fn add_branch_matching_git_ref_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let reference = PatchReference {
        name: "existing-branch".into(),
        target: CommitOrChangeId::CommitId(test_ctx.commits[0].id().to_string()),
    };
    let result = test_ctx.branch.add_branch(&ctx, reference.clone());
    assert_eq!(
        result.err().unwrap().to_string(),
        "A git reference with the name existing-branch exists"
    );
    Ok(())
}

#[test]
fn add_branch_including_refs_head_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let reference = PatchReference {
        name: "refs/heads/my-branch".into(),
        target: CommitOrChangeId::CommitId(test_ctx.commits[0].id().to_string()),
    };
    let result = test_ctx.branch.add_branch(&ctx, reference.clone());
    assert_eq!(
        result.err().unwrap().to_string(),
        "Stack head name cannot start with 'refs/heads'"
    );
    Ok(())
}

#[test]
fn add_branch_target_commit_doesnt_exist() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let reference = PatchReference {
        name: "my-branch".into(),
        target: CommitOrChangeId::CommitId("30696678319e0fa3a20e54f22d47fc8cf1ceaade".into()), // does not exist
    };
    let result = test_ctx.branch.add_branch(&ctx, reference.clone());
    assert!(result
        .err()
        .unwrap()
        .to_string()
        .contains("object not found"),);
    Ok(())
}

#[test]
fn add_branch_target_change_id_doesnt_exist() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let reference = PatchReference {
        name: "my-branch".into(),
        target: CommitOrChangeId::ChangeId("does-not-exist".into()), // does not exist
    };
    let result = test_ctx.branch.add_branch(&ctx, reference.clone());
    assert_eq!(
        result.err().unwrap().to_string(),
        "Commit with change id does-not-exist not found"
    );
    Ok(())
}

#[test]
fn add_branch_target_commit_not_in_stack() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let other_commit_id = test_ctx.other_commits.last().unwrap().id().to_string();
    let reference = PatchReference {
        name: "my-branch".into(),
        target: CommitOrChangeId::CommitId(other_commit_id.clone()), // does not exist
    };
    let result = test_ctx.branch.add_branch(&ctx, reference.clone());
    assert_eq!(
        result.err().unwrap().to_string(),
        format!(
            "The commit {} is not between the stack head and the stack base",
            other_commit_id
        )
    );
    Ok(())
}

#[test]
fn remove_branch_uninitialized_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let result = test_ctx.branch.remove_branch(&ctx, "some-name".to_string());
    assert_eq!(
        result.err().unwrap().to_string(),
        "Stack has not been initialized"
    );
    Ok(())
}

#[test]
fn remove_branch_last_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let result = test_ctx
        .branch
        .remove_branch(&ctx, test_ctx.branch.heads[0].name.clone());
    assert_eq!(
        result.err().unwrap().to_string(),
        "Cannot remove the last branch from the stack"
    );
    Ok(())
}

#[test]
fn remove_branch_nonexistent_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let result = test_ctx
        .branch
        .remove_branch(&ctx, "does-not-exist".to_string());
    assert_eq!(
        result.err().unwrap().to_string(),
        "Series with name does-not-exist not found"
    );
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
        handle,
    })
}
struct TestContext<'a> {
    branch: gitbutler_branch::Branch,
    commits: Vec<git2::Commit<'a>>,
    #[allow(dead_code)]
    other_commits: Vec<git2::Commit<'a>>,
    handle: VirtualBranchesHandle,
}
