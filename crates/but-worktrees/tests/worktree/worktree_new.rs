use crate::util::test_ctx;
use anyhow::Context as _;
use but_meta::VirtualBranchesTomlMetadata;
use but_workspace::legacy::{StacksFilter, stacks_v3};
use but_worktrees::new::worktree_new;

#[test]
fn can_create_worktree_from_feature_a() -> anyhow::Result<()> {
    let feature_a_name = gix::refs::FullName::try_from("refs/heads/feature-a")?;
    let mut test_ctx = test_ctx("stacked-and-parallel")?;

    let guard = test_ctx.ctx.exclusive_worktree_access();
    let repo = test_ctx.ctx.clone_repo_for_merging()?;
    let meta = VirtualBranchesTomlMetadata::from_path(
        test_ctx
            .ctx
            .legacy_project
            .gb_dir()
            .join("virtual_branches.toml"),
    )?;
    let stacks = stacks_v3(&repo, &meta, StacksFilter::InWorkspace, None)?;
    let feature_a = stacks
        .into_iter()
        .flat_map(|s| s.heads)
        .find(|h| h.name == b"feature-a")
        .context("Expect to find feature-a")?;

    let outcome = worktree_new(
        &mut test_ctx.ctx,
        guard.read_permission(),
        feature_a_name.as_ref(),
    )?;

    assert_eq!(
        outcome.created.base,
        Some(feature_a.tip),
        "The base should the the same as the tip of feature-a"
    );
    let worktree = repo.worktrees()?[0].clone();
    let worktree_repo = worktree.clone().into_repo()?;
    assert_eq!(
        worktree.base()?,
        outcome.created.path.canonicalize()?,
        "Worktree should be created where we say"
    );
    assert_eq!(
        Some(worktree_repo.head()?.id().unwrap().detach()),
        outcome.created.base,
        "Worktree should have base checked out"
    );

    Ok(())
}

#[test]
fn can_create_worktree_from_feature_b() -> anyhow::Result<()> {
    let feature_b_name = gix::refs::FullName::try_from("refs/heads/feature-b")?;
    let mut test_ctx = test_ctx("stacked-and-parallel")?;

    let guard = test_ctx.ctx.exclusive_worktree_access();
    let repo = test_ctx.ctx.clone_repo_for_merging()?;
    let meta = VirtualBranchesTomlMetadata::from_path(
        test_ctx
            .ctx
            .legacy_project
            .gb_dir()
            .join("virtual_branches.toml"),
    )?;
    let stacks = stacks_v3(&repo, &meta, StacksFilter::InWorkspace, None)?;
    let feature_b = stacks
        .into_iter()
        .flat_map(|s| s.heads)
        .find(|h| h.name == b"feature-b")
        .context("Expect to find feature-b")?;

    let outcome = worktree_new(
        &mut test_ctx.ctx,
        guard.read_permission(),
        feature_b_name.as_ref(),
    )?;

    assert_eq!(
        outcome.created.base,
        Some(feature_b.tip),
        "The base should the the same as the tip of feature-b"
    );
    let worktree = repo.worktrees()?[0].clone();
    let worktree_repo = worktree.clone().into_repo()?;
    assert_eq!(
        worktree.base()?,
        outcome.created.path.canonicalize()?,
        "Worktree should be created where we say"
    );
    assert_eq!(
        Some(worktree_repo.head()?.id().unwrap().detach()),
        outcome.created.base,
        "Worktree should have base checked out"
    );

    Ok(())
}

#[test]
fn can_create_worktree_from_feature_c() -> anyhow::Result<()> {
    let feature_c_name = gix::refs::FullName::try_from("refs/heads/feature-c")?;
    let mut test_ctx = test_ctx("stacked-and-parallel")?;

    let guard = test_ctx.ctx.exclusive_worktree_access();
    let repo = test_ctx.ctx.clone_repo_for_merging()?;
    let meta = VirtualBranchesTomlMetadata::from_path(
        test_ctx
            .ctx
            .legacy_project
            .gb_dir()
            .join("virtual_branches.toml"),
    )?;
    let stacks = stacks_v3(&repo, &meta, StacksFilter::InWorkspace, None)?;
    let feature_c = stacks
        .into_iter()
        .flat_map(|s| s.heads)
        .find(|h| h.name == b"feature-c")
        .context("Expect to find feature-c")?;

    let outcome = worktree_new(
        &mut test_ctx.ctx,
        guard.read_permission(),
        feature_c_name.as_ref(),
    )?;

    assert_eq!(
        outcome.created.base,
        Some(feature_c.tip),
        "The base should the the same as the tip of feature-c"
    );
    let worktree = repo.worktrees()?[0].clone();
    let worktree_repo = worktree.clone().into_repo()?;
    assert_eq!(
        worktree.base()?,
        outcome.created.path.canonicalize()?,
        "Worktree should be created where we say"
    );
    assert_eq!(
        Some(worktree_repo.head()?.id().unwrap().detach()),
        outcome.created.base,
        "Worktree should have base checked out"
    );

    Ok(())
}
