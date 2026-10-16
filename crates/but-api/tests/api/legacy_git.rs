use but_testsupport::{CommandExt, git_at_dir, open_repo};

#[test]
fn remote_branches_skip_one_component_remote_refs() -> anyhow::Result<()> {
    let (repo, tmp) = crate::support::repo_with_feature_branch()?;
    drop(repo);
    for name in [
        "refs/remotes/origin/HEAD",
        "refs/remotes/origin/feature",
        "refs/remotes/pr2536",
        "refs/remotes/upstream/main",
    ] {
        git_at_dir(tmp.path())
            .args(["update-ref", name, "HEAD"])
            .run();
    }
    let ctx = but_ctx::Context::from_repo_for_testing(open_repo(tmp.path())?)?;

    let names: Vec<_> = but_api::legacy::git::git_remote_branches(&ctx)?
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        names,
        [
            "refs/remotes/origin/feature",
            "refs/remotes/origin/main",
            "refs/remotes/upstream/main",
        ],
        "representable refs keep their order, while HEAD and the one-component ref are omitted"
    );
    Ok(())
}
