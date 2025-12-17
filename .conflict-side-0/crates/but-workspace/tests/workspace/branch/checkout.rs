use but_testsupport::{git_status, visualize_commit_graph_all, visualize_disk_tree_skip_dot_git};
use but_workspace::branch::{checkout, checkout::UncommitedWorktreeChanges, safe_checkout};
use gix::object::tree::EntryKind;

use crate::{
    branch::checkout::utils::build_commit,
    utils::{
        read_only_in_memory_scenario, visualize_index, writable_scenario, writable_scenario_slow,
    },
};

#[test]
fn update_unborn_head() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("unborn-empty");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"");
    insta::assert_snapshot!(git_status(&repo)?, @r"");

    let empty_tree = repo.empty_tree().id;
    let head_commit = repo.new_commit("init", empty_tree, None::<gix::ObjectId>)?;

    let out = safe_checkout(empty_tree, head_commit.id, &repo, Default::default())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        snapshot_tree: None,
        num_deleted_files: 0,
        num_added_or_updated_files: 0,
        head_update: "Update refs/heads/main to Some(Object(Sha1(36d9c8013ccd91e3a1d53a3bc86c12ca81cc4a11)))",
    }
    "#);

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 36d9c80 (HEAD -> main) init");
    insta::assert_snapshot!(git_status(&repo)?, @r"");
    Ok(())
}

#[test]
fn no_op_trees_never_touch_worktree() -> anyhow::Result<()> {
    let repo = read_only_in_memory_scenario("all-file-types-renamed-and-modified")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 4e26689 (HEAD -> main) init");
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @r"
    100755:01e79c3 executable
    100644:3aac70f file
    120000:c4c364c link
    ");
    insta::assert_snapshot!(git_status(&repo)?, @r"
     D executable
     D file
     D link
    ?? executable-renamed
    ?? file-renamed
    ?? link-renamed
    ");

    let a_commit = repo.head_commit()?;
    let a_tree = a_commit.tree_id()?.detach();

    let out = safe_checkout(a_tree, a_commit.id, &repo, Default::default())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        snapshot_tree: None,
        num_deleted_files: 0,
        num_added_or_updated_files: 0,
        head_update: "None",
    }
    "#);

    // Nothing changed
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 4e26689 (HEAD -> main) init");
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @r"
    100755:01e79c3 executable
    100644:3aac70f file
    120000:c4c364c link
    ");
    insta::assert_snapshot!(git_status(&repo)?, @r"
     D executable
     D file
     D link
    ?? executable-renamed
    ?? file-renamed
    ?? link-renamed
    ");
    Ok(())
}

#[test]
fn worktree_and_index_deletions_are_ignored_in_snapshots() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("deletion-addition-untracked");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 226d5ea (HEAD -> main) init");
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @r"
    100644:3e75765 added-to-index
    100644:d95f3ad to-be-deleted
    ");
    insta::assert_snapshot!(git_status(&repo)?, @r"
    A  added-to-index
     D to-be-deleted
    D  to-be-deleted-in-index
    ?? untracked
    ");

    // Turn deleted files into directory - these won't conflict no matter what they were in the index.
    let (head_commit, new_commit) = build_commit(
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

    let out = safe_checkout(head_commit.id, new_commit.id, &repo, Default::default())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        snapshot_tree: None,
        num_deleted_files: 1,
        num_added_or_updated_files: 1,
        head_update: "Update refs/heads/main to Some(Object(Sha1(24f802a1250d2f84e1f49094e3b8bb1e5c0d29ad)))",
    }
    "#);

    // Nothing changed as the checkout was aborted.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 24f802a (HEAD -> main) turn changed file into a directory
    * 226d5ea init
    ");
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @r"
    100644:3e75765 added-to-index
    100644:637f034 to-be-deleted-in-index
    100644:e69de29 to-be-deleted/a
    ");
    insta::assert_snapshot!(git_status(&repo)?, @r"
    A  added-to-index
    ?? untracked
    ");

    Ok(())
}

#[test]
fn worktree_changes_do_not_cause_conflict_markers_but_fail() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("mixed-hunk-modifications");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 647cc94 (HEAD -> main) init");
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @r"
    100755:3d3b36f file
    100755:cb89473 file-in-index
    100644:3d3b36f file-renamed-in-index
    100644:3d3b36f file-to-be-renamed
    ");
    insta::assert_snapshot!(git_status(&repo)?, @r"
     M file
    M  file-in-index
    RM file-to-be-renamed-in-index -> file-renamed-in-index
     D file-to-be-renamed
    ?? file-renamed
    ");
    let file_path = repo.workdir_path("file").unwrap();
    let actual = std::fs::read_to_string(&file_path)?;
    insta::assert_debug_snapshot!(actual, @r#""1\n2\n3\n4\n5\n6-7\n8\n9\nten\neleven\n12\n20\n21\n22\n15\n16\n""#);

    // In the target tree, make a surgical edit (one changed line) so the changes should still apply cleany
    let (head_commit, new_commit) = build_commit(
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

    let err = safe_checkout(head_commit.id, new_commit.id, &repo, Default::default()).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Worktree changes would be overwritten by checkout: \"file\"",
        "we check for conflict markers, and fail."
    );
    // Nothing else changes
    let actual = std::fs::read_to_string(&file_path)?;
    insta::assert_debug_snapshot!(actual, @r#""1\n2\n3\n4\n5\n6-7\n8\n9\nten\neleven\n12\n20\n21\n22\n15\n16\n""#);
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 647cc94 (HEAD -> main) init");
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @r"
    100755:3d3b36f file
    100755:cb89473 file-in-index
    100644:3d3b36f file-renamed-in-index
    100644:3d3b36f file-to-be-renamed
    ");

    insta::assert_snapshot!(git_status(&repo)?, @r"
     M file
    M  file-in-index
    RM file-to-be-renamed-in-index -> file-renamed-in-index
     D file-to-be-renamed
    ?? file-renamed
    ");

    Ok(())
}

#[test]
fn worktree_snapshot_reapplies_with_hunk_granularity() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("mixed-hunk-modifications");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 647cc94 (HEAD -> main) init");
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @r"
    100755:3d3b36f file
    100755:cb89473 file-in-index
    100644:3d3b36f file-renamed-in-index
    100644:3d3b36f file-to-be-renamed
    ");
    insta::assert_snapshot!(git_status(&repo)?, @r"
     M file
    M  file-in-index
    RM file-to-be-renamed-in-index -> file-renamed-in-index
     D file-to-be-renamed
    ?? file-renamed
    ");
    let file_path = repo.workdir_path("file").unwrap();
    let actual = std::fs::read_to_string(&file_path)?;
    insta::assert_snapshot!(actual, @r"
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
    ");

    // In the target tree, make a surgical edit (one changed line) so the changes should still apply cleany
    let (head_commit, new_commit) = build_commit(
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

    let out = safe_checkout(head_commit.id, new_commit.id, &repo, Default::default())
        .expect("no error as we keep the snapshot for later");
    // File is still changed, after all we re-applied the worktree changes.
    let actual = std::fs::read_to_string(&file_path)?;
    insta::assert_snapshot!(actual, @r"
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
    ");

    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        snapshot_tree: Some(
            Sha1(76de10879a78339980d6a33ecfd6f2f711960106),
        ),
        num_deleted_files: 0,
        num_added_or_updated_files: 1,
        head_update: "Update refs/heads/main to Some(Object(Sha1(89b113aeae66a3cb1116bb23a195422edbd6af27)))",
    }
    "#);
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 89b113a (HEAD -> main) edited 'file' (add single line)
    * 647cc94 init
    ");
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @r"
    100644:832f532 file
    100755:cb89473 file-in-index
    100644:3d3b36f file-renamed-in-index
    100644:3d3b36f file-to-be-renamed
    100644:3d3b36f file-to-be-renamed-in-index
    ");
    // Notably, 'file' is not in the index anymore, as that now always matches the worktree.
    insta::assert_snapshot!(git_status(&repo)?, @r"
    M  file
    M  file-in-index
    AM file-renamed-in-index
    ?? file-renamed
    ");

    Ok(())
}

#[test]
fn snapshot_fails_by_default_if_changed_file_turns_into_directory() -> anyhow::Result<()> {
    if but_testsupport::gix_testtools::is_ci::cached() {
        // Fails on checkout on Linux as it can't deal with `file`.
        // Probably the `git2` OS error code handling isn't working cross-platform?
        eprintln!("SKIPPING TEST KNOWN TO FAIL ON CI ONLY");
        return Ok(());
    }
    let (repo, _tmp) = writable_scenario("mixed-hunk-modifications");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 647cc94 (HEAD -> main) init");
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @r"
    100755:3d3b36f file
    100755:cb89473 file-in-index
    100644:3d3b36f file-renamed-in-index
    100644:3d3b36f file-to-be-renamed
    ");
    insta::assert_snapshot!(git_status(&repo)?, @r"
     M file
    M  file-in-index
    RM file-to-be-renamed-in-index -> file-renamed-in-index
     D file-to-be-renamed
    ?? file-renamed
    ");

    // Turn changed file into directory - conflict as snapshot won't apply cleanly.
    let (head_commit, new_commit) = build_commit(
        &repo,
        |tree| {
            tree.upsert("file/a", EntryKind::Blob, repo.empty_blob().id)?;
            tree.upsert("file-in-index/a", EntryKind::Blob, repo.empty_blob().id)?;
            Ok(())
        },
        "turn changed files into a directories",
    )?;

    let err = safe_checkout(head_commit.id, new_commit.id, &repo, Default::default()).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Worktree changes would be overwritten by checkout: \"file\", \"file-in-index\"",
        "conflicting worktree changes prevent a commit"
    );

    // Nothing changed as the checkout was aborted.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 647cc94 (HEAD -> main) init");
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @r"
    100755:3d3b36f file
    100755:cb89473 file-in-index
    100644:3d3b36f file-renamed-in-index
    100644:3d3b36f file-to-be-renamed
    ");
    insta::assert_snapshot!(git_status(&repo)?, @r"
     M file
    M  file-in-index
    RM file-to-be-renamed-in-index -> file-renamed-in-index
     D file-to-be-renamed
    ?? file-renamed
    ");

    let out = safe_checkout(head_commit.id, new_commit.id, &repo, overwrite_options())
        .expect("no error as we keep the snapshot for later");
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        snapshot_tree: Some(
            Sha1(ec1460cd4e8cf13a94e6248c42363f7ba869724b),
        ),
        num_deleted_files: 2,
        num_added_or_updated_files: 2,
        head_update: "Update refs/heads/main to Some(Object(Sha1(434b90855459c3a7421a7c8b32b3423e6eafe107)))",
    }
    "#);
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 434b908 (HEAD -> main) turn changed files into a directories
    * 647cc94 init
    ");
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @r"
    100644:e69de29 file-in-index/a
    100644:3d3b36f file-renamed-in-index
    100644:3d3b36f file-to-be-renamed
    100644:3d3b36f file-to-be-renamed-in-index
    100644:e69de29 file/a
    ");
    // Note how the deleted file (which is in the destination tree) was restored, because we are additive,
    // and can't differentiate between missing files and deleted files.
    insta::assert_snapshot!(git_status(&repo)?, @r"
    AM file-renamed-in-index
    ?? file-renamed
    ");

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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   2a6d103 (HEAD -> merge) Merge branch 'A' into merge
    |\  
    | * 7f389ed (A) add 10 to the beginning
    * | 91ef6f6 (B) add 10 to the end
    |/  
    * ff045ef (main) init
    ");
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @"100644:e8823e1 file");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    // Turn file into directory
    let (head_commit, new_commit) = build_commit(
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
    let out = safe_checkout(head_commit.id, new_commit.id, &repo, Default::default())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        snapshot_tree: None,
        num_deleted_files: 1,
        num_added_or_updated_files: 3,
        head_update: "Update refs/heads/merge to Some(Object(Sha1(df178e3012ac0862407185ae7dd8d634a6cde677)))",
    }
    "#);

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * df178e3 (HEAD -> merge) turn file into a directory
    *   2a6d103 Merge branch 'A' into merge
    |\  
    | * 7f389ed (A) add 10 to the beginning
    * | 91ef6f6 (B) add 10 to the end
    |/  
    * ff045ef (main) init
    ");
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @r"
    100644:e69de29 file/c
    100644:e69de29 file/sub/a
    100644:e69de29 file/sub2/b
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    let (head_commit, new_commit) = build_commit(
        &repo,
        |tree| {
            let empty_blob = repo.empty_blob();
            tree.upsert("file", EntryKind::Blob, empty_blob.id)?;
            Ok(())
        },
        "turn a directory back into a file",
    )?;
    let out = safe_checkout(head_commit.id, new_commit.id, &repo, Default::default())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        snapshot_tree: None,
        num_deleted_files: 3,
        num_added_or_updated_files: 1,
        head_update: "Update refs/heads/merge to Some(Object(Sha1(94cc54fa25411ad51e319a9895d031d8da97b7ab)))",
    }
    "#);

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 94cc54f (HEAD -> merge) turn a directory back into a file
    * df178e3 turn file into a directory
    *   2a6d103 Merge branch 'A' into merge
    |\  
    | * 7f389ed (A) add 10 to the beginning
    * | 91ef6f6 (B) add 10 to the end
    |/  
    * ff045ef (main) init
    ");
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @"100644:e69de29 file");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    Ok(())
}

#[test]
fn unrelated_additions_are_fine_even_with_conflicts_in_index() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("merge-with-two-branches-conflict");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 88d7acc (A) 10 to 20
    | * 47334c6 (HEAD -> merge, B) 20 to 30
    |/  
    * 15bcd1b (main) init
    ");
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @r"
    100644:e69de29 file:1
    100644:e6c4914 file:2
    100644:e33f5e9 file:3
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"UU file");

    let (head_commit, new_commit) = build_commit(
        &repo,
        |tree| {
            tree.upsert("unrelated", EntryKind::Blob, repo.empty_blob().id)?;
            Ok(())
        },
        "add unrelated file",
    )?;
    let out = safe_checkout(head_commit.id, new_commit.id, &repo, Default::default())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        snapshot_tree: None,
        num_deleted_files: 0,
        num_added_or_updated_files: 1,
        head_update: "Update refs/heads/merge to Some(Object(Sha1(a7f60850de59562526c0f31331d47903a78d1d43)))",
    }
    "#);

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 88d7acc (A) 10 to 20
    | * a7f6085 (HEAD -> merge) add unrelated file
    | * 47334c6 (B) 20 to 30
    |/  
    * 15bcd1b (main) init
    ");
    // Only the unrelated file was added, only visible in the index.
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @r"
    100644:e69de29 file:1
    100644:e6c4914 file:2
    100644:e33f5e9 file:3
    100644:e69de29 unrelated
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"UU file");

    // Edit the file that is conflicting
    let (head_commit, new_commit) = build_commit(
        &repo,
        |tree| {
            tree.upsert("file", EntryKind::Blob, repo.empty_blob().id)?;
            Ok(())
        },
        "overwrite conflicting file",
    )?;

    let err = safe_checkout(head_commit.id, new_commit.id, &repo, Default::default()).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Worktree changes would be overwritten by checkout: \"file\"",
        "We don't allow to checkout conflicting files with default settings as there is no snapshot"
    );

    // Nothing was changed
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 88d7acc (A) 10 to 20
    | * a7f6085 (HEAD -> merge) add unrelated file
    | * 47334c6 (B) 20 to 30
    |/  
    * 15bcd1b (main) init
    ");
    // The worktree is unaltered.
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @r"
    100644:e69de29 file:1
    100644:e6c4914 file:2
    100644:e33f5e9 file:3
    100644:e69de29 unrelated
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"UU file");

    // We can force the conflict to be overwritten.
    let out = safe_checkout(head_commit.id, new_commit.id, &repo, overwrite_options())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        snapshot_tree: Some(
            Sha1(d4506ed0f2312971e38ebae18705b373fbec1e5a),
        ),
        num_deleted_files: 0,
        num_added_or_updated_files: 1,
        head_update: "Update refs/heads/merge to Some(Object(Sha1(247c3b8fe4a1a270ba099a88ee7d8ab37721d5a1)))",
    }
    "#);
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 88d7acc (A) 10 to 20
    | * 247c3b8 (HEAD -> merge) overwrite conflicting file
    | * a7f6085 add unrelated file
    | * 47334c6 (B) 20 to 30
    |/  
    * 15bcd1b (main) init
    ");
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @r"
    100644:e82faaf file
    100644:e69de29 unrelated
    ");
    Ok(())
}

#[test]
fn forced_changes_with_snapshot_and_directory_to_file() -> anyhow::Result<()> {
    if but_testsupport::gix_testtools::is_ci::cached() {
        // Fails on checkout on Linux as it tries to get null from the ODB for some reason.
        // Too strange, usually related to the index somehow.
        eprintln!("SKIPPING TEST KNOWN TO FAIL ON CI ONLY");
        return Ok(());
    }
    let (repo, _tmp) = writable_scenario_slow("all-file-types-renamed-and-overwriting-existing");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* af77f7c (HEAD -> main) init");
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @r"
    100644:e69de29 dir-to-be-file/content
    100755:01e79c3 executable
    100644:3aac70f file
    100644:e69de29 file-to-be-dir
    120000:c4c364c link
    100644:dcefb7d other-file
    100644:e69de29 to-be-overwritten
    ");
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
    .
    ├── .git:40755
    ├── dir-to-be-file:100755
    ├── file-to-be-dir:40755
    │   └── file:100644
    ├── link-renamed:120755
    └── to-be-overwritten:100644
    ");
    insta::assert_snapshot!(git_status(&repo)?, @r"
     D dir-to-be-file/content
     D executable
     D file
     D file-to-be-dir
     D link
     D other-file
     M to-be-overwritten
    ?? dir-to-be-file
    ?? link-renamed
    ");

    let (head_commit, new_commit) = build_commit(
        &repo,
        |tree| {
            tree.upsert("dir-to-be-file", EntryKind::Blob, repo.empty_blob().id)?;
            tree.upsert("file-to-be-dir/b/a", EntryKind::Blob, repo.empty_blob().id)?;
            Ok(())
        },
        "dir to file and file to dir",
    )?;
    let out = safe_checkout(head_commit.id, new_commit.id, &repo, overwrite_options())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        snapshot_tree: Some(
            Sha1(7b5d75d4a661ba159bf04cd222ec65e12d0c29ca),
        ),
        num_deleted_files: 2,
        num_added_or_updated_files: 2,
        head_update: "Update refs/heads/main to Some(Object(Sha1(ace716c5fae006fe5c7057017bafbdadf1e2fcbb)))",
    }
    "#);

    // TODO: use `gix` to also checkout 'dir-to-be-file', for some reason `git2` doesn't check it out
    //       even though it's given and it's part of the tree.
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
    .
    ├── .git:40755
    ├── executable:100755
    ├── file:100644
    ├── file-to-be-dir:40755
    │   ├── b:40755
    │   │   └── a:100644
    │   └── file:100644
    ├── link:120755
    ├── link-renamed:120755
    ├── other-file:100644
    └── to-be-overwritten:100644
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * ace716c (HEAD -> main) dir to file and file to dir
    * af77f7c init
    ");
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @r"
    100755:01e79c3 executable
    100644:3aac70f file
    100644:e69de29 file-to-be-dir/b/a
    100644:66f816c file-to-be-dir/file
    120000:c4c364c link
    100644:dcefb7d other-file
    100644:e69de29 to-be-overwritten
    ");
    insta::assert_snapshot!(git_status(&repo)?, @r"
    D  dir-to-be-file
    A  file-to-be-dir/file
     M to-be-overwritten
    ?? link-renamed
    ");

    // To empty tree.
    let out = safe_checkout(
        repo.head_id()?.detach(),
        repo.empty_tree().id,
        &repo,
        overwrite_options(),
    )?;
    // We are able to check out to an empty tree if needed, keeping all changes everything else in a stash
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        snapshot_tree: Some(
            Sha1(e2cf369ffdb86eeedb5254be25389f0873e87607),
        ),
        num_deleted_files: 7,
        num_added_or_updated_files: 0,
        head_update: "None",
    }
    "#);
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
    .
    ├── .git:40755
    ├── file-to-be-dir:40755
    │   └── file:100644
    └── link-renamed:120755
    ");
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @r"
    100755:01e79c3 executable
    100644:3aac70f file
    100644:e69de29 file-to-be-dir/b/a
    100644:66f816c file-to-be-dir/file
    120000:c4c364c link
    100644:dcefb7d other-file
    100644:e69de29 to-be-overwritten
    ");
    Ok(())
}

#[test]
fn unrelated_additions_do_not_affect_worktree_changes() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("all-file-types-renamed-and-modified");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 4e26689 (HEAD -> main) init");
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @r"
    100755:01e79c3 executable
    100644:3aac70f file
    120000:c4c364c link
    ");
    insta::assert_snapshot!(git_status(&repo)?, @r"
     D executable
     D file
     D link
    ?? executable-renamed
    ?? file-renamed
    ?? link-renamed
    ");

    let (head_commit, new_commit) = build_commit(
        &repo,
        |tree| {
            tree.upsert("unrelated", EntryKind::Blob, repo.empty_blob().id)?;
            Ok(())
        },
        "add unrelated file",
    )?;
    let out = safe_checkout(head_commit.id, new_commit.id, &repo, Default::default())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        snapshot_tree: None,
        num_deleted_files: 0,
        num_added_or_updated_files: 1,
        head_update: "Update refs/heads/main to Some(Object(Sha1(7add6cadcf636e5b3a6c15c75e82abbec97d6eef)))",
    }
    "#);

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 7add6ca (HEAD -> main) add unrelated file
    * 4e26689 init
    ");
    // Only the unrelated file was added, only visible in the index.
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @r"
    100755:01e79c3 executable
    100644:3aac70f file
    120000:c4c364c link
    100644:e69de29 unrelated
    ");

    // It also restored the deleted files, after all they are part of the tree.
    insta::assert_snapshot!(git_status(&repo)?, @r"
    ?? executable-renamed
    ?? file-renamed
    ?? link-renamed
    ");
    Ok(())
}

fn overwrite_options() -> checkout::Options {
    checkout::Options {
        uncommitted_changes: UncommitedWorktreeChanges::KeepConflictingInSnapshotAndOverwrite,
        skip_head_update: false,
    }
}

mod utils {
    /// Using the `repo` `HEAD` commit, build a new commit based on its tree with `edit` and `message`, and return the `(current_commit, new_commit)`.
    pub fn build_commit<'repo>(
        repo: &'repo gix::Repository,
        mut edit: impl FnMut(&mut gix::object::tree::Editor) -> anyhow::Result<()>,
        message: &str,
    ) -> anyhow::Result<(gix::Commit<'repo>, gix::Commit<'repo>)> {
        let head_commit = repo.head_commit()?;

        repo.write_blob([])?;
        let mut editor = head_commit.tree()?.edit()?;
        edit(&mut editor)?;

        let new_commit_id = repo
            .write_object(gix::objs::Commit {
                tree: editor.write()?.detach(),
                parents: [head_commit.id].into(),
                message: message.into(),
                ..head_commit.decode()?.to_owned()
            })?
            .detach();
        Ok((head_commit, repo.find_commit(new_commit_id)?))
    }
}
