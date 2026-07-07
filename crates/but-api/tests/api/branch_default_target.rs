use crate::support::{repo_with_feature_branch, write_file};
use but_testsupport::{CommandExt, git_at_dir, open_repo};

#[test]
fn set_default_target_accepts_remote_tracking_ref_and_persists_metadata() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let target_ref = gix::refs::FullName::try_from("refs/remotes/origin/main")?;

    let project_meta =
        but_api::branch::set_default_target(&mut ctx, target_ref.as_ref(), Some("origin".into()))?;

    let stored_meta = ctx.project_meta()?;
    assert_eq!(project_meta, stored_meta);
    assert_eq!(stored_meta.target_ref.as_ref(), Some(&target_ref));
    assert!(stored_meta.target_commit_id.is_some());
    assert_eq!(stored_meta.push_remote.as_deref(), Some("origin"));

    Ok(())
}

#[test]
fn set_default_target_rejects_local_branch_refs() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let target_ref = gix::refs::FullName::try_from("refs/heads/feature")?;

    let err = but_api::branch::set_default_target(&mut ctx, target_ref.as_ref(), None)
        .expect_err("local branches are not valid default targets");
    assert_eq!(
        err.to_string(),
        "Default target must be a remote-tracking branch under refs/remotes, got 'refs/heads/feature'"
    );

    Ok(())
}

#[test]
fn set_default_target_uses_merge_base_not_target_tip() -> anyhow::Result<()> {
    let (_repo, tmp) = repo_with_feature_branch()?;
    git_at_dir(tmp.path()).args(["checkout", "feature"]).run();
    write_file(tmp.path(), "feature.txt", "feature\n")?;
    git_at_dir(tmp.path()).args(["add", "feature.txt"]).run();
    git_at_dir(tmp.path())
        .args(["commit", "-m", "feature"])
        .run();
    git_at_dir(tmp.path())
        .args(["update-ref", "refs/remotes/origin/feature", "HEAD"])
        .run();
    git_at_dir(tmp.path()).args(["checkout", "main"]).run();

    let repo = open_repo(tmp.path())?;
    let target_ref = gix::refs::FullName::try_from("refs/remotes/origin/feature")?;
    let target_tip = repo
        .find_reference(target_ref.as_ref())?
        .peel_to_id()?
        .detach();
    let current_head = repo.head_id()?.detach();
    let expected_merge_base = repo.merge_base(current_head, target_tip)?.detach();
    assert_ne!(expected_merge_base, target_tip);

    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let project_meta = but_api::branch::set_default_target(&mut ctx, target_ref.as_ref(), None)?;

    assert_eq!(project_meta.target_commit_id, Some(expected_merge_base));
    assert_eq!(project_meta.push_remote, None);

    Ok(())
}
