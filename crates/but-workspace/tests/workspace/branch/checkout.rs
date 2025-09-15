use crate::branch::checkout::utils::build_commit;
use crate::utils::{
    read_only_in_memory_scenario, visualize_index, writable_scenario, writable_scenario_slow,
};
use but_testsupport::{git_status, visualize_commit_graph_all};
use but_workspace::branch::safe_checkout;
use gix::object::tree::EntryKind;

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
        head_update: "Update refs/heads/main to Some(Object(Sha1(6f695b43b0a2fc309a7444d3e918226f1561c66c)))",
    }
    "#);

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 6f695b4 (HEAD -> main) init");
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
#[ignore = "TBD: needs gix support for learning about affected paths, Editor::get_all()"]
fn snapshot_fails_by_default_if_changed_file_turns_into_directory() -> anyhow::Result<()> {
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
        "turn changed file into a directory",
    )?;

    let err = safe_checkout(head_commit.id, new_commit.id, &repo, Default::default()).unwrap_err();
    assert_eq!(
        err.to_string(),
        "TBD",
        "conflicting worktree changes prevent a commit"
    );

    // Nothing changed as the checkout was aborted.
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
fn checkout_handles_directory_and_file_replacements() -> anyhow::Result<()> {
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
        "Refusing to overwrite conflicting paths: 'file'",
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
