use but_testsupport::{CommandExt, git_at_dir, open_repo};

#[test]
fn log_target_first_parent_uses_persisted_target_outside_workspace() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let remote_dir = tmp.path().join("remote");
    std::fs::create_dir(&remote_dir)?;

    git_at_dir(&remote_dir)
        .args(["init", "-b", "main", "--object-format=sha1"])
        .run();
    std::fs::write(remote_dir.join("file"), "initial\n")?;
    git_at_dir(&remote_dir).args(["add", "file"]).run();
    git_at_dir(&remote_dir)
        .args(["commit", "-m", "initial"])
        .run();

    git_at_dir(&remote_dir)
        .args(["checkout", "-b", "feature"])
        .run();
    std::fs::write(remote_dir.join("feature"), "feature\n")?;
    git_at_dir(&remote_dir).args(["add", "feature"]).run();
    git_at_dir(&remote_dir)
        .args(["commit", "-m", "feature"])
        .run();
    git_at_dir(&remote_dir).args(["checkout", "main"]).run();

    let clone_dir = tmp.path().join("clone");
    git_at_dir(tmp.path())
        .args(["clone", remote_dir.to_str().expect("valid UTF-8 path")])
        .arg(&clone_dir)
        .run();
    git_at_dir(&clone_dir)
        .args(["checkout", "-b", "feature", "origin/feature"])
        .run();

    let ctx = but_ctx::Context::from_repo(open_repo(&clone_dir)?)?;
    let (main_tip, feature_tip) = {
        let repo = ctx.repo.get()?;
        (
            repo.rev_parse_single("refs/remotes/origin/main")?.detach(),
            repo.rev_parse_single("refs/remotes/origin/feature")?
                .detach(),
        )
    };
    assert_ne!(
        main_tip, feature_tip,
        "the checked-out branch upstream must differ from the configured GitButler target"
    );

    std::fs::create_dir_all(ctx.project_data_dir())?;
    std::fs::write(
        ctx.project_data_dir().join("virtual_branches.toml"),
        format!(
            r#"
[default_target]
branchName = "main"
remoteName = "origin"
remoteUrl = ""
sha = "{main_tip}"

[branch_targets]

[branches]
"#
        ),
    )?;

    let commits = but_workspace::legacy::log_target_first_parent(&ctx, None, 1)?;

    assert_eq!(
        commits.first().map(|commit| commit.id),
        Some(main_tip),
        "outside-workspace target history must use persisted GitButler target metadata, not the current branch upstream"
    );
    Ok(())
}
