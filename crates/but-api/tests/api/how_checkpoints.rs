use std::{fs, path::Path};

use but_api::how::{
    HowCreateCheckpointResult, HowUpdateCheckpointMessageResult,
    HowUpdateCheckpointMessageSkippedReason,
};
use but_testsupport::{CommandExt as _, git_at_dir, gix_testtools::tempfile::TempDir};

fn repository() -> anyhow::Result<(gix::Repository, TempDir)> {
    let tmp = TempDir::new()?;
    git_at_dir(tmp.path()).args(["init", "-b", "main"]).run();
    git_at_dir(tmp.path())
        .args(["config", "user.email", "user@example.com"])
        .run();
    git_at_dir(tmp.path())
        .args(["config", "user.name", "User"])
        .run();
    git_at_dir(tmp.path())
        .args(["config", "gitbutler.signCommits", "false"])
        .run();
    commit_file(tmp.path(), "notes.md", "initial\n", "Initial");
    let repo = gix::open(tmp.path())?;
    Ok((repo, tmp))
}

fn commit_file(repo_path: &Path, name: &str, contents: &str, message: &str) {
    fs::write(repo_path.join(name), contents).expect("write test file");
    git_at_dir(repo_path).args(["add", "--all"]).run();
    git_at_dir(repo_path)
        .args(["commit", "--no-gpg-sign", "--message", message])
        .run();
}

fn write_file(repo_path: &Path, name: &str, contents: &str) {
    fs::write(repo_path.join(name), contents).expect("write test file");
}

fn create_checkpoint(
    ctx: &mut but_ctx::Context,
    message: &str,
) -> anyhow::Result<(String, String)> {
    match but_api::how::how_create_checkpoint(ctx, message.into())? {
        HowCreateCheckpointResult::Created {
            commit_id,
            change_id,
        } => Ok((commit_id, change_id)),
        HowCreateCheckpointResult::Unchanged => {
            anyhow::bail!("test expected a checkpoint to be created")
        }
    }
}

fn run_git(repo_path: &Path, args: &[&str]) -> String {
    let output = git_at_dir(repo_path)
        .args(args)
        .output()
        .expect("git command runs");
    assert!(
        output.status.success(),
        "git command should succeed: {args:?}"
    );
    String::from_utf8(output.stdout)
        .expect("git output is utf8")
        .trim()
        .to_owned()
}

#[test]
fn create_checkpoint_stores_explicit_change_id() -> anyhow::Result<()> {
    let (repo, tmp) = repository()?;
    let mut ctx = but_ctx::Context::from_repo(repo)?.with_memory_app_cache();
    write_file(tmp.path(), "notes.md", "first checkpoint\n");

    let (commit_id, change_id) = create_checkpoint(&mut ctx, "Checkpoint: first")?;
    let checkpoints = but_api::how::how_list_checkpoints(&ctx, 10)?;

    assert_eq!(checkpoints.len(), 1);
    assert_eq!(checkpoints[0].id, commit_id);
    assert_eq!(
        checkpoints[0].change_id.as_deref(),
        Some(change_id.as_str())
    );
    let commit_object = run_git(tmp.path(), &["cat-file", "-p", "HEAD"]);
    assert!(
        commit_object.contains(&change_id),
        "checkpoint commit should store an explicit Change ID header"
    );
    Ok(())
}

#[test]
fn update_checkpoint_message_by_change_id_rebases_descendants() -> anyhow::Result<()> {
    let (repo, tmp) = repository()?;
    let mut ctx = but_ctx::Context::from_repo(repo)?.with_memory_app_cache();

    write_file(tmp.path(), "notes.md", "checkpoint A\n");
    let (a_commit_id, a_change_id) = create_checkpoint(&mut ctx, "Checkpoint: A")?;
    write_file(tmp.path(), "notes.md", "checkpoint B\n");
    let (b_commit_id, b_change_id) = create_checkpoint(&mut ctx, "Checkpoint: B")?;

    let result = but_api::how::how_update_checkpoint_message_by_change_id(
        &mut ctx,
        a_change_id.clone(),
        "Checkpoint: renamed A\n\nA better summary.".into(),
    )?;

    let HowUpdateCheckpointMessageResult::Updated { checkpoint } = result else {
        anyhow::bail!("test expected checkpoint update")
    };
    assert_eq!(checkpoint.title, "Checkpoint: renamed A");
    assert_eq!(checkpoint.change_id.as_deref(), Some(a_change_id.as_str()));
    assert_ne!(checkpoint.id, a_commit_id);

    let checkpoints = but_api::how::how_list_checkpoints(&ctx, 10)?;
    assert_eq!(checkpoints.len(), 2);
    assert_eq!(checkpoints[0].title, "Checkpoint: B");
    assert_eq!(
        checkpoints[0].change_id.as_deref(),
        Some(b_change_id.as_str())
    );
    assert_ne!(checkpoints[0].id, b_commit_id);
    assert_eq!(checkpoints[1].title, "Checkpoint: renamed A");
    assert_eq!(
        fs::read_to_string(tmp.path().join("notes.md"))?,
        "checkpoint B\n"
    );
    Ok(())
}

#[test]
fn update_checkpoint_message_skips_published_checkpoints() -> anyhow::Result<()> {
    let (repo, tmp) = repository()?;
    let mut ctx = but_ctx::Context::from_repo(repo)?.with_memory_app_cache();

    write_file(tmp.path(), "notes.md", "published checkpoint\n");
    let (_commit_id, change_id) = create_checkpoint(&mut ctx, "Checkpoint: published")?;

    let bare = TempDir::new()?;
    git_at_dir(bare.path()).args(["init", "--bare"]).run();
    git_at_dir(tmp.path())
        .args(["remote", "add", "origin", bare.path().to_str().unwrap()])
        .run();
    git_at_dir(tmp.path())
        .args(["push", "-u", "origin", "main"])
        .run();

    drop(ctx);
    let repo = gix::open(tmp.path())?;
    let mut reopened_ctx = but_ctx::Context::from_repo(repo)?.with_memory_app_cache();

    let result = but_api::how::how_update_checkpoint_message_by_change_id(
        &mut reopened_ctx,
        change_id,
        "Checkpoint: should not publish rewrite".into(),
    )?;

    assert!(matches!(
        result,
        HowUpdateCheckpointMessageResult::Skipped {
            reason: HowUpdateCheckpointMessageSkippedReason::Published
        }
    ));
    assert_eq!(
        run_git(tmp.path(), &["log", "-1", "--format=%s"]),
        "Checkpoint: published"
    );
    Ok(())
}
