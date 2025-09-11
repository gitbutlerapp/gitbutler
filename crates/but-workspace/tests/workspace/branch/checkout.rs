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
    let head_refname = repo.head_name()?.expect("refname is predefined");
    let head_commit_id = repo
        .commit("HEAD", "init", empty_tree, None::<gix::ObjectId>)?
        .detach();
    // restore the previous state (can't avoid setting the ref for now).
    std::fs::remove_file(repo.git_dir().join(head_refname.to_string()))?;

    let out = safe_checkout(empty_tree, head_commit_id, &repo, Default::default())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        snapshot_tree: None,
        num_changed_files: 0,
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
        num_changed_files: 0,
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
#[ignore = "TBD"]
fn overlapping_directory_in_worktree_changes() -> anyhow::Result<()> {
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
        num_changed_files: 1,
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
    ");
    insta::assert_snapshot!(git_status(&repo)?, @r"
    UU file
    D  unrelated
    ");

    // Edit the file that is conflicting
    let (head_commit, new_commit) = build_commit(
        &repo,
        |tree| {
            tree.upsert("file", EntryKind::Blob, repo.empty_blob().id)?;
            Ok(())
        },
        "overwrite conflicting file",
    )?;

    let out = safe_checkout(head_commit.id, new_commit.id, &repo, Default::default())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        snapshot_tree: None,
        num_changed_files: 1,
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
    // Only the unrelated file was added, only visible in the index.
    insta::assert_snapshot!(visualize_index(&*repo.index()?), @r"
    100644:e69de29 file:1
    100644:e6c4914 file:2
    100644:e33f5e9 file:3
    ");
    insta::assert_snapshot!(git_status(&repo)?, @r"
    UU file
    D  unrelated
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
        num_changed_files: 1,
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
    ");
    insta::assert_snapshot!(git_status(&repo)?, @r"
     D executable
     D file
     D link
    D  unrelated
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

        // TODO(gix): empty blobs should always be present
        repo.write_blob(&[])?;

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
