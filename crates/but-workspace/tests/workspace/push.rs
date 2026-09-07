use std::fmt::Write as _;

use crate::utils::{r, writable_scenario_slow};
use bstr::ByteSlice;
use but_core::ref_metadata::ProjectMeta;
use but_testsupport::{CommandExt, git, visualize_commit_graph_all};
use but_workspace::{RefInfo, ref_info::Options};
use gitbutler_git::PushResult;
use snapbox::str;

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
    // The fixture's relative remote URL is normalized to an absolute path.
    git(&repo)
        .args(["remote", "set-url", "origin"])
        .arg(tmp.path().join("remote.git"))
        .run();
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
        &mut but_testsupport::project_db(repo)?,
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
) -> anyhow::Result<PushResult> {
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

fn apply_remote_tracking_updates(repo: &gix::Repository, result: &PushResult) {
    for ((_branch, remote_refname, _remote_branch_name), (_, _, after_sha)) in result
        .branch_to_remote
        .iter()
        .zip(result.branch_sha_updates.iter())
    {
        git(repo)
            .arg("update-ref")
            .arg(remote_refname.as_bstr().to_os_str_lossy())
            .arg(after_sha)
            .run();
    }
}

/// The remote the push defaulted to, then one line per pushed branch with the remote
/// tracking ref it updated and its name on that remote.
fn render_push_result(result: &PushResult) -> String {
    let mut out = format!("remote: {}\n", result.remote);
    for (branch, remote_refname, remote_branch_name) in &result.branch_to_remote {
        writeln!(out, "{branch} -> {remote_refname} ({remote_branch_name})")
            .expect("in-memory write succeeds");
    }
    out
}

/// One line per branch with its push status as projected by `info`.
fn render_statuses(info: &RefInfo, branches: &[&str]) -> String {
    let mut out = String::new();
    for branch in branches {
        let push_status = info
            .stacks
            .iter()
            .flat_map(|stack| &stack.segments)
            .find(|segment| {
                segment
                    .ref_info
                    .as_ref()
                    .is_some_and(|ref_info| ref_info.ref_name.shorten() == branch.as_bytes())
            })
            .unwrap_or_else(|| panic!("fixture should contain branch `{branch}`"))
            .push_status;
        writeln!(out, "{branch}: {push_status:?}").expect("in-memory write succeeds");
    }
    out
}

/// The branches that pushing `branch` synchronizes, from the branch itself down to its
/// lowest ancestor.
fn logical_scope(info: &RefInfo, branch: &str) -> String {
    let branch = gix::refs::Category::LocalBranch
        .to_full_name(branch)
        .expect("valid fixture branch name");
    but_workspace::legacy::push::branch_and_ancestor_segments(info, branch.as_ref())
        .values()
        .filter_map(|segment| {
            segment
                .ref_info
                .as_ref()
                .map(|ref_info| ref_info.ref_name.shorten().to_string())
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[test]
fn logical_push_scope_is_selected_branch_plus_ancestors() -> anyhow::Result<()> {
    let (_tmp, repo, meta) = fixture("push")?;
    let (info, _) = head_info(&repo, &meta)?;

    snapbox::assert_data_eq!(logical_scope(&info, "bottom"), str!["bottom"]);
    snapbox::assert_data_eq!(logical_scope(&info, "middle"), str!["middle, bottom"]);
    snapbox::assert_data_eq!(logical_scope(&info, "top"), str!["top, middle, bottom"]);
    // An unrelated stack must not enter the selected scope.
    snapbox::assert_data_eq!(logical_scope(&info, "solo"), str!["solo"]);
    Ok(())
}

#[test]
fn pushed_branch_reports_its_name_on_the_remote_it_landed_on() -> anyhow::Result<()> {
    let (tmp, repo, meta) = fixture("push")?;
    // Track `bottom` on a second remote so its own remote differs from the push default,
    // which is derived from the target ref and stays `origin`.
    let fork = tmp.path().join("remote.git");
    // The tracking ref has to exist for the branch to be seen as tracking `fork`, and it
    // points at the base so `bottom` still has commits left to push.
    let base = repo.rev_parse_single("main")?.to_string();
    for args in [
        vec!["remote", "add", "fork", fork.to_str().expect("utf8 path")],
        vec!["config", "branch.bottom.remote", "fork"],
        vec!["config", "branch.bottom.merge", "refs/heads/bottom"],
        vec!["update-ref", "refs/remotes/fork/bottom", base.as_str()],
    ] {
        git(&repo).args(args).run();
    }
    // Reopen so the configuration written above is visible.
    let repo = gix::open(repo.path())?;

    let result = push(&repo, &meta, r("refs/heads/bottom"), false, false, false)?;

    // The push default still comes from the target ref, while the branch name on the remote
    // is stripped of the remote the branch actually landed on.
    snapbox::assert_data_eq!(
        render_push_result(&result),
        str![[r#"
remote: origin
bottom -> refs/remotes/fork/bottom (bottom)

"#]]
    );
    Ok(())
}

#[test]
fn pushing_bottom_of_stack_reports_only_bottom_as_pushed() -> anyhow::Result<()> {
    let (_tmp, repo, meta) = fixture("push")?;

    let result = push(&repo, &meta, r("refs/heads/bottom"), false, false, false)?;
    // Pushing the bottom branch does not push the top branch.
    snapbox::assert_data_eq!(
        render_push_result(&result),
        str![[r#"
remote: origin
bottom -> refs/remotes/origin/bottom (bottom)

"#]]
    );

    apply_remote_tracking_updates(&repo, &result);
    let (info, _) = head_info(&repo, &meta)?;
    snapbox::assert_data_eq!(
        render_statuses(&info, &["bottom", "top"]),
        str![[r#"
bottom: NothingToPush
top: CompletelyUnpushed

"#]]
    );
    Ok(())
}

#[test]
fn pushing_top_of_stack_reports_top_as_pushed_after_bottom_is_current() -> anyhow::Result<()> {
    let (_tmp, repo, meta) = fixture("push")?;

    let bottom_result = push(&repo, &meta, r("refs/heads/bottom"), false, false, false)?;
    apply_remote_tracking_updates(&repo, &bottom_result);
    let middle_result = push(&repo, &meta, r("refs/heads/middle"), false, false, false)?;
    apply_remote_tracking_updates(&repo, &middle_result);

    let result = push(&repo, &meta, r("refs/heads/top"), false, false, false)?;
    // Once the ancestors are current, pushing the top branch reports only the top.
    snapbox::assert_data_eq!(
        render_push_result(&result),
        str![[r#"
remote: origin
top -> refs/remotes/origin/top (top)

"#]]
    );

    apply_remote_tracking_updates(&repo, &result);
    let (info, _) = head_info(&repo, &meta)?;
    // Already-current ancestors remain in the logical synchronization scope.
    snapbox::assert_data_eq!(logical_scope(&info, "top"), str!["top, middle, bottom"]);
    snapbox::assert_data_eq!(
        render_statuses(&info, &["bottom", "middle", "top"]),
        str![[r#"
bottom: NothingToPush
middle: NothingToPush
top: NothingToPush

"#]]
    );
    Ok(())
}

#[test]
fn force_push_protection_is_observed_when_pushing_bottom_branch() -> anyhow::Result<()> {
    let (_tmp, repo, meta) = fixture("push-requiring-force")?;
    let (info, _) = head_info(&repo, &meta)?;
    snapbox::assert_data_eq!(
        render_statuses(&info, &["bottom"]),
        str![[r#"
bottom: UnpushedCommitsRequiringForce

"#]]
    );

    let err = push(&repo, &meta, r("refs/heads/bottom"), true, false, true)
        .expect_err("force-with-lease should reject the stale remote branch");
    snapbox::assert_data_eq!(
        format!("{err:#}"),
        str![[r#"
GitForcePushProtection: The force push was blocked because the remote branch contains commits that would be overwritten.

git command exited with non-zero exit code 1:

ARGS:
["push", "--quiet", "--no-verify", "origin", "48f3b49bdfba19331a42e373aaefbd772b17857f:refs/heads/bottom", "--force-with-lease", "--force-if-includes"]

STDOUT:


STDERR:
To [..]/remote.git
 ! [rejected]        48f3b49bdfba19331a42e373aaefbd772b17857f -> bottom (remote ref updated since checkout)
error: failed to push some refs to '[..]/remote.git'
hint: Updates were rejected because the tip of the remote-tracking branch has
hint: been updated since the last checkout. If you want to integrate the
hint: remote changes, use 'git pull' before pushing again.
hint: See the 'Note about fast-forwards' in 'git push --help' for details.
"#]]
    );

    // Skipping force push protection allows pushing the rewritten bottom branch.
    let result = push(&repo, &meta, r("refs/heads/bottom"), true, true, true)?;
    snapbox::assert_data_eq!(
        render_push_result(&result),
        str![[r#"
remote: origin
bottom -> refs/remotes/origin/bottom (bottom)

"#]]
    );
    Ok(())
}

#[test]
fn force_push_protection_is_observed_when_pushing_top_branch() -> anyhow::Result<()> {
    let (_tmp, repo, meta) = fixture("push-requiring-force")?;

    // Pushing the top branch observes the bottom branch's force protection first.
    let err = push(&repo, &meta, r("refs/heads/top"), true, false, true)
        .expect_err("pushing the top branch should observe bottom branch force protection first");
    snapbox::assert_data_eq!(
        format!("{err:#}"),
        str![[r#"
GitForcePushProtection: The force push was blocked because the remote branch contains commits that would be overwritten.

git command exited with non-zero exit code 1:

ARGS:
["push", "--quiet", "--no-verify", "origin", "48f3b49bdfba19331a42e373aaefbd772b17857f:refs/heads/bottom", "--force-with-lease", "--force-if-includes"]

STDOUT:


STDERR:
To [..]/remote.git
 ! [rejected]        48f3b49bdfba19331a42e373aaefbd772b17857f -> bottom (remote ref updated since checkout)
error: failed to push some refs to '[..]/remote.git'
hint: Updates were rejected because the tip of the remote-tracking branch has
hint: been updated since the last checkout. If you want to integrate the
hint: remote changes, use 'git pull' before pushing again.
hint: See the 'Note about fast-forwards' in 'git push --help' for details.
"#]]
    );

    // Skipping force push protection allows pushing the bottom ancestor and the top branch.
    let result = push(&repo, &meta, r("refs/heads/top"), true, true, true)?;
    snapbox::assert_data_eq!(
        render_push_result(&result),
        str![[r#"
remote: origin
bottom -> refs/remotes/origin/bottom (bottom)
top -> refs/remotes/origin/top (top)

"#]]
    );
    Ok(())
}

#[test]
fn pushing_with_an_ordinary_branch_checked_out_pushes_it_and_its_ancestors() -> anyhow::Result<()> {
    let (_tmp, repo, meta) = fixture("push-single-branch")?;
    // The fixture has no workspace branch.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        str![[r#"
* 18af0f8 (HEAD -> top) top
* 9cfa5ba (bottom) bottom
* 85efbe4 (origin/main, main) M

"#]]
    );
    let (info, _) = head_info(&repo, &meta)?;
    // The checked-out branch and its ancestor form the push scope, and nothing was pushed yet.
    snapbox::assert_data_eq!(logical_scope(&info, "top"), str!["top, bottom"]);
    snapbox::assert_data_eq!(
        render_statuses(&info, &["bottom", "top"]),
        str![[r#"
bottom: CompletelyUnpushed
top: CompletelyUnpushed

"#]]
    );

    let result = push(&repo, &meta, r("refs/heads/top"), false, false, false)?;
    // The checked-out branch and its unpushed ancestor are pushed.
    snapbox::assert_data_eq!(
        render_push_result(&result),
        str![[r#"
remote: origin
bottom -> refs/remotes/origin/bottom (bottom)
top -> refs/remotes/origin/top (top)

"#]]
    );

    apply_remote_tracking_updates(&repo, &result);
    let (info, _) = head_info(&repo, &meta)?;
    snapbox::assert_data_eq!(
        render_statuses(&info, &["bottom", "top"]),
        str![[r#"
bottom: NothingToPush
top: NothingToPush

"#]]
    );
    // Pushing leaves the checkout untouched.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        str![[r#"
* 18af0f8 (HEAD -> top, origin/top) top
* 9cfa5ba (origin/bottom, bottom) bottom
* 85efbe4 (origin/main, main) M

"#]]
    );
    Ok(())
}
