use but_api::{
    commit::json::RelativeTo,
    workspace::json::{BottomUpdate, BottomUpdateKind},
};
use but_testsupport::{CommandExt, git_at_dir, open_repo};

use crate::support::{repo_with_feature_branch, write_file};

#[test]
#[cfg(unix)]
fn dry_run_does_not_sign_the_commits_it_rebases() -> anyhow::Result<()> {
    let (repo, tmp) = repo_with_feature_branch()?;
    let root = tmp.path();
    git_at_dir(root).args(["switch", "feature"]).run();
    write_file(root, "feature.txt", "feature\n")?;
    git_at_dir(root).args(["add", "feature.txt"]).run();
    git_at_dir(root).args(["commit", "-m", "feature"]).run();
    let feature_commit = repo.find_reference("refs/heads/feature")?.peel_to_id()?;
    let feature_commit = feature_commit.detach();
    git_at_dir(root)
        .args(["switch", "-c", "gitbutler/workspace"])
        .run();
    git_at_dir(root)
        .args([
            "commit",
            "--allow-empty",
            "-m",
            "GitButler Workspace Commit",
        ])
        .run();
    drop(repo);

    // A signer that records each call and returns a signature gix accepts.
    let calls = root.join("signer-calls");
    let signer = root.join("signer.sh");
    std::fs::write(
        &signer,
        format!(
            "#!/bin/sh\ncat >/dev/null\necho called >>'{}'\necho '[GNUPG:] SIG_CREATED ' >&2\necho fake-signature\n",
            calls.display()
        ),
    )?;
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&signer, std::fs::Permissions::from_mode(0o755))?;
    }
    for (key, value) in [
        ("gpg.format", "openpgp"),
        (
            "gpg.program",
            signer.to_str().expect("temp paths are UTF-8"),
        ),
        ("user.signingKey", "test-key"),
        ("gitbutler.signCommits", "true"),
    ] {
        git_at_dir(root).args(["config", key, value]).run();
    }

    let mut ctx =
        but_ctx::Context::from_repo_for_testing(open_repo(root)?)?.with_memory_app_cache();
    let target_ref = gix::refs::FullName::try_from("refs/heads/main")?;
    but_api::workspace::set_target_ref_and_init_project(&mut ctx, target_ref.as_ref(), None)?;
    git_at_dir(root)
        .args(["update-ref", "refs/remotes/origin/main", "main"])
        .run();
    ctx.invalidate_workspace_cache()?;
    let rebase_feature = || {
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(feature_commit),
        }]
    };

    but_api::workspace::workspace_integrate_upstream(
        &mut ctx,
        rebase_feature(),
        but_core::DryRun::Yes,
    )?;
    assert!(
        !calls.exists(),
        "the preview discards its rebased commits, so signing them is wasted work"
    );

    but_api::workspace::workspace_integrate_upstream(
        &mut ctx,
        rebase_feature(),
        but_core::DryRun::No,
    )?;
    assert_eq!(
        std::fs::read_to_string(&calls)?.lines().count(),
        1,
        "the real update signs the rebased feature commit"
    );
    Ok(())
}

#[test]
fn the_update_rebases_a_linked_worktree_onto_the_target() -> anyhow::Result<()> {
    let (repo, tmp) = repo_with_feature_branch()?;
    drop(repo);
    let root = tmp.path();
    git_at_dir(root)
        .args(["switch", "-c", "gitbutler/workspace"])
        .run();
    git_at_dir(root)
        .args([
            "commit",
            "--allow-empty",
            "-m",
            "GitButler Workspace Commit",
        ])
        .run();
    let mut ctx =
        but_ctx::Context::from_repo_for_testing(open_repo(root)?)?.with_memory_app_cache();
    let target_ref = gix::refs::FullName::try_from("refs/heads/main")?;
    but_api::workspace::set_target_ref_and_init_project(&mut ctx, target_ref.as_ref(), None)?;
    ctx.db.get_cache_mut()?.worktree_meta_mut().mark_adopted()?;

    let outside = tempfile::tempdir()?;
    let agent = outside.path().join("agent");
    git_at_dir(root)
        .args(["worktree", "add", "-q", "-b", "agent"])
        .arg(&agent)
        .arg("origin/main")
        .run();
    git_at_dir(&agent)
        .args(["commit", "--allow-empty", "-m", "agent work"])
        .run();
    git_at_dir(root)
        .args(["update-ref", "refs/remotes/origin/main", "main"])
        .run();
    ctx.invalidate_workspace_cache()?;
    let behind = |ctx: &mut but_ctx::Context| -> anyhow::Result<(bool, Vec<String>)> {
        let mut guard = ctx.exclusive_worktree_access();
        let base =
            but_api::legacy::virtual_branches::get_base_branch_data(ctx, guard.write_permission())?
                .expect("the target is set");
        Ok((base.workspace_behind, base.worktrees_behind))
    };
    assert_eq!(
        behind(&mut ctx)?,
        (true, vec!["agent".to_string()]),
        "the workspace lacks the new target commit, and so does agent, which rests on the old one"
    );

    but_api::workspace::workspace_integrate_upstream(&mut ctx, vec![], but_core::DryRun::No)?;

    let repo = open_repo(root)?;
    assert_eq!(
        repo.rev_parse_single("agent~")?.detach(),
        repo.rev_parse_single("origin/main")?.detach(),
        "the worktree's own commit now rests on the target's newest commit, as `but pull` leaves it"
    );
    ctx.invalidate_workspace_cache()?;
    assert_eq!(
        behind(&mut ctx)?,
        (false, vec![]),
        "the update brought the workspace and agent onto the target"
    );
    Ok(())
}

/// A blobless partial clone (`git clone --filter=blob:none`) fetches the target's new commits and
/// trees, but not their blobs; Git fetches those on first use, gix doesn't. The integration
/// fetches the blobs the target gained before it rebases onto it.
#[test]
#[cfg(feature = "legacy")]
fn partial_clone_fetches_the_blobs_the_target_gained() -> anyhow::Result<()> {
    crate::support::disable_askpass();
    let tmp = tempfile::tempdir()?;
    let remote_dir = tmp.path().join("remote");
    std::fs::create_dir_all(remote_dir.join("src"))?;
    git_at_dir(&remote_dir).args(["init", "-b", "main"]).run();
    git_at_dir(&remote_dir)
        .args(["config", "uploadpack.allowFilter", "true"])
        .run();
    write_file(&remote_dir, "src/a.txt", "a\n")?;
    git_at_dir(&remote_dir).args(["add", "src/a.txt"]).run();
    git_at_dir(&remote_dir).args(["commit", "-m", "a"]).run();

    // Only a transport applies the filter, so clone through the file protocol.
    let clone_dir = tmp.path().join("clone");
    let remote_url = format!("file://{}", remote_dir.display());
    git_at_dir(tmp.path())
        .args(["clone", "--filter=blob:none", &remote_url])
        .arg(&clone_dir)
        .run();
    let root = clone_dir.as_path();
    // A branch adding a file next to the one the target gains, so the rebase rewrites that tree.
    git_at_dir(root).args(["switch", "-c", "feature"]).run();
    write_file(root, "src/b.txt", "b\n")?;
    git_at_dir(root).args(["add", "src/b.txt"]).run();
    git_at_dir(root).args(["commit", "-m", "b"]).run();
    let feature_commit = open_repo(root)?
        .find_reference("refs/heads/feature")?
        .peel_to_id()?
        .detach();
    git_at_dir(root)
        .args(["switch", "-c", "gitbutler/workspace"])
        .run();
    git_at_dir(root)
        .args([
            "commit",
            "--allow-empty",
            "-m",
            "GitButler Workspace Commit",
        ])
        .run();
    let mut ctx =
        but_ctx::Context::from_repo_for_testing(open_repo(root)?)?.with_memory_app_cache();
    let target_ref = gix::refs::FullName::try_from("refs/heads/main")?;
    but_api::workspace::set_target_ref_and_init_project(&mut ctx, target_ref.as_ref(), None)?;

    // The target gains a file. The fetch brings the commit and its tree, but not the blob.
    write_file(&remote_dir, "src/c.txt", "c\n")?;
    git_at_dir(&remote_dir).args(["add", "src/c.txt"]).run();
    git_at_dir(&remote_dir).args(["commit", "-m", "c"]).run();
    git_at_dir(root).args(["fetch", "--quiet"]).run();
    ctx.invalidate_workspace_cache()?;
    let gained_blob = open_repo(root)?
        .rev_parse_single("refs/remotes/origin/main:src/c.txt")?
        .detach();
    assert!(
        !open_repo(root)?.has_object(gained_blob),
        "the fetch left the blob behind, as a partial clone does"
    );

    but_api::workspace::workspace_integrate_upstream(
        &mut ctx,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(feature_commit),
        }],
        but_core::DryRun::No,
    )?;

    assert!(
        open_repo(root)?.has_object(gained_blob),
        "the integration fetched the blob before the rebase needed it"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("src/c.txt"))?,
        "c\n",
        "the target's new file is checked out"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("src/b.txt"))?,
        "b\n",
        "the rebased branch kept its file"
    );
    Ok(())
}
