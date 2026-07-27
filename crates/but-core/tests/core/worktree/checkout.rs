use bstr::ByteSlice;
use but_core::worktree::{checkout, prepare_safe_checkout_from_head, safe_checkout_from_head};
use but_testsupport::{
    CommandExt, git_at_dir, git_status, open_repo, read_only_in_memory_scenario,
    visualize_commit_graph_all, visualize_disk_tree_skip_dot_git, visualize_index,
    visualize_index_with_content, writable_scenario, writable_scenario_slow,
};
use gix::object::tree::EntryKind;
use snapbox::prelude::*;

use crate::worktree::utils::build_commit;

struct LinkedWorktreeTestContext {
    main: gix::Repository,
    linked_a: gix::Repository,
    linked_b: gix::Repository,
    _tmp: but_testsupport::gix_testtools::tempfile::TempDir,
}

fn linked_worktree_test_context() -> anyhow::Result<LinkedWorktreeTestContext> {
    let root = but_testsupport::gix_testtools::tempfile::TempDir::new()?;
    let main_path = root.path().join("main");
    let linked_a_path = root.path().join("linked-a");
    let linked_b_path = root.path().join("linked-b");
    gix::init(&main_path)?;
    let main_repo = open_repo(&main_path)?;

    let old_blob = main_repo.write_blob(b"old\n")?;
    let unrelated_blob = main_repo.write_blob(b"base\n")?;
    let mut tree = main_repo.empty_tree().edit()?;
    tree.upsert("tracked.txt", EntryKind::Blob, old_blob)?;
    tree.upsert("unrelated.txt", EntryKind::Blob, unrelated_blob)?;
    let initial = main_repo.new_commit("initial", tree.write()?.detach(), None::<gix::ObjectId>)?;
    let initial_id = initial.id;
    drop(initial);
    safe_checkout_from_head(initial_id, &main_repo, Default::default())?;

    git_at_dir(&main_path)
        .args(["worktree", "add", "-b", "linked-a"])
        .arg(&linked_a_path)
        .run();
    git_at_dir(&main_path)
        .args(["worktree", "add", "-b", "linked-b"])
        .arg(&linked_b_path)
        .run();
    let linked_a_repo = open_repo(&linked_a_path)?;
    let linked_b_repo = open_repo(&linked_b_path)?;
    Ok(LinkedWorktreeTestContext {
        main: main_repo,
        linked_a: linked_a_repo,
        linked_b: linked_b_repo,
        _tmp: root,
    })
}

fn visualize_linked_worktree(repo: &gix::Repository) -> anyhow::Result<String> {
    let workdir = repo.workdir().expect("non-bare repository");
    let tracked = std::fs::read_to_string(workdir.join("tracked.txt"))?;
    let unrelated = std::fs::read_to_string(workdir.join("unrelated.txt"))?;
    Ok(format!(
        "HEAD\n{}INDEX\n{}WORKTREE\n{}FILES\ntracked.txt: {tracked:?}\nunrelated.txt: {unrelated:?}\nSTATUS\n{}",
        std::fs::read_to_string(repo.git_dir().join("HEAD"))?,
        visualize_index_with_content(repo, &**repo.index()?),
        visualize_disk_tree_skip_dot_git(workdir)?,
        git_status(repo)?
    ))
}

#[test]
fn prepared_checkouts_use_the_old_linked_worktree_heads_after_their_refs_move() -> anyhow::Result<()>
{
    let ctx = linked_worktree_test_context()?;
    let target_blob = ctx.main.write_blob(b"new\n")?;
    let target = build_commit(
        &ctx.main,
        |tree| {
            tree.upsert("tracked.txt", EntryKind::Blob, target_blob)?;
            Ok(())
        },
        "update tracked file",
    )?;
    std::fs::write(
        ctx.linked_a
            .workdir_path("unrelated.txt")
            .expect("non-bare repository"),
        "dirty\n",
    )?;

    let prepared_a = prepare_safe_checkout_from_head(
        target.id,
        &ctx.linked_a,
        checkout::Options {
            skip_head_update: true,
            ..Default::default()
        },
    )?;
    let prepared_b = prepare_safe_checkout_from_head(
        target.id,
        &ctx.linked_b,
        checkout::Options {
            skip_head_update: true,
            ..Default::default()
        },
    )?;
    git_at_dir(ctx.main.workdir().expect("non-bare repository"))
        .args(["update-ref", "refs/heads/linked-a", &target.id.to_string()])
        .run();
    git_at_dir(ctx.main.workdir().expect("non-bare repository"))
        .args(["update-ref", "refs/heads/linked-b", &target.id.to_string()])
        .run();

    let graph = visualize_commit_graph_all(&ctx.main)?;
    // Both shared branch refs have moved to the target commit.
    snapbox::assert_data_eq!(
        graph.as_str(),
        snapbox::str![[r#"
* c2aefa0 (linked-b, linked-a) update tracked file
* 5e6f558 (HEAD -> main) initial

"#]]
    );
    // Linked A still has its old index and unrelated worktree change.
    snapbox::assert_data_eq!(
        visualize_linked_worktree(&ctx.linked_a)?,
        snapbox::str![[r#"
HEAD
ref: refs/heads/linked-a
INDEX
100644:3367afd tracked.txt "old\n"
100644:df967b9 unrelated.txt "base\n"
WORKTREE
.
├── .git:100644
├── tracked.txt:100644
└── unrelated.txt:100644
FILES
tracked.txt: "old\n"
unrelated.txt: "dirty\n"
STATUS
M  tracked.txt
 M unrelated.txt

"#]]
        .raw()
    );
    // Linked B still has its old index and clean worktree.
    snapbox::assert_data_eq!(
        visualize_linked_worktree(&ctx.linked_b)?,
        snapbox::str![[r#"
HEAD
ref: refs/heads/linked-b
INDEX
100644:3367afd tracked.txt "old\n"
100644:df967b9 unrelated.txt "base\n"
WORKTREE
.
├── .git:100644
├── tracked.txt:100644
└── unrelated.txt:100644
FILES
tracked.txt: "old\n"
unrelated.txt: "base\n"
STATUS
M  tracked.txt

"#]]
        .raw()
    );

    let outcome_a = prepared_a.execute()?;
    let outcome_b = prepared_b.execute()?;

    // Executing A updates one file without updating HEAD.
    snapbox::assert_data_eq!(
        outcome_a.to_debug(),
        snapbox::str![[r#"
Outcome {
    head_update: "None",
}

"#]]
    );
    // Executing B updates one file without updating HEAD.
    snapbox::assert_data_eq!(
        outcome_b.to_debug(),
        snapbox::str![[r#"
Outcome {
    head_update: "None",
}

"#]]
    );

    // Execution does not move the shared refs again.
    snapbox::assert_data_eq!(visualize_commit_graph_all(&ctx.main)?, graph.raw());
    // Linked A now has the target content while preserving its unrelated dirt.
    snapbox::assert_data_eq!(
        visualize_linked_worktree(&ctx.linked_a)?,
        snapbox::str![[r#"
HEAD
ref: refs/heads/linked-a
INDEX
100644:3e75765 tracked.txt "new\n"
100644:df967b9 unrelated.txt "base\n"
WORKTREE
.
├── .git:100644
├── tracked.txt:100644
└── unrelated.txt:100644
FILES
tracked.txt: "new\n"
unrelated.txt: "dirty\n"
STATUS
 M unrelated.txt

"#]]
        .raw()
    );
    // Linked B now has the target content and remains clean.
    snapbox::assert_data_eq!(
        visualize_linked_worktree(&ctx.linked_b)?,
        snapbox::str![[r#"
HEAD
ref: refs/heads/linked-b
INDEX
100644:3e75765 tracked.txt "new\n"
100644:df967b9 unrelated.txt "base\n"
WORKTREE
.
├── .git:100644
├── tracked.txt:100644
└── unrelated.txt:100644
FILES
tracked.txt: "new\n"
unrelated.txt: "base\n"
STATUS

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn conflicting_linked_worktree_checkout_fails_during_preparation_without_mutation()
-> anyhow::Result<()> {
    let ctx = linked_worktree_test_context()?;
    let target_blob = ctx.main.write_blob(b"target\n")?;
    let target = build_commit(
        &ctx.main,
        |tree| {
            tree.upsert("tracked.txt", EntryKind::Blob, target_blob)?;
            Ok(())
        },
        "conflicting target",
    )?;
    let conflicting_tracked_path = ctx
        .linked_b
        .workdir_path("tracked.txt")
        .expect("non-bare repository");
    std::fs::write(&conflicting_tracked_path, "local\n")?;

    let graph_before = visualize_commit_graph_all(&ctx.main)?;
    // All refs still point to the initial commit.
    snapbox::assert_data_eq!(
        graph_before.as_str(),
        snapbox::str![[r#"
* 5e6f558 (HEAD -> main, linked-b, linked-a) initial

"#]]
    );
    let linked_a_before = visualize_linked_worktree(&ctx.linked_a)?;
    // Linked A starts clean.
    snapbox::assert_data_eq!(
        linked_a_before.as_str(),
        snapbox::str![[r#"
HEAD
ref: refs/heads/linked-a
INDEX
100644:3367afd tracked.txt "old\n"
100644:df967b9 unrelated.txt "base\n"
WORKTREE
.
├── .git:100644
├── tracked.txt:100644
└── unrelated.txt:100644
FILES
tracked.txt: "old\n"
unrelated.txt: "base\n"
STATUS

"#]]
        .raw()
    );
    let linked_b_before = visualize_linked_worktree(&ctx.linked_b)?;
    // Linked B starts with the conflicting local edit.
    snapbox::assert_data_eq!(
        linked_b_before.as_str(),
        snapbox::str![[r#"
HEAD
ref: refs/heads/linked-b
INDEX
100644:3367afd tracked.txt "old\n"
100644:df967b9 unrelated.txt "base\n"
WORKTREE
.
├── .git:100644
├── tracked.txt:100644
└── unrelated.txt:100644
FILES
tracked.txt: "local\n"
unrelated.txt: "base\n"
STATUS
 M tracked.txt

"#]]
        .raw()
    );

    let prepared = prepare_safe_checkout_from_head(
        target.id,
        &ctx.linked_a,
        checkout::Options {
            skip_head_update: true,
            ..Default::default()
        },
    )?;
    let err = match prepare_safe_checkout_from_head(
        target.id,
        &ctx.linked_b,
        checkout::Options {
            skip_head_update: true,
            ..Default::default()
        },
    ) {
        Ok(_) => panic!("conflicting worktree changes must fail during preparation"),
        Err(err) => err,
    };

    // The conflicting edit is rejected during preparation.
    snapbox::assert_data_eq!(
        err.to_string(),
        snapbox::str![[r#"
Uncommitted files would be overwritten by checkout: "tracked.txt"
"#]]
    );
    // Failed preparation does not move any refs.
    snapbox::assert_data_eq!(visualize_commit_graph_all(&ctx.main)?, graph_before.raw());
    // Prepared but unexecuted A remains unchanged.
    snapbox::assert_data_eq!(
        visualize_linked_worktree(&ctx.linked_a)?,
        linked_a_before.raw()
    );
    // Failed preparation leaves B unchanged.
    snapbox::assert_data_eq!(
        visualize_linked_worktree(&ctx.linked_b)?,
        linked_b_before.raw()
    );
    drop(prepared);
    Ok(())
}

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
        visualize_index(&*repo.index()?),
        snapbox::str![[r#"
100644:3aac70f file
120000:c4c364c link

"#]]
    );
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        snapbox::str![[r#"
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

    // Nothing changed as the checkout was aborted.
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
        "1\r\n2\r\n3\r\n",
        "the tracked blob must start from digit-only CRLF content so the later spelled-out edits are clearly distinguishable"
    );

    // This write is with line-endings that are unchanged from the ones on disk, and from what's in Git (CRLF).
    std::fs::write(&file_path, b"1\r\ntwo from worktree\r\n3\r\n")?;
    assert_eq!(
        git_status(&repo)?,
        " M ImportOrdersJob.cs\n",
        "the worktree edit must be visible before checkout"
    );

    let new_commit = build_commit(
        &repo,
        |tree| {
            // This commit also has the right line endings (CRLF)
            let blob_id = repo.write_blob(b"1\r\n2\r\nthree from target\r\n")?;
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
    head_update: "Update refs/heads/main to Some(Object(Sha1(a530b145a2513ba5b2a4418bbb74920d3967f8fb)))",
}

"#]]
    );

    assert_eq!(
        std::fs::read(&file_path)?.as_bstr(),
        "1\r\ntwo from worktree\r\nthree from target\r\n",
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
    let file_path = repo.workdir_path("file").unwrap();
    let file2_path = repo.workdir_path("file2").unwrap();
    assert_eq!(
        std::fs::read_to_string(&file_path)?,
        "line1\nadded-a\nadded-b\nline2\nline3\n"
    );

    // Amending `added-a` into a commit outside this checkout's history leaves its
    // `HEAD` where it is, so the checkout has no tree change of its own to ride on.
    let head = repo.head_commit()?.id;
    let consumed = build_commit(
        &repo,
        |tree| {
            let blob_id = repo.write_blob(b"line1\nadded-a\nline2\nline3\n")?;
            tree.upsert("file", EntryKind::Blob, blob_id)?;
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
        "line1\nadded-b\nline2\nline3\n",
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
    let file_path = repo.workdir_path("file").unwrap();
    let head = repo.head_commit()?.id;
    let consumed = build_commit(
        &repo,
        |tree| {
            let blob_id = repo.write_blob(b"line1\nadded-a\nline2\nline3\n")?;
            tree.upsert("file", EntryKind::Blob, blob_id)?;
            Ok(())
        },
        "HEAD^{tree} plus the consumed change",
    )?
    .tree_id()?
    .detach();

    // Someone writes to the file between computing the override and checking out.
    std::fs::write(
        &file_path,
        "line1\nadded-a\nadded-b\nline2\nline3\nappended\n",
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
        "line1\nadded-b\nline2\nline3\nappended\n",
        "the snapshot is taken live, so the concurrent edit survives the cancellation"
    );
    Ok(())
}

mod utils {}
