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
