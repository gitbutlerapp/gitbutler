use anyhow::Result;
use bstr::ByteSlice;
use but_ctx::Context;
use but_oxidize::ObjectIdExt;
use but_workspace::ui::Commit;
use gitbutler_branch_actions::squash_commits;
use gitbutler_stack::{StackBranch, VirtualBranchesHandle};
use itertools::Itertools;
use tempfile::TempDir;

// Squash commit into it's parent without affecting stack heads
//
// - commit 5 (a-branch-3)
// - commit 4 (a-branch-2)
// - commit 3              ──┐
// - commit 2             ◄──┘
// - commit 1 (a-branch-1)
//
// Result:
// - commit 5 (a-branch-3)
// - commit 3 (a-branch-2)
// - commit 2+3
// - commit 1  (a-branch-1)
#[test]
fn squash_without_affecting_stack() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx()?;
    let test = test_ctx(&ctx)?;
    squash_commits(&ctx, test.stack.id, vec![test.commit_3], test.commit_2)?;

    let branches = list_branches(&ctx)?;
    // branch 1
    assert_eq!(branches.b1.patches.len(), 1);
    assert_eq!(branches.b1.patches[0].message, "commit 1");

    // branch 2
    assert_eq!(branches.b2.patches.len(), 2);
    assert_eq!(branches.b2.patches[0].message, "commit 4");
    assert_eq!(branches.b2.patches[1].message, "commit 2\ncommit 3");
    assert_eq!(
        blob_content(&*ctx.git2_repo.get()?, branches.b2.patches[1].id, "file2_3")?,
        "change3\n"
    );

    // branch 3
    assert_eq!(branches.b3.patches.len(), 1);
    assert_eq!(branches.b3.patches[0].message, "commit 5");
    Ok(())
}

// Squash commit into another commit below it that not an immediate parent
// Also asserts: Squash commit that is a stacked branch head makes the parent the new head
//
// - commit 5 (a-branch-3)
// - commit 4 (a-branch-2) ──┐
// - commit 3                │
// - commit 2             ◄──┘
// - commit 1 (a-branch-1)
//
// Result:
// - commit 5 (a-branch-3)
// - commit 3 (a-branch-2)
// - commit 2+4
// - commit 1  (a-branch-1)
#[test]
fn squash_below() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx()?;
    let test = test_ctx(&ctx)?;
    squash_commits(&ctx, test.stack.id, vec![test.commit_4], test.commit_2)?;

    let branches = list_branches(&ctx)?;
    // branch 1
    assert_eq!(branches.b1.patches.len(), 1);
    assert_eq!(branches.b1.patches[0].message, "commit 1");

    // branch 2
    assert_eq!(branches.b2.patches.len(), 2);
    assert_eq!(branches.b2.patches[0].message, "commit 3");
    assert_eq!(branches.b2.patches[1].message, "commit 2\ncommit 4");
    assert_eq!(
        blob_content(&*ctx.git2_repo.get()?, branches.b2.patches[1].id, "file2_3")?,
        "change2\n"
    );
    assert_eq!(
        blob_content(&*ctx.git2_repo.get()?, branches.b2.patches[1].id, "file4")?,
        "change4\n"
    );

    // branch 3
    assert_eq!(branches.b3.patches.len(), 1);
    assert_eq!(branches.b3.patches[0].message, "commit 5");
    Ok(())
}

// Squash commit into another commit above it that not an immediate parent
// Also asserts: Squash commit that is a stacked branch head can leave an empty branch if it was the only commit
// Also asserts: Squash commit that can result in the bottom stacked branch becoming empty
//
// - commit 5 (a-branch-3)
// - commit 4 (a-branch-2)
// - commit 3             ◄──┐
// - commit 2                │
// - commit 1 (a-branch-1) ──┘
//
// Result:
// - commit 5 (a-branch-3)
// - commit 4 (a-branch-2)
// - commit 3+1
// - commit 2
// - base     (a-branch-1)
#[test]
fn squash_above() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx()?;
    let test = test_ctx(&ctx)?;
    squash_commits(&ctx, test.stack.id, vec![test.commit_1], test.commit_3)?;

    let branches = list_branches(&ctx)?;
    // branch 1
    assert_eq!(branches.b1.patches.len(), 0);

    // branch 2
    assert_eq!(branches.b2.patches.len(), 3);
    assert_eq!(branches.b2.patches[0].message, "commit 4");
    assert_eq!(branches.b2.patches[1].message, "commit 3\ncommit 1");
    assert_eq!(
        blob_content(&*ctx.git2_repo.get()?, branches.b2.patches[1].id, "file2_3")?,
        "change3\n"
    );
    assert_eq!(
        blob_content(&*ctx.git2_repo.get()?, branches.b2.patches[1].id, "file1")?,
        "change1\n"
    );
    assert_eq!(branches.b2.patches[2].message, "commit 2");

    // branch 3
    assert_eq!(branches.b3.patches.len(), 1);
    assert_eq!(branches.b3.patches[0].message, "commit 5");
    Ok(())
}

// Squash up commit into another with the result being a delightful squashed commit
//
// - commit 5 (a-branch-3)
// - commit 4 (a-branch-2)
// - commit 3             ◄──┐
// - commit 2              ──┘
// - commit 1 (a-branch-1)
//
// Commits 3 and 2 update the same file and line number
// Result:
// - commit 5 (a-branch-3)
// - commit 3 (a-branch-2)
// - commit 3+2
// - commit 1  (a-branch-1)
#[test]
fn squash_upwards_works() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx()?;
    let test = test_ctx(&ctx)?;
    squash_commits(&ctx, test.stack.id, vec![test.commit_2], test.commit_3)?;

    let branches = list_branches(&ctx)?;
    // branch 1
    assert_eq!(branches.b1.patches.len(), 1);
    assert_eq!(branches.b1.patches[0].message, "commit 1");

    // branch 2
    assert_eq!(branches.b2.patches.len(), 2);
    assert_eq!(branches.b2.patches[0].message, "commit 4");
    assert_eq!(branches.b2.patches[1].message, "commit 3\ncommit 2");
    assert_eq!(
        blob_content(&*ctx.git2_repo.get()?, branches.b2.patches[1].id, "file2_3")?,
        "change3\n"
    );

    // branch 3
    assert_eq!(branches.b3.patches.len(), 1);
    assert_eq!(branches.b3.patches[0].message, "commit 5");
    Ok(())
}

// Squash down commit into another with overlap is ok
//
// - commit 5 (a-branch-3)
// - commit 4 (a-branch-2)
// - commit 3              ──┐
// - commit 2             ◄──┘
// - commit 1 (a-branch-1)
//
// Result
// - commit 5 (a-branch-3)
// - commit 4 (a-branch-2)
// - commit 2+3
// - commit 1 (a-branch-1)
//
// Commits 3 and 2 update the same file and line number
#[test]
fn squash_down_with_overlap_ok() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx()?;
    let test = test_ctx(&ctx)?;
    squash_commits(&ctx, test.stack.id, vec![test.commit_3], test.commit_2)?;
    let branches = list_branches(&ctx)?;

    // branch 1
    assert_eq!(branches.b1.patches.len(), 1);
    assert_eq!(branches.b1.patches[0].message, "commit 1");

    // branch 2
    assert_eq!(branches.b2.patches.len(), 2);
    assert_eq!(branches.b2.patches[0].message, "commit 4");
    assert_eq!(branches.b2.patches[1].message, "commit 2\ncommit 3");
    assert_eq!(
        blob_content(&*ctx.git2_repo.get()?, branches.b2.patches[1].id, "file2_3")?,
        "change3\n"
    );

    // branch 3
    assert_eq!(branches.b3.patches.len(), 1);
    assert_eq!(branches.b3.patches[0].message, "commit 5");
    Ok(())
}

// Squash a commit into a commit that itself is a stacked branch head
//
// - commit 5 (a-branch-3)
// - commit 4 (a-branch-2) ──┐
// - commit 3                │
// - commit 2                │
// - commit 1 (a-branch-1)◄──┘
//
// Result:
// - commit 5   (a-branch-3)
// - commit 3   (a-branch-2)
// - commit 2
// - commit 1+4 (a-branch-1)
#[test]
fn squash_below_into_stack_head() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx()?;
    let test = test_ctx(&ctx)?;
    squash_commits(&ctx, test.stack.id, vec![test.commit_4], test.commit_1)?;
    let branches = list_branches(&ctx)?;

    // branch 1
    assert_eq!(branches.b1.patches.len(), 1);
    assert_eq!(branches.b1.patches[0].message, "commit 1\ncommit 4");
    assert_eq!(
        blob_content(&*ctx.git2_repo.get()?, branches.b1.patches[0].id, "file4")?,
        "change4\n"
    );
    assert_eq!(
        blob_content(&*ctx.git2_repo.get()?, branches.b1.patches[0].id, "file1")?,
        "change1\n"
    );

    // branch 2
    assert_eq!(branches.b2.patches.len(), 2);
    assert_eq!(branches.b2.patches[0].message, "commit 3");
    assert_eq!(branches.b2.patches[1].message, "commit 2");

    // branch 3
    assert_eq!(branches.b3.patches.len(), 1);
    assert_eq!(branches.b3.patches[0].message, "commit 5");
    Ok(())
}

// Squash multiple commits into another commit below it that not an immediate parent to either
// Also assert: Squash multiple commits where one is a stacked branch head
//
// - commit 5 (a-branch-3)
// - commit 4 (a-branch-2) ──┐
// - commit 3                │
// - commit 2              ──┤
// - commit 1 (a-branch-1)◄──┘
//
// Result:
// - commit 5     (a-branch-3)
// - commit 3     (a-branch-2)
// - commit 1+4+2 (a-branch-1)
#[test]
fn squash_multiple() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx()?;
    let test = test_ctx(&ctx)?;
    squash_commits(
        &ctx,
        test.stack.id,
        vec![test.commit_4, test.commit_2],
        test.commit_1,
    )?;
    let branches = list_branches(&ctx)?;

    // branch 1
    assert_eq!(branches.b1.patches.len(), 1);
    assert_eq!(
        branches.b1.patches[0].message,
        "commit 1\ncommit 4\ncommit 2"
    );
    assert_eq!(
        blob_content(&*ctx.git2_repo.get()?, branches.b1.patches[0].id, "file4")?,
        "change4\n"
    );
    assert_eq!(
        blob_content(&*ctx.git2_repo.get()?, branches.b1.patches[0].id, "file2_3")?,
        "change2\n"
    );
    assert_eq!(
        blob_content(&*ctx.git2_repo.get()?, branches.b1.patches[0].id, "file1")?,
        "change1\n"
    );

    // branch 2
    assert_eq!(branches.b2.patches.len(), 1);
    assert_eq!(branches.b2.patches[0].message, "commit 3");

    // branch 3
    assert_eq!(branches.b3.patches.len(), 1);
    assert_eq!(branches.b3.patches[0].message, "commit 5");
    Ok(())
}

// Squash multiple commits where both are stacked branch heads
//
// - commit 5 (a-branch-3) ──┐
// - commit 4 (a-branch-2) ──┤
// - commit 3                │
// - commit 2             ◄──┘
// - commit 1 (a-branch-1)
//
// Result:
//            (a-branch-3)
// - commit 3 (a-branch-2)
// - commit 2+5+4
// - commit 1 (a-branch-1)
#[test]
fn squash_multiple_from_heads() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx()?;
    let test = test_ctx(&ctx)?;
    squash_commits(
        &ctx,
        test.stack.id,
        vec![test.commit_5, test.commit_4],
        test.commit_2,
    )?;
    let branches = list_branches(&ctx)?;

    // branch 1
    assert_eq!(branches.b1.patches.len(), 1);
    assert_eq!(branches.b1.patches[0].message, "commit 1");

    // branch 2
    assert_eq!(branches.b2.patches.len(), 2);
    assert_eq!(branches.b2.patches[0].message, "commit 3");
    assert_eq!(
        branches.b2.patches[1].message,
        "commit 2\ncommit 5\ncommit 4"
    );
    assert_eq!(
        blob_content(&*ctx.git2_repo.get()?, branches.b2.patches[1].id, "file5")?,
        "change5\n"
    );
    assert_eq!(
        blob_content(&*ctx.git2_repo.get()?, branches.b2.patches[1].id, "file4")?,
        "change4\n"
    );
    assert_eq!(
        blob_content(&*ctx.git2_repo.get()?, branches.b2.patches[1].id, "file2_3")?,
        "change2\n"
    );

    // branch 3
    assert_eq!(branches.b3.patches.len(), 0);
    Ok(())
}

// Squash multiple commits from above and below
//
// - commit 5 (a-branch-3) ──┐
// - commit 4 (a-branch-2)   │
// - commit 3             ◄──┤
// - commit 2                │
// - commit 1 (a-branch-1) ──┘
//
// Result:
//            (a-branch-3)
// - commit 4 (a-branch-2)
// - commit 3+5+1
// - commit 2
// - base     (a-branch-1)
#[test]
fn squash_multiple_above_and_below() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx()?;
    let test = test_ctx(&ctx)?;
    squash_commits(
        &ctx,
        test.stack.id,
        vec![test.commit_5, test.commit_1],
        test.commit_3,
    )?;
    let branches = list_branches(&ctx)?;

    // branch 1
    assert_eq!(branches.b1.patches.len(), 0);

    // branch 2
    assert_eq!(branches.b2.patches.len(), 3);
    assert_eq!(branches.b2.patches[0].message, "commit 4");
    assert_eq!(
        branches.b2.patches[1].message,
        "commit 3\ncommit 5\ncommit 1"
    );
    assert_eq!(
        blob_content(&*ctx.git2_repo.get()?, branches.b2.patches[1].id, "file2_3")?,
        "change3\n"
    );
    assert_eq!(
        blob_content(&*ctx.git2_repo.get()?, branches.b2.patches[1].id, "file5")?,
        "change5\n"
    );
    assert_eq!(
        blob_content(&*ctx.git2_repo.get()?, branches.b2.patches[1].id, "file1")?,
        "change1\n"
    );
    assert_eq!(branches.b2.patches[2].message, "commit 2");

    // branch 3
    assert_eq!(branches.b3.patches.len(), 0);
    Ok(())
}

fn command_ctx() -> Result<(Context, TempDir)> {
    gitbutler_testsupport::writable::fixture("squash.sh", "multiple-commits")
}

fn test_ctx(ctx: &Context) -> Result<TestContext> {
    let handle = VirtualBranchesHandle::new(ctx.project_data_dir());
    let stacks = handle.list_all_stacks()?;
    let stack = stacks.iter().find(|b| b.name == "my_stack").unwrap();
    let branches = stack.branches();
    let branch_1 = branches.iter().find(|b| b.name() == "my_stack").unwrap();
    let git2_repo = &*ctx.git2_repo.get()?;
    let project = &ctx.legacy_project;
    let commit_1 = branch_1.commits(git2_repo, project, stack)?.local_commits[0].clone();
    let branch_2 = branches.iter().find(|b| b.name() == "a-branch-2").unwrap();
    let commit_2 = branch_2.commits(git2_repo, project, stack)?.local_commits[0].clone();
    let commit_3 = branch_2.commits(git2_repo, project, stack)?.local_commits[1].clone();
    let commit_4 = branch_2.commits(git2_repo, project, stack)?.local_commits[2].clone();
    let branch_3 = branches.iter().find(|b| b.name() == "a-branch-3").unwrap();
    let commit_5 = branch_3.commits(git2_repo, project, stack)?.local_commits[0].clone();
    Ok(TestContext {
        stack: stack.clone(),
        branch_1: branch_1.clone(),
        branch_2: branch_2.clone(),
        branch_3: branch_3.clone(),
        commit_1: commit_1.id(),
        commit_2: commit_2.id(),
        commit_3: commit_3.id(),
        commit_4: commit_4.id(),
        commit_5: commit_5.id(),
    })
}

// The fixture:
// - commit 5 (a-branch-3)
// - commit 4 (a-branch-2)
// - commit 3
// - commit 2
// - commit 1 (a-branch-1)
#[expect(unused)]
struct TestContext {
    stack: gitbutler_stack::Stack,
    branch_1: StackBranch,
    branch_2: StackBranch,
    branch_3: StackBranch,
    commit_1: git2::Oid,
    commit_2: git2::Oid,
    commit_3: git2::Oid,
    commit_4: git2::Oid,
    commit_5: git2::Oid,
}

/// Stack branches, but from the list API
#[derive(Debug, Clone)]
struct TestBranchListing {
    b1: Branch,
    b2: Branch,
    b3: Branch,
}

#[derive(Debug, Clone)]
struct Branch {
    name: String,
    patches: Vec<Commit>,
}

/// Stack branches from the API
fn list_branches(ctx: &Context) -> Result<TestBranchListing> {
    let details = gitbutler_testsupport::stack_details(ctx);
    let (_, details) = details.first().unwrap();
    let branches: Vec<Branch> = details
        .branch_details
        .iter()
        .map(|d| Branch {
            name: d.name.clone().to_string(),
            patches: d.commits.clone(),
        })
        .collect_vec();
    fn find(branches: &[Branch], name: &str) -> Branch {
        branches.iter().find(|b| b.name == name).unwrap().clone()
    }
    Ok(TestBranchListing {
        b1: find(&branches, "my_stack"),
        b2: find(&branches, "a-branch-2"),
        b3: find(&branches, "a-branch-3"),
    })
}

fn blob_content(repo: &git2::Repository, commit_oid: gix::ObjectId, file: &str) -> Result<String> {
    let tree = repo.find_commit(commit_oid.to_git2())?.tree()?;
    let entry = tree.get_name(file).unwrap();
    let blob = repo.find_blob(entry.id())?;
    let blob_content: &str = blob.content().to_str()?;
    Ok(blob_content.to_string())
}
