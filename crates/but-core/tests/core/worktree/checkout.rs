use bstr::ByteSlice;
use but_core::worktree::{checkout, safe_checkout_from_head};
use but_testsupport::{
    CommandExt, git_at_dir, git_status, open_repo, read_only_in_memory_scenario,
    visualize_commit_graph_all, visualize_disk_tree_skip_dot_git, visualize_index,
    writable_scenario, writable_scenario_slow,
};
use gix::object::tree::EntryKind;
use snapbox::prelude::*;

use crate::worktree::utils::build_commit;

#[test]
fn update_unborn_head() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("unborn-empty");
    snapbox::assert_data_eq!(visualize_commit_graph_all(&repo)?, snapbox::str![""]);
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    let empty_tree = repo.empty_tree().id;
    let head_commit = repo.new_commit("init", empty_tree, None::<gix::ObjectId>)?;

    let out = safe_checkout_from_head(head_commit.id, &repo, Default::default())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    head_update: "Update refs/heads/main to Some(Object(Sha1(31ec8eacfba4051fd673e4fe23c775e87896a463)))",
}

"#]]
    );

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 31ec8ea (HEAD -> main) init

"#]]
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);
    Ok(())
}

#[test]
fn update_unborn_head_in_sha256_repository() -> anyhow::Result<()> {
    let tmp = but_testsupport::gix_testtools::tempfile::TempDir::new()?;
    git_at_dir(tmp.path())
        .args(["init", "--object-format=sha256"])
        .run();
    let repo = open_repo(tmp.path())?;

    let empty_tree = repo.empty_tree().id;
    let head_commit = repo.new_commit("init", empty_tree, None::<gix::ObjectId>)?;
    safe_checkout_from_head(head_commit.id, &repo, Default::default())?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 2c00519 (HEAD -> main) init

"#]]
    );
    Ok(())
}

#[test]
fn deleting_what_the_worktree_already_deleted_leaves_no_index_entry() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("worktree-deletion-the-target-also-makes");
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        snapbox::str![[r#"
 D x

"#]]
    );

    let target = repo.rev_parse_single("b")?.detach();
    safe_checkout_from_head(target, &repo, Default::default())?;

    // Like `git checkout b`: both sides deleted `x`, so nothing is left to show.
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);
    snapbox::assert_data_eq!(
        visualize_index(&*repo.index()?),
        snapbox::str![[r#"
100644:2fa992c keep

"#]]
    );
    Ok(())
}

#[test]
#[cfg(unix)]
fn deleting_below_a_directory_the_worktree_turned_into_a_symlink_leaves_no_index_entry()
-> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("worktree-dir-turned-symlink-the-target-deletes");
    let workdir = repo.workdir().expect("non-bare");
    std::os::unix::fs::symlink("elsewhere", workdir.join("d"))?;

    let target = repo.rev_parse_single("b")?.detach();
    safe_checkout_from_head(target, &repo, Default::default())?;

    // Like Git, `d/x` behind the symlink isn't in the worktree, so both sides deleted it.
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        snapbox::str![[r#"
?? d

"#]]
    );
    assert_eq!(
        std::fs::read_to_string(workdir.join("elsewhere/x"))?,
        "elsewhere\n",
        "nothing behind the symlink changes"
    );
    Ok(())
}

#[test]
fn deleting_a_file_the_worktree_replaced_with_a_directory_leaves_no_index_entry()
-> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("worktree-deletion-the-target-also-makes");
    std::fs::create_dir(repo.workdir().expect("non-bare").join("x"))?;

    let target = repo.rev_parse_single("b")?.detach();
    safe_checkout_from_head(target, &repo, Default::default())?;

    // An empty directory isn't the file, so both sides deleted `x`. Git doesn't list empty directories.
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);
    Ok(())
}

#[test]
fn checkout_keeps_the_stages_of_an_unresolved_conflict_the_worktree_deleted() -> anyhow::Result<()>
{
    let (repo, _tmp) = writable_scenario("worktree-deletion-the-target-also-makes");
    let base = repo.rev_parse_single("HEAD:x")?.detach();
    let ours = repo.write_blob("ours\n")?.detach();
    let theirs = repo.write_blob("theirs\n")?.detach();
    but_testsupport::git(&repo)
        .args(["update-index", "--force-remove", "x"])
        .run();
    let mut info = but_testsupport::git(&repo)
        .args(["update-index", "--index-info"])
        .stdin(std::process::Stdio::piped())
        .spawn()?;
    std::io::Write::write_all(
        info.stdin.as_mut().expect("piped"),
        format!("100644 {base} 1\tx\n100644 {ours} 2\tx\n100644 {theirs} 3\tx\n").as_bytes(),
    )?;
    assert!(info.wait()?.success());
    let stages = but_testsupport::git(&repo)
        .args(["ls-files", "-s", "x"])
        .output()?
        .stdout;
    assert_eq!(stages.lines().count(), 3, "the conflict is in the index");

    let target = repo.rev_parse_single("b")?.detach();
    let _ = safe_checkout_from_head(target, &repo, Default::default());

    // Git refuses to check out over an unresolved conflict. Whatever checkout does, the
    // conflict stays.
    assert_eq!(
        but_testsupport::git(&repo)
            .args(["ls-files", "-s", "x"])
            .output()?
            .stdout
            .as_bstr(),
        stages.as_bstr()
    );
    Ok(())
}

#[test]
fn checkout_keeps_a_staged_modification_the_worktree_deleted() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("worktree-deletion-the-target-also-makes");
    let x = repo.workdir().expect("non-bare").join("x");
    std::fs::write(&x, "staged\n")?;
    but_testsupport::git(&repo).args(["add", "x"]).run();
    std::fs::remove_file(&x)?;
    let staged = but_testsupport::git(&repo)
        .args(["ls-files", "-s", "x"])
        .output()?
        .stdout;

    let target = repo.rev_parse_single("b")?.detach();
    safe_checkout_from_head(target, &repo, Default::default())?;

    // `git checkout b` refuses to lose the staged content. The checkout keeps it in the index.
    assert_eq!(
        but_testsupport::git(&repo)
            .args(["ls-files", "-s", "x"])
            .output()?
            .stdout
            .as_bstr(),
        staged.as_bstr()
    );
    Ok(())
}

#[test]
fn checkout_keeps_a_staged_mode_change_the_worktree_deleted() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("worktree-deletion-the-target-also-makes");
    let x = repo.workdir().expect("non-bare").join("x");
    std::fs::write(&x, "one\n")?;
    but_testsupport::git(&repo)
        .args(["update-index", "--chmod=+x", "x"])
        .run();
    std::fs::remove_file(&x)?;
    let staged = but_testsupport::git(&repo)
        .args(["ls-files", "-s", "x"])
        .output()?
        .stdout;
    assert!(staged.starts_with(b"100755"), "the mode change is staged");

    let target = repo.rev_parse_single("b")?.detach();
    safe_checkout_from_head(target, &repo, Default::default())?;

    assert_eq!(
        but_testsupport::git(&repo)
            .args(["ls-files", "-s", "x"])
            .output()?
            .stdout
            .as_bstr(),
        staged.as_bstr()
    );
    Ok(())
}

#[test]
fn checkout_keeps_a_staged_addition_the_worktree_deleted() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("staged-addition-deleted-from-worktree");
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        snapbox::str![[r#"
AD n

"#]]
    );

    let target = repo.rev_parse_single("b")?.detach();
    safe_checkout_from_head(target, &repo, Default::default())?;

    // Like `git checkout b`: `b` doesn't know `n`, so the staged addition stays.
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        snapbox::str![[r#"
AD n

"#]]
    );
    Ok(())
}

#[test]
fn a_worktree_modification_the_target_deletes_stays_a_conflict() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("worktree-modification-the-target-deletes");

    let target = repo.rev_parse_single("b")?.detach();
    let out = safe_checkout_from_head(
        target,
        &repo,
        checkout::Options {
            allow_uncommitted_changes_to_conflict_with_new_head: true,
            ..Default::default()
        },
    )?;

    // Like `git checkout -m b`, the modified file stays, conflicted with its deletion.
    // GitButler's `ours` is the worktree, so the sides are swapped compared to Git.
    assert!(out.conflict_occurred);
    snapbox::assert_data_eq!(
        std::fs::read_to_string(repo.workdir().unwrap().join("x"))?,
        snapbox::str![[r#"
two

"#]]
    );
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        snapbox::str![[r#"
UD x

"#]]
    );
    Ok(())
}

#[test]
fn allowed_text_conflicts_keep_markers_stages_and_unrelated_staged_content() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("worktree-modification-the-target-deletes");
    let target = build_commit(
        &repo,
        |tree| {
            tree.upsert("x", EntryKind::Blob, repo.write_blob(b"three\n")?)?;
            Ok(())
        },
        "conflicting target",
    )?
    .id;
    let staged_blob_id = repo.write_blob(b"staged\n")?.detach();
    git_at_dir(repo.workdir().expect("non-bare"))
        .args([
            "update-index",
            "--cacheinfo",
            &format!("100644,{staged_blob_id},keep"),
        ])
        .run();

    let out = safe_checkout_from_head(
        target,
        &repo,
        checkout::Options {
            allow_uncommitted_changes_to_conflict_with_new_head: true,
            ..Default::default()
        },
    )?;

    assert!(
        out.conflict_occurred,
        "the overlapping worktree change remains unresolved"
    );
    snapbox::assert_data_eq!(
        std::fs::read_to_string(repo.workdir().expect("non-bare").join("x"))?,
        snapbox::str![[r#"
<<<<<<< ours
two
=======
three
>>>>>>> theirs

"#]]
    );
    let index = repo.index()?;
    for (stage, contents) in [
        (gix::index::entry::Stage::Base, "one\n"),
        (gix::index::entry::Stage::Ours, "two\n"),
        (gix::index::entry::Stage::Theirs, "three\n"),
    ] {
        let entry = index
            .entry_by_path_and_stage("x".into(), stage)
            .expect("conflict stage exists");
        assert_eq!(
            entry.id,
            repo.write_blob(contents)?.detach(),
            "the original content survives at {stage:?}"
        );
    }
    assert!(
        index
            .entry_by_path_and_stage("x".into(), gix::index::entry::Stage::Unconflicted)
            .is_none(),
        "conflict markers are not staged as ordinary content"
    );
    assert_eq!(
        index
            .entry_by_path("keep".into())
            .expect("unrelated path remains staged")
            .id,
        staged_blob_id,
        "unrelated staged content survives conflict materialization"
    );
    Ok(())
}

#[test]
fn allowed_text_conflicts_honor_diff3_configuration() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("worktree-modification-the-target-deletes");
    git_at_dir(repo.workdir().expect("non-bare"))
        .args(["config", "merge.conflictstyle", "diff3"])
        .run();
    let repo = open_repo(repo.git_dir())?;
    let target = build_commit(
        &repo,
        |tree| {
            tree.upsert("x", EntryKind::Blob, repo.write_blob(b"three\n")?)?;
            Ok(())
        },
        "conflicting target",
    )?
    .id;
    safe_checkout_from_head(
        target,
        &repo,
        checkout::Options {
            allow_uncommitted_changes_to_conflict_with_new_head: true,
            ..Default::default()
        },
    )?;
    snapbox::assert_data_eq!(
        std::fs::read_to_string(repo.workdir().expect("non-bare").join("x"))?,
        snapbox::str![[r#"
<<<<<<< ours
two
||||||| ancestor
one
=======
three
>>>>>>> theirs

"#]]
    );
    Ok(())
}

#[test]
fn failed_checkout_does_not_persist_prepared_index_deletions() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("worktree-deletion-the-target-also-makes");
    let target = build_commit(
        &repo,
        |tree| {
            tree.remove("x")?;
            tree.upsert("blocked", EntryKind::Blob, repo.write_blob(b"target\n")?)?;
            Ok(())
        },
        "remove missing file and add blocked path",
    )?
    .id;
    let workdir = repo.workdir().expect("non-bare");
    std::fs::write(workdir.join(".gitignore"), b"blocked\n")?;
    git_at_dir(workdir).args(["init", "blocked"]).run();
    std::fs::write(workdir.join("blocked/keep"), b"preserve\n")?;
    let index_before = std::fs::read(repo.index_path())?;
    let head_before = repo.head_id()?.detach();

    safe_checkout_from_head(target, &repo, Default::default())
        .expect_err("an ignored repository prevents materializing the target");

    assert_eq!(
        std::fs::read(repo.index_path())?,
        index_before,
        "index deletions prepared before preflight remain unpersisted on failure"
    );
    assert_eq!(
        repo.head_id()?.detach(),
        head_before,
        "a failed checkout does not advance HEAD"
    );
    assert_eq!(
        std::fs::read(workdir.join("blocked/keep"))?,
        b"preserve\n",
        "the obstructing file remains intact"
    );
    Ok(())
}

#[test]
fn deleting_below_a_directory_the_worktree_turned_into_a_file_leaves_no_index_entry()
-> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("worktree-dir-turned-file-the-target-deletes");

    let target = repo.rev_parse_single("b")?.detach();
    safe_checkout_from_head(target, &repo, Default::default())?;

    // Both sides deleted `a/b`, and the file `a` the user put there stays untracked.
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        snapbox::str![[r#"
?? a

"#]]
    );
    Ok(())
}

#[test]
fn no_op_trees_never_touch_worktree() -> anyhow::Result<()> {
    let repo = read_only_in_memory_scenario("all-file-types-renamed-and-modified")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 4e26689 (HEAD -> main) init

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_index(&*repo.index()?),
        snapbox::str![[r#"
100755:01e79c3 executable
100644:3aac70f file
120000:c4c364c link

"#]]
    );
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        snapbox::str![[r#"
 D executable
 D file
 D link
?? executable-renamed
?? file-renamed
?? link-renamed

"#]]
    );

    let a_commit = repo.head_commit()?;

    let out = safe_checkout_from_head(a_commit.id, &repo, Default::default())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    head_update: "None",
}

"#]]
    );

    // Nothing changed
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 4e26689 (HEAD -> main) init

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_index(&*repo.index()?),
        snapbox::str![[r#"
100755:01e79c3 executable
100644:3aac70f file
120000:c4c364c link

"#]]
    );
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        snapbox::str![[r#"
 D executable
 D file
 D link
?? executable-renamed
?? file-renamed
?? link-renamed

"#]]
    );
    Ok(())
}

#[test]
fn conflicted_commits_cannot_be_checked_out() -> anyhow::Result<()> {
    let repo = crate::commit::conflict_repo("normal-and-artificial")?;
    let conflicted = repo.rev_parse_single("conflicted")?.detach();

    let err = safe_checkout_from_head(conflicted, &repo, Default::default())
        .expect_err("safe_checkout must reject GitButler-conflicted commits");
    assert_eq!(
        err.to_string(),
        "Refusing to check out conflicted commit 84503317a1e1464381fcff65ece14bc1f4315b7c",
        "the rejection retains its descriptive message"
    );
    assert_eq!(
        err.downcast_ref::<but_error::Code>(),
        Some(&but_error::Code::ConflictedCommitCheckout),
        "callers can classify the rejection without matching its message"
    );

    safe_checkout_from_head(
        conflicted,
        &repo,
        checkout::Options {
            allow_conflicted_commit_checkout: true,
            ..Default::default()
        },
    )
    .expect("internal callers can explicitly opt into conflicted commit checkout");

    Ok(())
}

#[test]
fn pure_deletion_checkout_does_not_restore_unrelated_worktree_deletions() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("all-file-types-renamed-and-modified");
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        snapbox::str![[r#"
 D executable
 D file
 D link
?? executable-renamed
?? file-renamed
?? link-renamed

"#]]
    );

    snapbox::assert_data_eq!(
        visualize_index(&*repo.index()?),
        snapbox::str![[r#"
100755:01e79c3 executable
100644:3aac70f file
120000:c4c364c link

"#]]
    );

    let new_commit = build_commit(
        &repo,
        |tree| {
            tree.remove("executable")?;
            Ok(())
        },
        "delete executable",
    )?;

    let out = safe_checkout_from_head(new_commit.id, &repo, Default::default())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    head_update: "Update refs/heads/main to Some(Object(Sha1(5eedd314adfb480212989a303c7651717062a9b2)))",
}

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?.to_string(),
        snapbox::str![[r#"
.
├── .git:40755
├── executable-renamed:100755
├── fifo-should-be-ignored:10644
├── file-renamed:100644
└── link-renamed:120755

"#]]
    );

    Ok(())
}

#[test]
fn pure_deletion_checkout_keeps_non_intersecting_worktree_deletion() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("unborn-empty");

    let blob_id = repo.write_blob(b"content")?;
    let mut editor = repo.empty_tree().edit()?;
    editor.upsert("a.txt", EntryKind::Blob, blob_id)?;
    editor.upsert("b.txt", EntryKind::Blob, blob_id)?;
    editor.upsert("c.txt", EntryKind::Blob, blob_id)?;
    let initial_tree_id = editor.write()?.detach();
    let initial_commit = repo.new_commit("init", initial_tree_id, None::<gix::ObjectId>)?;
    safe_checkout_from_head(initial_commit.id, &repo, Default::default())?;

    std::fs::remove_file(repo.workdir_path("b.txt").expect("non-bare repository"))?;
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        snapbox::str![[r#"
 D b.txt

"#]]
    );

    let new_commit = build_commit(
        &repo,
        |tree| {
            tree.remove("a.txt")?;
            Ok(())
        },
        "delete a.txt",
    )?;
    let out = safe_checkout_from_head(new_commit.id, &repo, Default::default())?;
    assert!(out.head_update.is_some());

    assert!(!repo.workdir_path("a.txt").unwrap().exists());
    assert!(!repo.workdir_path("b.txt").unwrap().exists());
    assert!(repo.workdir_path("c.txt").unwrap().exists());
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        snapbox::str![[r#"
 D b.txt

"#]]
    );

    Ok(())
}

#[test]
fn pure_deletion_checkout_keeps_empty_worktree_root() -> anyhow::Result<()> {
    let root = but_testsupport::gix_testtools::tempfile::TempDir::new()?;
    let git_dir = root.path().join("git-dir");
    let worktree = root.path().join("worktree");
    std::fs::create_dir(&worktree)?;

    git_at_dir(root.path())
        .args(["init", "--bare"])
        .arg(&git_dir)
        .run();
    git_at_dir(root.path())
        .arg(format!("--git-dir={}", git_dir.display()))
        .args(["config", "core.bare", "false"])
        .run();
    git_at_dir(root.path())
        .arg(format!("--git-dir={}", git_dir.display()))
        .args(["config", "core.worktree"])
        .arg(&worktree)
        .run();
    let repo = open_repo(&git_dir)?;

    let blob_id = repo.write_blob(b"content")?;
    let mut editor = repo.empty_tree().edit()?;
    editor.upsert("nested/only.txt", EntryKind::Blob, blob_id)?;
    let initial_tree_id = editor.write()?.detach();
    let initial_commit = repo.new_commit("init", initial_tree_id, None::<gix::ObjectId>)?;
    safe_checkout_from_head(initial_commit.id, &repo, Default::default())?;

    snapbox::assert_data_eq!(
        visualize_disk_tree_skip_dot_git(&worktree)?.to_string(),
        snapbox::str![[r#"
.
└── nested:40755
    └── only.txt:100644

"#]]
    );

    let new_commit = build_commit(
        &repo,
        |tree| {
            tree.remove("nested/only.txt")?;
            Ok(())
        },
        "delete only file",
    )?;
    safe_checkout_from_head(new_commit.id, &repo, Default::default())?;
    assert!(
        worktree.is_dir(),
        "safe checkout must not delete the worktree root while cleaning up empty parents"
    );
    snapbox::assert_data_eq!(
        visualize_disk_tree_skip_dot_git(&worktree)?.to_string(),
        snapbox::str![[r#"
.

"#]]
    );

    Ok(())
}

#[test]
fn worktree_and_index_deletions_are_ignored_in_snapshots() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("deletion-addition-untracked");
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 226d5ea (HEAD -> main) init

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_index(&*repo.index()?),
        snapbox::str![[r#"
100644:3e75765 added-to-index
100644:d95f3ad to-be-deleted

"#]]
    );
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        snapbox::str![[r#"
A  added-to-index
 D to-be-deleted
D  to-be-deleted-in-index
?? untracked

"#]]
    );

    // Turn deleted files into directory - these won't conflict no matter what they were in the index.
    let new_commit = build_commit(
        &repo,
        |tree| {
            let empty_blob = repo.empty_blob();
            tree.upsert("to-be-deleted/a", EntryKind::Blob, empty_blob.id)?;
            // TODO(gix): needs `gix` impl of checkout as `git2` fails, trying to access a null object
            //            The issue is that it should checkout a file inside of a directory, which was previously
            //            a file that is deleted from the index and the worktree.
            // tree.upsert("to-be-deleted-in-index/a", EntryKind::Blob, empty_blob.id)?;
            Ok(())
        },
        "turn changed file into a directory",
    )?;

    let out = safe_checkout_from_head(new_commit.id, &repo, Default::default())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    head_update: "Update refs/heads/main to Some(Object(Sha1(24f802a1250d2f84e1f49094e3b8bb1e5c0d29ad)))",
}

"#]]
    );

    // The checkout succeeded while preserving unrelated index and worktree changes.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 24f802a (HEAD -> main) turn changed file into a directory
* 226d5ea init

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_index(&*repo.index()?),
        snapbox::str![[r#"
100644:3e75765 added-to-index
100644:e69de29 to-be-deleted/a

"#]]
    );
    // `to-be-deleted-in-index` was staged for deletion (`git rm`) and is no longer
    // restored by the checkout — the checkout only touches `to-be-deleted` → `to-be-deleted/a`.
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        snapbox::str![[r#"
A  added-to-index
D  to-be-deleted-in-index
?? untracked

"#]]
    );

    Ok(())
}

#[test]
fn worktree_changes_do_not_cause_conflict_markers_but_fail() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("mixed-hunk-modifications");
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 647cc94 (HEAD -> main) init

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_index(&*repo.index()?),
        snapbox::str![[r#"
100755:3d3b36f file
100755:cb89473 file-in-index
100644:3d3b36f file-renamed-in-index
100644:3d3b36f file-to-be-renamed

"#]]
    );
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        snapbox::str![[r#"
 M file
M  file-in-index
RM file-to-be-renamed-in-index -> file-renamed-in-index
 D file-to-be-renamed
?? file-renamed

"#]]
    );
    let file_path = repo.workdir_path("file").unwrap();
    let actual = std::fs::read_to_string(&file_path)?;
    snapbox::assert_data_eq!(
        actual.to_debug(),
        snapbox::str![[r#"
"1\n2\n3\n4\n5\n6-7\n8\n9\nten\neleven\n12\n20\n21\n22\n15\n16\n"

"#]]
        .raw()
    );

    // In the target tree, make a surgical edit (one changed line) so the changes should still apply cleany
    let new_commit = build_commit(
        &repo,
        |tree| {
            let blob_id = repo.write_blob(
                b"5
6
7
8
9
10
11
12
13
14
15
16
this will cause a conflict
17
18
",
            )?;
            tree.upsert("file", EntryKind::Blob, blob_id)?;
            Ok(())
        },
        "edited 'file' (add single line)",
    )?;

    let err = safe_checkout_from_head(new_commit.id, &repo, Default::default()).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Uncommitted files would be overwritten by checkout: \"file\"",
        "we check for conflict markers, and fail."
    );
    // Nothing else changes
    let actual = std::fs::read_to_string(&file_path)?;
    snapbox::assert_data_eq!(
        actual.to_debug(),
        snapbox::str![[r#"
"1\n2\n3\n4\n5\n6-7\n8\n9\nten\neleven\n12\n20\n21\n22\n15\n16\n"

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 647cc94 (HEAD -> main) init

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_index(&*repo.index()?),
        snapbox::str![[r#"
100755:3d3b36f file
100755:cb89473 file-in-index
100644:3d3b36f file-renamed-in-index
100644:3d3b36f file-to-be-renamed

"#]]
    );

    snapbox::assert_data_eq!(
        git_status(&repo)?,
        snapbox::str![[r#"
 M file
M  file-in-index
RM file-to-be-renamed-in-index -> file-renamed-in-index
 D file-to-be-renamed
?? file-renamed

"#]]
    );

    Ok(())
}

#[test]
fn worktree_snapshot_reapplies_with_hunk_granularity() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("mixed-hunk-modifications");
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 647cc94 (HEAD -> main) init

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_index(&*repo.index()?),
        snapbox::str![[r#"
100755:3d3b36f file
100755:cb89473 file-in-index
100644:3d3b36f file-renamed-in-index
100644:3d3b36f file-to-be-renamed

"#]]
    );
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        snapbox::str![[r#"
 M file
M  file-in-index
RM file-to-be-renamed-in-index -> file-renamed-in-index
 D file-to-be-renamed
?? file-renamed

"#]]
    );
    let file_path = repo.workdir_path("file").unwrap();
    let actual = std::fs::read_to_string(&file_path)?;
    snapbox::assert_data_eq!(
        actual,
        snapbox::str![[r#"
1
2
3
4
5
6-7
8
9
ten
eleven
12
20
21
22
15
16

"#]]
    );

    // In the target tree, make a surgical edit (one changed line) so the changes should still apply cleany
    let new_commit = build_commit(
        &repo,
        |tree| {
            let blob_id = repo.write_blob(
                b"5
6
7
8
inserted in new tree
9
10
11
12
13
14
15
16
17
18
",
            )?;
            tree.upsert("file", EntryKind::Blob, blob_id)?;
            Ok(())
        },
        "edited 'file' (add single line)",
    )?;

    let out = safe_checkout_from_head(new_commit.id, &repo, Default::default())
        .expect("no error as we keep the snapshot for later");
    // File is still changed, after all we re-applied the worktree changes.
    let actual = std::fs::read_to_string(&file_path)?;
    snapbox::assert_data_eq!(
        actual,
        snapbox::str![[r#"
1
2
3
4
5
6-7
8
inserted in new tree
9
ten
eleven
12
20
21
22
15
16

"#]]
    );

    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    head_update: "Update refs/heads/main to Some(Object(Sha1(89b113aeae66a3cb1116bb23a195422edbd6af27)))",
}

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 89b113a (HEAD -> main) edited 'file' (add single line)
* 647cc94 init

"#]]
    );
    // `file-to-be-renamed-in-index` is no longer restored by the checkout — the checkout
    // only touches `file`, so the index rename (`RM file-to-be-renamed-in-index -> …`) is preserved.
    snapbox::assert_data_eq!(
        visualize_index(&*repo.index()?),
        snapbox::str![[r#"
100644:832f532 file
100755:cb89473 file-in-index
100644:3d3b36f file-renamed-in-index
100644:3d3b36f file-to-be-renamed

"#]]
    );
    // Notably, 'file' is not in the index anymore, as that now always matches the worktree.
    // The rename of `file-to-be-renamed-in-index` and deletion of `file-to-be-renamed` are
    // preserved — the checkout only touched `file`.
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        snapbox::str![[r#"
M  file
M  file-in-index
RM file-to-be-renamed-in-index -> file-renamed-in-index
 D file-to-be-renamed
?? file-renamed

"#]]
    );

    Ok(())
}

#[test]
fn worktree_snapshot_of_legacy_crlf_blob_merges_cleanly_with_independent_target_change()
-> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("legacy-crlf-blob-with-gitattributes");
    let file_path = repo.workdir_path("ImportOrdersJob.cs").unwrap();
    let legacy_blob = repo
        .find_object(repo.rev_parse_single("@:ImportOrdersJob.cs")?)?
        .into_blob();
    assert_eq!(
        legacy_blob.data.as_bstr(),
        "1\r\n2\r\nunchanged\r\n3\r\n",
        "the tracked blob must keep legacy CRLF content so the later edits exercise renormalization"
    );

    // This write is with line-endings that are unchanged from the ones on disk, and from what's in Git (CRLF).
    std::fs::write(&file_path, b"1\r\ntwo from worktree\r\nunchanged\r\n3\r\n")?;
    assert_eq!(
        git_status(&repo)?,
        " M ImportOrdersJob.cs\n",
        "the worktree edit must be visible before checkout"
    );

    let new_commit = build_commit(
        &repo,
        |tree| {
            // This commit also has the right line endings (CRLF)
            let blob_id = repo.write_blob(b"1\r\n2\r\nunchanged\r\nthree from target\r\n")?;
            tree.upsert("ImportOrdersJob.cs", EntryKind::Blob, blob_id)?;
            Ok(())
        },
        "edit same legacy crlf file independently",
    )?;

    // A lot happens here, but the significant part is that the overlapping worktree changes are cherry-picked
    // onto the `new_commit` to be transferred by merge. That snapshot now normalizes line endings correctly,
    // so the independent edits merge cleanly instead of being treated as a whole-file conflict.
    let out = safe_checkout_from_head(new_commit.id, &repo, Default::default())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    head_update: "Update refs/heads/main to Some(Object(Sha1(85b1faf797d37682f3caccdb9fb941d9acdfe325)))",
}

"#]]
    );

    assert_eq!(
        std::fs::read(&file_path)?.as_bstr(),
        "1\r\ntwo from worktree\r\nunchanged\r\nthree from target\r\n",
        "checkout keeps the worktree edit and applies the independent target change"
    );
    assert_eq!(
        repo.head_id()?,
        new_commit.id,
        "checkout updates HEAD to the target commit"
    );

    Ok(())
}

#[test]
fn checkout_handles_directory_and_file_replacements() -> anyhow::Result<()> {
    if but_testsupport::gix_testtools::is_ci::cached() {
        // TODO(gix): remove this once `gitoxide` unconditional reset/checkout is available.
        // Fails on checkout on CI Linux as it can't deal with `file`.
        // Probably the `git2` OS error code handling isn't working cross-platform?
        eprintln!("SKIPPING TEST KNOWN TO FAIL ON CI ONLY");
        return Ok(());
    }
    let (repo, _tmp) = writable_scenario("merge-with-two-branches-line-offset");
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   2a6d103 (HEAD -> merge) Merge branch 'A' into merge
|\  
| * 7f389ed (A) add 10 to the beginning
* | 91ef6f6 (B) add 10 to the end
|/  
* ff045ef (main) init

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(
        visualize_index(&*repo.index()?),
        snapbox::str![[r#"
100644:e8823e1 file

"#]]
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    // Turn file into directory
    let new_commit = build_commit(
        &repo,
        |tree| {
            let empty_blob = repo.empty_blob();
            tree.upsert("file/sub/a", EntryKind::Blob, empty_blob.id)?;
            tree.upsert("file/sub2/b", EntryKind::Blob, empty_blob.id)?;
            tree.upsert("file/c", EntryKind::Blob, empty_blob.id)?;
            Ok(())
        },
        "turn file into a directory",
    )?;
    let out = safe_checkout_from_head(new_commit.id, &repo, Default::default())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    head_update: "Update refs/heads/merge to Some(Object(Sha1(df178e3012ac0862407185ae7dd8d634a6cde677)))",
}

"#]]
    );

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* df178e3 (HEAD -> merge) turn file into a directory
*   2a6d103 Merge branch 'A' into merge
|\  
| * 7f389ed (A) add 10 to the beginning
* | 91ef6f6 (B) add 10 to the end
|/  
* ff045ef (main) init

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(
        visualize_index(&*repo.index()?),
        snapbox::str![[r#"
100644:e69de29 file/c
100644:e69de29 file/sub/a
100644:e69de29 file/sub2/b

"#]]
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    let new_commit = build_commit(
        &repo,
        |tree| {
            let empty_blob = repo.empty_blob();
            tree.upsert("file", EntryKind::Blob, empty_blob.id)?;
            Ok(())
        },
        "turn a directory back into a file",
    )?;
    let out = safe_checkout_from_head(new_commit.id, &repo, Default::default())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    head_update: "Update refs/heads/merge to Some(Object(Sha1(94cc54fa25411ad51e319a9895d031d8da97b7ab)))",
}

"#]]
    );

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 94cc54f (HEAD -> merge) turn a directory back into a file
* df178e3 turn file into a directory
*   2a6d103 Merge branch 'A' into merge
|\  
| * 7f389ed (A) add 10 to the beginning
* | 91ef6f6 (B) add 10 to the end
|/  
* ff045ef (main) init

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(
        visualize_index(&*repo.index()?),
        snapbox::str![[r#"
100644:e69de29 file

"#]]
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    Ok(())
}

#[test]
fn unrelated_additions_do_not_affect_worktree_changes() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("all-file-types-renamed-and-modified");
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 4e26689 (HEAD -> main) init

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_index(&*repo.index()?),
        snapbox::str![[r#"
100755:01e79c3 executable
100644:3aac70f file
120000:c4c364c link

"#]]
    );
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        snapbox::str![[r#"
 D executable
 D file
 D link
?? executable-renamed
?? file-renamed
?? link-renamed

"#]]
    );

    let new_commit = build_commit(
        &repo,
        |tree| {
            tree.upsert("unrelated", EntryKind::Blob, repo.empty_blob().id)?;
            Ok(())
        },
        "add unrelated file",
    )?;
    let out = safe_checkout_from_head(new_commit.id, &repo, Default::default())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    head_update: "Update refs/heads/main to Some(Object(Sha1(7add6cadcf636e5b3a6c15c75e82abbec97d6eef)))",
}

"#]]
    );

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 7add6ca (HEAD -> main) add unrelated file
* 4e26689 init

"#]]
    );
    // Only the unrelated file was added, only visible in the index.
    snapbox::assert_data_eq!(
        visualize_index(&*repo.index()?),
        snapbox::str![[r#"
100755:01e79c3 executable
100644:3aac70f file
120000:c4c364c link
100644:e69de29 unrelated

"#]]
    );

    // Deleted files stay deleted — the checkout only adds `unrelated`, which
    // doesn't intersect with the worktree deletions.
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        snapbox::str![[r#"
 D executable
 D file
 D link
?? executable-renamed
?? file-renamed
?? link-renamed

"#]]
    );
    Ok(())
}

#[test]
fn partial_commit_with_adjacent_lines_conflicts_on_checkout() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("adjacent-line-additions");
    // Worktree has two added lines (added-a, added-b) between line1 and line2.
    let file_path = repo.workdir_path("file").unwrap();
    let worktree_content = std::fs::read_to_string(&file_path)?;
    assert_eq!(worktree_content, "line1\nadded-a\nadded-b\nline2\nline3\n");

    // Simulate a partial commit: the new tree has only one of the two added lines.
    let new_commit = build_commit(
        &repo,
        |tree| {
            let blob_id = repo.write_blob(b"line1\nadded-a\nline2\nline3\n")?;
            tree.upsert("file", EntryKind::Blob, blob_id)?;
            Ok(())
        },
        "commit only one added line",
    )?;

    // The remaining worktree change (added-b) conflicts with the committed change
    // (added-a) because both add at the same position. Without a merge-base
    // override that includes the consumed changes, the 3-way merge treats this
    // as a conflict.
    let err = safe_checkout_from_head(new_commit.id, &repo, Default::default()).unwrap_err();
    assert!(
        err.to_string()
            .contains("Uncommitted files would be overwritten"),
        "checkout must abort on partial-commit conflict: {err}"
    );

    Ok(())
}

#[test]
fn partial_commit_with_deletion_plus_insertion_conflicts_on_checkout() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("adjacent-line-additions");
    // Worktree replaced old-line with new-line.
    let file_path = repo.workdir_path("file2").unwrap();
    assert_eq!(
        std::fs::read_to_string(&file_path)?,
        "line1\nnew-line\nline3\n"
    );

    // Commit only the deletion of old-line, not the insertion of new-line.
    let new_commit = build_commit(
        &repo,
        |tree| {
            let blob_id = repo.write_blob(b"line1\nline3\n")?;
            tree.upsert("file2", EntryKind::Blob, blob_id)?;
            Ok(())
        },
        "commit only the deletion",
    )?;

    // The three-way merge sees ours deleting old-line and theirs replacing it
    // with new-line — both modify the same region. Same class of bug as the
    // adjacent-line case: commit_create avoids this by skipping checkout entirely.
    let err = safe_checkout_from_head(new_commit.id, &repo, Default::default()).unwrap_err();
    assert!(
        err.to_string()
            .contains("Uncommitted files would be overwritten"),
        "checkout must abort on partial-commit conflict: {err}"
    );

    Ok(())
}

#[test]
fn consumed_changes_cancel_even_when_the_tree_does_not_change() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("adjacent-line-additions");
    let file_path = repo.workdir_path("separated").unwrap();
    let file2_path = repo.workdir_path("file2").unwrap();
    assert_eq!(
        std::fs::read_to_string(&file_path)?,
        "line1\nadded-a\nunchanged\nadded-b\nline2\nline3\n"
    );

    // Amending `added-a` into a commit outside this checkout's history leaves its
    // `HEAD` where it is, so the checkout has no tree change of its own to ride on.
    let head = repo.head_commit()?.id;
    let consumed = build_commit(
        &repo,
        |tree| {
            let blob_id = repo.write_blob(b"line1\nadded-a\nunchanged\nline2\nline3\n")?;
            tree.upsert("separated", EntryKind::Blob, blob_id)?;
            Ok(())
        },
        "HEAD^{tree} plus the consumed change",
    )?
    .tree_id()?
    .detach();

    safe_checkout_from_head(
        head,
        &repo,
        checkout::Options {
            merge_base_override: Some(consumed),
            ..Default::default()
        },
    )?;

    assert_eq!(
        std::fs::read_to_string(&file_path)?,
        "line1\nunchanged\nadded-b\nline2\nline3\n",
        "the consumed line is gone - it lives in the commit now - and the one that \
         wasn't consumed stays behind"
    );
    assert_eq!(
        std::fs::read_to_string(&file2_path)?,
        "line1\nnew-line\nline3\n",
        "dirt in files the override doesn't mention is untouched"
    );
    Ok(())
}

#[test]
fn cancelling_a_consumed_addition_removes_it_and_leaves_other_dirt_alone() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("adjacent-line-additions");
    let added_path = repo.workdir_path("added.txt").unwrap();
    std::fs::write(&added_path, "added\n")?;

    let head = repo.head_commit()?.id;
    let consumed = build_commit(
        &repo,
        |tree| {
            let blob_id = repo.write_blob(b"added\n")?;
            tree.upsert("added.txt", EntryKind::Blob, blob_id)?;
            Ok(())
        },
        "HEAD^{tree} plus the consumed addition",
    )?
    .tree_id()?
    .detach();

    safe_checkout_from_head(
        head,
        &repo,
        checkout::Options {
            merge_base_override: Some(consumed),
            ..Default::default()
        },
    )?;

    assert!(
        !added_path.exists(),
        "the file lives in a commit now, so it must not linger here as an untracked duplicate"
    );
    assert_eq!(
        std::fs::read_to_string(repo.workdir_path("file").unwrap())?,
        "line1\nadded-a\nadded-b\nline2\nline3\n",
        "removing the consumed addition must not let the checkout loose on unrelated dirt"
    );
    assert_eq!(
        std::fs::read_to_string(repo.workdir_path("file2").unwrap())?,
        "line1\nnew-line\nline3\n"
    );
    Ok(())
}

#[test]
fn cancelling_consumed_changes_keeps_a_concurrent_edit() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("adjacent-line-additions");
    let file_path = repo.workdir_path("separated").unwrap();
    let head = repo.head_commit()?.id;
    let consumed = build_commit(
        &repo,
        |tree| {
            let blob_id = repo.write_blob(b"line1\nadded-a\nunchanged\nline2\nline3\n")?;
            tree.upsert("separated", EntryKind::Blob, blob_id)?;
            Ok(())
        },
        "HEAD^{tree} plus the consumed change",
    )?
    .tree_id()?
    .detach();

    // Someone writes to the file between computing the override and checking out.
    std::fs::write(
        &file_path,
        "line1\nadded-a\nunchanged\nadded-b\nline2\nline3\nappended\n",
    )?;

    safe_checkout_from_head(
        head,
        &repo,
        checkout::Options {
            merge_base_override: Some(consumed),
            ..Default::default()
        },
    )?;

    assert_eq!(
        std::fs::read_to_string(&file_path)?,
        "line1\nunchanged\nadded-b\nline2\nline3\nappended\n",
        "the snapshot is taken live, so the concurrent edit survives the cancellation"
    );
    Ok(())
}

#[test]
fn skipping_index_update_materializes_consumption_without_writing_staging() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("adjacent-line-additions");
    let path = repo.workdir_path("separated").unwrap();
    let consumed = build_commit(
        &repo,
        |tree| {
            tree.upsert(
                "separated",
                EntryKind::Blob,
                repo.write_blob(b"line1\nadded-a\nunchanged\nline2\nline3\n")?,
            )?;
            Ok(())
        },
        "consume added-a",
    )?
    .tree_id()?
    .detach();
    let index_before = std::fs::read(repo.index_path())?;

    safe_checkout_from_head(
        repo.head_id()?.detach(),
        &repo,
        checkout::Options {
            merge_base_override: Some(consumed),
            skip_index_update: true,
            ..Default::default()
        },
    )?;

    assert_eq!(
        std::fs::read_to_string(path)?,
        "line1\nunchanged\nadded-b\nline2\nline3\n",
        "consumed changes are removed from the worktree"
    );
    assert_eq!(
        std::fs::read(repo.index_path())?,
        index_before,
        "the temporary index is never persisted"
    );
    Ok(())
}

mod utils {}
