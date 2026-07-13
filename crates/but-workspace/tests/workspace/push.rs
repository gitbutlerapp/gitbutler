use crate::utils::{r, writable_scenario_slow};
use bstr::ByteSlice;
use but_core::ref_metadata::ProjectMeta;
use but_workspace::{
    RefInfo,
    ref_info::Options,
    ui::PushStatus::{CompletelyUnpushed, NothingToPush, UnpushedCommitsRequiringForce},
};

static ASKPASS: std::sync::Once = std::sync::Once::new();

fn fixture(
    name: &str,
) -> anyhow::Result<(
    but_testsupport::gix_testtools::tempfile::TempDir,
    gix::Repository,
    but_meta::VirtualBranchesTomlMetadata,
)> {
    ASKPASS.call_once(but_askpass::disable);
    let (repo, tmp) = writable_scenario_slow(name);
    let remote = tmp.path().join("remote.git");
    let status = std::process::Command::new("git")
        .current_dir(repo.workdir().expect("fixtures have workdirs"))
        .args(["remote", "set-url", "origin"])
        .arg(remote)
        .status()?;
    assert!(
        status.success(),
        "fixture remote URL should be normalized to an absolute path"
    );
    let meta = but_meta::VirtualBranchesTomlMetadata::from_path(
        repo.path().join("virtual-branches.toml"),
    )?;
    Ok((tmp, repo, meta))
}

fn project_meta(repo: &gix::Repository) -> anyhow::Result<ProjectMeta> {
    Ok(ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        target_commit_id: Some(repo.rev_parse_single("main")?.detach()),
        push_remote: None,
    })
}

fn head_info(
    repo: &gix::Repository,
    meta: &but_meta::VirtualBranchesTomlMetadata,
) -> anyhow::Result<(RefInfo, but_graph::Workspace)> {
    but_workspace::head_info_and_workspace(
        repo,
        meta,
        Options {
            project_meta: project_meta(repo)?,
            expensive_commit_info: true,
            ..Default::default()
        },
    )
}

fn push(
    repo: &gix::Repository,
    meta: &but_meta::VirtualBranchesTomlMetadata,
    branch: &gix::refs::FullNameRef,
    with_force: bool,
    skip_force_push_protection: bool,
    force_push_protection: bool,
) -> anyhow::Result<gitbutler_git::PushResult> {
    let (info, workspace) = head_info(repo, meta)?;
    let mut db = but_db::DbHandle::new_at_path(":memory:")?;
    but_workspace::legacy::workspace_branch_and_ancestors_push(
        repo,
        &workspace,
        &project_meta(repo)?,
        &info,
        &mut db,
        false,
        with_force,
        skip_force_push_protection,
        force_push_protection,
        branch,
        false,
        false,
        Vec::new(),
    )
}

fn apply_remote_tracking_updates(
    repo: &gix::Repository,
    result: &gitbutler_git::PushResult,
) -> anyhow::Result<()> {
    for ((_branch, remote_refname), (_, _, after_sha)) in result
        .branch_to_remote
        .iter()
        .zip(result.branch_sha_updates.iter())
    {
        let status = std::process::Command::new("git")
            .current_dir(repo.workdir().expect("fixtures have workdirs"))
            .args(["update-ref", remote_refname.as_bstr().to_str()?, after_sha])
            .status()?;
        assert!(
            status.success(),
            "git update-ref should update pushed remote-tracking refs"
        );
    }
    Ok(())
}

fn status(info: &RefInfo, branch: &str) -> but_workspace::ui::PushStatus {
    info.stacks
        .iter()
        .flat_map(|stack| &stack.segments)
        .find(|segment| {
            segment
                .ref_info
                .as_ref()
                .is_some_and(|ref_info| ref_info.ref_name.shorten() == branch.as_bytes())
        })
        .unwrap_or_else(|| panic!("fixture should contain branch `{branch}`"))
        .push_status
}

#[test]
fn pushing_bottom_of_stack_reports_only_bottom_as_pushed() -> anyhow::Result<()> {
    let (_tmp, repo, meta) = fixture("push")?;

    let result = push(&repo, &meta, r("refs/heads/bottom"), false, false, false)?;
    assert_eq!(
        result
            .branch_to_remote
            .iter()
            .map(|(branch, _)| branch.as_str())
            .collect::<Vec<_>>(),
        ["bottom"],
        "pushing the bottom branch should not push the top branch"
    );

    apply_remote_tracking_updates(&repo, &result)?;
    let (info, _) = head_info(&repo, &meta)?;
    assert_eq!(status(&info, "bottom"), NothingToPush);
    assert_eq!(status(&info, "top"), CompletelyUnpushed);

    Ok(())
}

#[test]
fn pushing_top_of_stack_reports_top_as_pushed_after_bottom_is_current() -> anyhow::Result<()> {
    let (_tmp, repo, meta) = fixture("push")?;

    let bottom_result = push(&repo, &meta, r("refs/heads/bottom"), false, false, false)?;
    apply_remote_tracking_updates(&repo, &bottom_result)?;

    let result = push(&repo, &meta, r("refs/heads/top"), false, false, false)?;
    assert_eq!(
        result
            .branch_to_remote
            .iter()
            .map(|(branch, _)| branch.as_str())
            .collect::<Vec<_>>(),
        ["top"],
        "once the bottom branch is current, pushing the top branch should report only the top"
    );

    apply_remote_tracking_updates(&repo, &result)?;
    let (info, _) = head_info(&repo, &meta)?;
    assert_eq!(status(&info, "bottom"), NothingToPush);
    assert_eq!(status(&info, "top"), NothingToPush);

    Ok(())
}

#[test]
fn force_push_protection_is_observed_when_pushing_bottom_branch() -> anyhow::Result<()> {
    let (_tmp, repo, meta) = fixture("push-requiring-force")?;
    let (info, _) = head_info(&repo, &meta)?;
    assert_eq!(status(&info, "bottom"), UnpushedCommitsRequiringForce);

    let err = push(&repo, &meta, r("refs/heads/bottom"), true, false, true)
        .expect_err("force-with-lease should reject the stale remote branch");
    let err = format!("{err:#}");
    assert!(
        err.contains("force push was blocked")
            && err.contains("--force-with-lease")
            && err.contains("--force-if-includes"),
        "error should come from force push protection: {err:#}"
    );

    let result = push(&repo, &meta, r("refs/heads/bottom"), true, true, true)?;
    assert_eq!(
        result
            .branch_to_remote
            .iter()
            .map(|(branch, _)| branch.as_str())
            .collect::<Vec<_>>(),
        ["bottom"],
        "skipping force push protection should allow pushing the rewritten bottom branch"
    );

    Ok(())
}

#[test]
fn force_push_protection_is_observed_when_pushing_top_branch() -> anyhow::Result<()> {
    let (_tmp, repo, meta) = fixture("push-requiring-force")?;

    let err = push(&repo, &meta, r("refs/heads/top"), true, false, true)
        .expect_err("pushing the top branch should observe bottom branch force protection first");
    let err = format!("{err:#}");
    assert!(
        err.contains("force push was blocked")
            && err.contains("--force-with-lease")
            && err.contains("--force-if-includes"),
        "error should come from force push protection: {err:#}"
    );

    let result = push(&repo, &meta, r("refs/heads/top"), true, true, true)?;
    assert_eq!(
        result
            .branch_to_remote
            .iter()
            .map(|(branch, _)| branch.as_str())
            .collect::<Vec<_>>(),
        ["bottom", "top"],
        "skipping force push protection should allow pushing the bottom ancestor and top branch"
    );

    Ok(())
}
