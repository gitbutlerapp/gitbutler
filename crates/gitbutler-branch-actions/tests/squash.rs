use anyhow::Result;
use bstr::ByteSlice;
use gitbutler_branch_actions::{internal::PatchSeries, list_virtual_branches, squash_commits};
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use gitbutler_stack::{
    stack_context::{CommandContextExt, StackContext},
    StackBranch, VirtualBranchesHandle,
};
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
    let stack_ctx = ctx.to_stack_context()?;
    let test = test_ctx(&ctx, &stack_ctx)?;
    squash_commits(
        ctx.project(),
        test.stack.id,
        vec![test.commit_3.id()],
        test.commit_2.id(),
    )?;

    let branches = list_branches(ctx.project())?;
    // branch 1
    assert_eq!(branches.b1.patches.len(), 1);
    assert_eq!(branches.b1.patches[0].description, "commit 1");

    // branch 2
    assert_eq!(branches.b2.patches.len(), 2);
    assert_eq!(branches.b2.patches[0].description, "commit 4");
    assert_eq!(branches.b2.patches[1].description, "commit 2\ncommit 3");
    assert_eq!(
        blob_content(ctx.repo(), branches.b2.patches[1].id, "file2_3")?,
        "change3\n"
    );

    // branch 3
    assert_eq!(branches.b3.patches.len(), 1);
    assert_eq!(branches.b3.patches[0].description, "commit 5");
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
    let stack_ctx = ctx.to_stack_context()?;
    let test = test_ctx(&ctx, &stack_ctx)?;
    squash_commits(
        ctx.project(),
        test.stack.id,
        vec![test.commit_4.id()],
        test.commit_2.id(),
    )?;

    let branches = list_branches(ctx.project())?;
    // branch 1
    assert_eq!(branches.b1.patches.len(), 1);
    assert_eq!(branches.b1.patches[0].description, "commit 1");

    // branch 2
    assert_eq!(branches.b2.patches.len(), 2);
    assert_eq!(branches.b2.patches[0].description, "commit 3");
    assert_eq!(branches.b2.patches[1].description, "commit 2\ncommit 4");
    assert_eq!(
        blob_content(ctx.repo(), branches.b2.patches[1].id, "file2_3")?,
        "change2\n"
    );
    assert_eq!(
        blob_content(ctx.repo(), branches.b2.patches[1].id, "file4")?,
        "change4\n"
    );

    // branch 3
    assert_eq!(branches.b3.patches.len(), 1);
    assert_eq!(branches.b3.patches[0].description, "commit 5");
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
    let stack_ctx = ctx.to_stack_context()?;
    let test = test_ctx(&ctx, &stack_ctx)?;
    squash_commits(
        ctx.project(),
        test.stack.id,
        vec![test.commit_1.id()],
        test.commit_3.id(),
    )?;

    let branches = list_branches(ctx.project())?;
    // branch 1
    assert_eq!(branches.b1.patches.len(), 0);

    // branch 2
    assert_eq!(branches.b2.patches.len(), 3);
    assert_eq!(branches.b2.patches[0].description, "commit 4");
    assert_eq!(branches.b2.patches[1].description, "commit 3\ncommit 1");
    assert_eq!(
        blob_content(ctx.repo(), branches.b2.patches[1].id, "file2_3")?,
        "change3\n"
    );
    assert_eq!(
        blob_content(ctx.repo(), branches.b2.patches[1].id, "file1")?,
        "change1\n"
    );
    assert_eq!(branches.b2.patches[2].description, "commit 2");

    // branch 3
    assert_eq!(branches.b3.patches.len(), 1);
    assert_eq!(branches.b3.patches[0].description, "commit 5");
    Ok(())
}

// Squash up commit into another with the result being a conflict returns an error
//
// - commit 5 (a-branch-3)
// - commit 4 (a-branch-2)
// - commit 3             ◄──┐
// - commit 2              ──┘
// - commit 1 (a-branch-1)
//
// Commits 3 and 2 update the same file and line number
//
// NB: We may want to change this behavior in the future
#[test]
fn squash_producting_conflict_errors_out() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx()?;
    let stack_ctx = ctx.to_stack_context()?;
    let test = test_ctx(&ctx, &stack_ctx)?;
    let result = squash_commits(
        ctx.project(),
        test.stack.id,
        vec![test.commit_2.id()],
        test.commit_3.id(),
    );
    assert_eq!(
        result.unwrap_err().to_string(),
        format!("cannot squash into conflicted destination commit",)
    );

    // After a failed squash, the stack should be unchanged (i.e. the reordering that takes place is reversed)
    let branches = list_branches(ctx.project())?;
    // branch 3
    assert_eq!(branches.b3.patches[0].description, "commit 5");
    // branch 2
    assert_eq!(branches.b2.patches.len(), 3);
    assert_eq!(branches.b2.patches[0].description, "commit 4");
    assert_eq!(branches.b2.patches[1].description, "commit 3");
    assert_eq!(branches.b2.patches[2].description, "commit 2");
    // branch 1
    assert_eq!(branches.b1.patches.len(), 1);
    assert_eq!(branches.b1.patches[0].description, "commit 1");
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
    let stack_ctx = ctx.to_stack_context()?;
    let test = test_ctx(&ctx, &stack_ctx)?;
    squash_commits(
        ctx.project(),
        test.stack.id,
        vec![test.commit_3.id()],
        test.commit_2.id(),
    )?;
    let branches = list_branches(ctx.project())?;

    // branch 1
    assert_eq!(branches.b1.patches.len(), 1);
    assert_eq!(branches.b1.patches[0].description, "commit 1");

    // branch 2
    assert_eq!(branches.b2.patches.len(), 2);
    assert_eq!(branches.b2.patches[0].description, "commit 4");
    assert_eq!(branches.b2.patches[1].description, "commit 2\ncommit 3");
    assert_eq!(
        blob_content(ctx.repo(), branches.b2.patches[1].id, "file2_3")?,
        "change3\n"
    );

    // branch 3
    assert_eq!(branches.b3.patches.len(), 1);
    assert_eq!(branches.b3.patches[0].description, "commit 5");
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
    let stack_ctx = ctx.to_stack_context()?;
    let test = test_ctx(&ctx, &stack_ctx)?;
    squash_commits(
        ctx.project(),
        test.stack.id,
        vec![test.commit_4.id()],
        test.commit_1.id(),
    )?;
    let branches = list_branches(ctx.project())?;

    // branch 1
    assert_eq!(branches.b1.patches.len(), 1);
    assert_eq!(branches.b1.patches[0].description, "commit 1\ncommit 4");
    assert_eq!(
        blob_content(ctx.repo(), branches.b1.patches[0].id, "file4")?,
        "change4\n"
    );
    assert_eq!(
        blob_content(ctx.repo(), branches.b1.patches[0].id, "file1")?,
        "change1\n"
    );

    // branch 2
    assert_eq!(branches.b2.patches.len(), 2);
    assert_eq!(branches.b2.patches[0].description, "commit 3");
    assert_eq!(branches.b2.patches[1].description, "commit 2");

    // branch 3
    assert_eq!(branches.b3.patches.len(), 1);
    assert_eq!(branches.b3.patches[0].description, "commit 5");
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
    let stack_ctx = ctx.to_stack_context()?;
    let test = test_ctx(&ctx, &stack_ctx)?;
    squash_commits(
        ctx.project(),
        test.stack.id,
        vec![test.commit_4.id(), test.commit_2.id()],
        test.commit_1.id(),
    )?;
    let branches = list_branches(ctx.project())?;

    // branch 1
    assert_eq!(branches.b1.patches.len(), 1);
    assert_eq!(
        branches.b1.patches[0].description,
        "commit 1\ncommit 4\ncommit 2"
    );
    assert_eq!(
        blob_content(ctx.repo(), branches.b1.patches[0].id, "file4")?,
        "change4\n"
    );
    assert_eq!(
        blob_content(ctx.repo(), branches.b1.patches[0].id, "file2_3")?,
        "change2\n"
    );
    assert_eq!(
        blob_content(ctx.repo(), branches.b1.patches[0].id, "file1")?,
        "change1\n"
    );

    // branch 2
    assert_eq!(branches.b2.patches.len(), 1);
    assert_eq!(branches.b2.patches[0].description, "commit 3");

    // branch 3
    assert_eq!(branches.b3.patches.len(), 1);
    assert_eq!(branches.b3.patches[0].description, "commit 5");
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
    let stack_ctx = ctx.to_stack_context()?;
    let test = test_ctx(&ctx, &stack_ctx)?;
    squash_commits(
        ctx.project(),
        test.stack.id,
        vec![test.commit_5.id(), test.commit_4.id()],
        test.commit_2.id(),
    )?;
    let branches = list_branches(ctx.project())?;

    // branch 1
    assert_eq!(branches.b1.patches.len(), 1);
    assert_eq!(branches.b1.patches[0].description, "commit 1");

    // branch 2
    assert_eq!(branches.b2.patches.len(), 2);
    assert_eq!(branches.b2.patches[0].description, "commit 3");
    assert_eq!(
        branches.b2.patches[1].description,
        "commit 2\ncommit 5\ncommit 4"
    );
    assert_eq!(
        blob_content(ctx.repo(), branches.b2.patches[1].id, "file5")?,
        "change5\n"
    );
    assert_eq!(
        blob_content(ctx.repo(), branches.b2.patches[1].id, "file4")?,
        "change4\n"
    );
    assert_eq!(
        blob_content(ctx.repo(), branches.b2.patches[1].id, "file2_3")?,
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
    let stack_ctx = ctx.to_stack_context()?;
    let test = test_ctx(&ctx, &stack_ctx)?;
    squash_commits(
        ctx.project(),
        test.stack.id,
        vec![test.commit_5.id(), test.commit_1.id()],
        test.commit_3.id(),
    )?;
    let branches = list_branches(ctx.project())?;

    // branch 1
    assert_eq!(branches.b1.patches.len(), 0);

    // branch 2
    assert_eq!(branches.b2.patches.len(), 3);
    assert_eq!(branches.b2.patches[0].description, "commit 4");
    assert_eq!(
        branches.b2.patches[1].description,
        "commit 3\ncommit 5\ncommit 1"
    );
    assert_eq!(
        blob_content(ctx.repo(), branches.b2.patches[1].id, "file2_3")?,
        "change3\n"
    );
    assert_eq!(
        blob_content(ctx.repo(), branches.b2.patches[1].id, "file5")?,
        "change5\n"
    );
    assert_eq!(
        blob_content(ctx.repo(), branches.b2.patches[1].id, "file1")?,
        "change1\n"
    );
    assert_eq!(branches.b2.patches[2].description, "commit 2");

    // branch 3
    assert_eq!(branches.b3.patches.len(), 0);
    Ok(())
}

fn command_ctx() -> Result<(CommandContext, TempDir)> {
    gitbutler_testsupport::writable::fixture("squash.sh", "multiple-commits")
}

fn test_ctx<'a>(ctx: &'a CommandContext, stack_ctx: &'a StackContext) -> Result<TestContext<'a>> {
    let handle = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let stacks = handle.list_all_stacks()?;
    let stack = stacks.iter().find(|b| b.name == "my_stack").unwrap();
    let branches = stack.branches();
    let branch_1 = branches.iter().find(|b| b.name == "a-branch-1").unwrap();
    let commit_1 = branch_1.commits(stack_ctx, stack)?.local_commits[0].clone();
    let branch_2 = branches.iter().find(|b| b.name == "a-branch-2").unwrap();
    let commit_2 = branch_2.commits(stack_ctx, stack)?.local_commits[0].clone();
    let commit_3 = branch_2.commits(stack_ctx, stack)?.local_commits[1].clone();
    let commit_4 = branch_2.commits(stack_ctx, stack)?.local_commits[2].clone();
    let branch_3 = branches.iter().find(|b| b.name == "a-branch-3").unwrap();
    let commit_5 = branch_3.commits(stack_ctx, stack)?.local_commits[0].clone();
    Ok(TestContext {
        stack: stack.clone(),
        branch_1: branch_1.clone(),
        branch_2: branch_2.clone(),
        branch_3: branch_3.clone(),
        commit_1,
        commit_2,
        commit_3,
        commit_4,
        commit_5,
    })
}

// The fixture:
// - commit 5 (a-branch-3)
// - commit 4 (a-branch-2)
// - commit 3
// - commit 2
// - commit 1 (a-branch-1)
#[allow(unused)]
struct TestContext<'a> {
    stack: gitbutler_stack::Stack,
    branch_1: StackBranch,
    branch_2: StackBranch,
    branch_3: StackBranch,
    commit_1: git2::Commit<'a>,
    commit_2: git2::Commit<'a>,
    commit_3: git2::Commit<'a>,
    commit_4: git2::Commit<'a>,
    commit_5: git2::Commit<'a>,
}

/// Stack branches, but from the list API
#[derive(Debug, PartialEq, Clone)]
struct TestBranchListing {
    b1: PatchSeries,
    b2: PatchSeries,
    b3: PatchSeries,
}

/// Stack branches from the API
fn list_branches(project: &Project) -> Result<TestBranchListing> {
    let branches = list_virtual_branches(project)?
        .branches
        .first()
        .unwrap()
        .series
        .iter()
        .map(|s| s.clone().unwrap())
        .collect_vec();
    fn find(branches: &[PatchSeries], name: &str) -> PatchSeries {
        branches.iter().find(|b| b.name == name).unwrap().clone()
    }
    Ok(TestBranchListing {
        b1: find(&branches, "a-branch-1"),
        b2: find(&branches, "a-branch-2"),
        b3: find(&branches, "a-branch-3"),
    })
}

fn blob_content(repo: &git2::Repository, commit_oid: git2::Oid, file: &str) -> Result<String> {
    let tree = repo.find_commit(commit_oid)?.tree()?;
    let entry = tree.get_name(file).unwrap();
    let blob = repo.find_blob(entry.id())?;
    let blob_content: &str = blob.content().to_str()?;
    Ok(blob_content.to_string())
}
