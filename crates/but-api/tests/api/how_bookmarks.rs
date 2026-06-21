use std::{fs, path::Path};

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

fn current_head(repo_path: &Path) -> String {
    let output = git_at_dir(repo_path)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git rev-parse runs");
    assert!(output.status.success(), "git rev-parse should succeed");
    String::from_utf8(output.stdout)
        .expect("git output is utf8")
        .trim()
        .to_owned()
}

#[test]
fn create_and_list_bookmark_with_private_ref() -> anyhow::Result<()> {
    let (repo, _tmp) = repository()?;
    let ctx = but_ctx::Context::from_repo(repo)?.with_memory_app_cache();

    let bookmark = but_api::how::how_create_bookmark(
        &ctx,
        "Prototype".into(),
        but_api::how::HowBookmarkKind::User,
    )?;

    assert_eq!(bookmark.name, "Prototype");
    assert_eq!(bookmark.kind, but_api::how::HowBookmarkKind::User);
    assert!(bookmark.is_current, "new bookmark points to HEAD");

    let repo = ctx.repo.get()?;
    let bookmark_ref = format!("refs/gitbutler/how/bookmarks/{}", bookmark.id);
    assert!(
        repo.try_find_reference(bookmark_ref.as_str())?.is_some(),
        "bookmark should be backed by a private ref"
    );
    let local_branch_ref = format!("refs/heads/{}", bookmark.id);
    assert!(
        repo.try_find_reference(local_branch_ref.as_str())?
            .is_none(),
        "bookmark should not create a local branch"
    );

    let bookmarks = but_api::how::how_list_bookmarks(&ctx)?;
    assert_eq!(bookmarks.len(), 1);
    assert_eq!(bookmarks[0].id, bookmark.id);
    Ok(())
}

#[test]
fn duplicate_bookmark_names_are_allowed() -> anyhow::Result<()> {
    let (repo, _tmp) = repository()?;
    let ctx = but_ctx::Context::from_repo(repo)?.with_memory_app_cache();

    let first = but_api::how::how_create_bookmark(
        &ctx,
        "Same".into(),
        but_api::how::HowBookmarkKind::User,
    )?;
    let second = but_api::how::how_create_bookmark(
        &ctx,
        "Same".into(),
        but_api::how::HowBookmarkKind::User,
    )?;

    assert_ne!(first.id, second.id, "bookmark IDs define identity");
    assert_eq!(but_api::how::how_list_bookmarks(&ctx)?.len(), 2);
    Ok(())
}

#[test]
fn switch_bookmark_moves_main_and_worktree() -> anyhow::Result<()> {
    let (repo, tmp) = repository()?;
    let mut ctx = but_ctx::Context::from_repo(repo)?.with_memory_app_cache();
    let first_head = current_head(tmp.path());
    let bookmark = but_api::how::how_create_bookmark(
        &ctx,
        "Initial".into(),
        but_api::how::HowBookmarkKind::User,
    )?;

    commit_file(tmp.path(), "notes.md", "second\n", "Second");
    assert_ne!(current_head(tmp.path()), first_head);

    but_api::how::how_switch_bookmark(&mut ctx, bookmark.id.clone())?;

    assert_eq!(current_head(tmp.path()), first_head);
    assert_eq!(
        fs::read_to_string(tmp.path().join("notes.md"))?,
        "initial\n",
        "switching should update the worktree"
    );
    let bookmarks = but_api::how::how_list_bookmarks(&ctx)?;
    assert!(
        bookmarks
            .iter()
            .any(|candidate| candidate.id == bookmark.id && candidate.is_current),
        "bookmark should be current after switching"
    );
    Ok(())
}

#[test]
fn switching_bookmark_does_not_move_it_above_newer_bookmarks() -> anyhow::Result<()> {
    let (repo, tmp) = repository()?;
    let mut ctx = but_ctx::Context::from_repo(repo)?.with_memory_app_cache();
    let older = but_api::how::how_create_bookmark(
        &ctx,
        "Older".into(),
        but_api::how::HowBookmarkKind::User,
    )?;

    commit_file(tmp.path(), "notes.md", "newer\n", "Newer");
    let newer = but_api::how::how_create_bookmark(
        &ctx,
        "Newer".into(),
        but_api::how::HowBookmarkKind::User,
    )?;

    but_api::how::how_switch_bookmark(&mut ctx, older.id.clone())?;

    let bookmarks = but_api::how::how_list_bookmarks(&ctx)?;
    assert_eq!(bookmarks[0].id, newer.id);
    assert_eq!(bookmarks[1].id, older.id);
    assert!(bookmarks[1].is_current);
    Ok(())
}

#[test]
fn update_rename_and_delete_bookmark() -> anyhow::Result<()> {
    let (repo, tmp) = repository()?;
    let ctx = but_ctx::Context::from_repo(repo)?.with_memory_app_cache();
    let bookmark = but_api::how::how_create_bookmark(
        &ctx,
        "Before".into(),
        but_api::how::HowBookmarkKind::Auto,
    )?;
    let first_target = bookmark.target_commit_id.clone();

    commit_file(tmp.path(), "notes.md", "updated\n", "Updated");
    let updated = but_api::how::how_update_bookmark(&ctx, bookmark.id.clone())?;
    assert_ne!(updated.target_commit_id, first_target);
    assert_eq!(updated.kind, but_api::how::HowBookmarkKind::User);

    let renamed = but_api::how::how_rename_bookmark(&ctx, bookmark.id.clone(), "After".into())?;
    assert_eq!(renamed.name, "After");
    assert_eq!(renamed.kind, but_api::how::HowBookmarkKind::User);

    but_api::how::how_delete_bookmark(&ctx, bookmark.id.clone())?;
    assert!(but_api::how::how_list_bookmarks(&ctx)?.is_empty());
    let repo = ctx.repo.get()?;
    let bookmark_ref = format!("refs/gitbutler/how/bookmarks/{}", bookmark.id);
    assert!(
        repo.try_find_reference(bookmark_ref.as_str())?.is_none(),
        "deleting should remove the bookmark pointer"
    );
    Ok(())
}
