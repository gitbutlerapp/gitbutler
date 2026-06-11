use crate::util::{test_ctx, worktree_new};

#[test]
fn can_create_worktree_from_feature_a() -> anyhow::Result<()> {
    can_create_worktree_from("refs/heads/feature-a")
}

#[test]
fn can_create_worktree_from_feature_b() -> anyhow::Result<()> {
    can_create_worktree_from("refs/heads/feature-b")
}

#[test]
fn can_create_worktree_from_feature_c() -> anyhow::Result<()> {
    can_create_worktree_from("refs/heads/feature-c")
}

fn can_create_worktree_from(refname: &str) -> anyhow::Result<()> {
    let branch_name = gix::refs::FullName::try_from(refname)?;
    let mut test_ctx = test_ctx("stacked-and-parallel")?;

    let guard = test_ctx.ctx.exclusive_worktree_access();
    let tip = test_ctx
        .ctx
        .repo
        .get()?
        .find_reference(branch_name.as_ref())?
        .id()
        .detach();

    let outcome = worktree_new(&test_ctx.ctx, guard.read_permission(), branch_name.as_ref())?;

    assert_eq!(
        outcome.created.base,
        Some(tip),
        "The base should be the same as the tip of {refname}"
    );
    let repo = test_ctx.ctx.repo.get()?;
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
