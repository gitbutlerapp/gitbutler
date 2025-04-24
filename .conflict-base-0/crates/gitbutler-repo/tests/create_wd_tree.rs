use std::{
    fs::{File, Permissions},
    path::Path,
};

use gitbutler_repo::RepositoryExt as _;
use gitbutler_testsupport::gix_testtools::scripted_fixture_read_only;
use gitbutler_testsupport::testing_repository::TestingRepository;
use gitbutler_testsupport::visualize_git2_tree;

const MAX_SIZE: u64 = 20;

/// These tests exercise the truth table that we use to update the HEAD
/// tree to match the worktree.
///
/// Truth table for upsert/remove:
/// | HEAD Tree -> Index | Index -> Worktree | Action |
/// | add                | delete            | no-action |
/// | modify             | delete            | remove |
/// |                    | delete            | remove |
/// | delete             |                   | remove |
/// | delete             | add               | upsert |
/// | add                |                   | upsert |
/// |                    | add               | upsert |
/// | add                | modify            | upsert |
/// | modify             | modify            | upsert |
#[cfg(test)]
mod head_upsert_truthtable {
    use super::*;
    use gitbutler_testsupport::visualize_git2_tree;

    // | add                | delete            | no-action |
    #[test]
    fn index_new_worktree_delete() -> anyhow::Result<()> {
        let test = TestingRepository::open_with_initial_commit(&[]);

        std::fs::write(test.tempdir.path().join("file1.txt"), "content1")?;

        let mut index = test.repository.index()?;
        index.add_path(Path::new("file1.txt"))?;
        index.write()?;

        std::fs::remove_file(test.tempdir.path().join("file1.txt"))?;

        let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;

        assert_eq!(tree.len(), 0, "Tree should end up empty");
        Ok(())
    }

    // | modify             | delete            | remove    |
    #[test]
    fn index_modify_worktree_delete() -> anyhow::Result<()> {
        let test = TestingRepository::open_with_initial_commit(&[("file1.txt", "content1")]);

        std::fs::write(test.tempdir.path().join("file1.txt"), "content2")?;

        let mut index = test.repository.index()?;
        index.add_path(Path::new("file1.txt"))?;
        index.write()?;

        std::fs::remove_file(test.tempdir.path().join("file1.txt"))?;

        let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;

        assert_eq!(tree.len(), 0, "Tree should end up empty");
        Ok(())
    }

    // |                    | delete            | remove    |
    #[test]
    fn worktree_delete() -> anyhow::Result<()> {
        let test = TestingRepository::open_with_initial_commit(&[("file1.txt", "content1")]);

        std::fs::remove_file(test.tempdir.path().join("file1.txt"))?;

        let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;

        assert_eq!(tree.len(), 0, "Tree should end up empty");
        Ok(())
    }

    // | delete             |                   | remove    |
    #[test]
    fn index_delete() -> anyhow::Result<()> {
        let test = TestingRepository::open_with_initial_commit(&[("file1.txt", "content1")]);

        let mut index = test.repository.index()?;
        index.remove_all(["*"], None)?;
        index.write()?;

        let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;

        // We should ignore whatever happens to the index - the current worktree state matters.
        insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r#"
        7cd1c45
        └── file1.txt:100644:dd954e7 "content1"
        "#);
        Ok(())
    }

    // | delete             | add               | upsert    |
    #[test]
    fn index_delete_worktree_add() -> anyhow::Result<()> {
        let test = TestingRepository::open_with_initial_commit(&[("file1.txt", "content1")]);

        let mut index = test.repository.index()?;
        index.remove_all(["*"], None)?;
        index.write()?;

        std::fs::write(test.tempdir.path().join("file1.txt"), "content2")?;

        let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;

        // Tree should match whatever is written on disk
        insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r#"
        f87e9ef
        └── file1.txt:100644:db00fd6 "content2"
        "#);
        Ok(())
    }

    // | add                |                   | upsert    |
    #[test]
    fn index_add() -> anyhow::Result<()> {
        let test = TestingRepository::open_with_initial_commit(&[]);

        std::fs::write(test.tempdir.path().join("file1.txt"), "content2")?;

        let mut index = test.repository.index()?;
        index.add_path(Path::new("file1.txt"))?;
        index.write()?;

        let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;

        insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r#"
        f87e9ef
        └── file1.txt:100644:db00fd6 "content2"
        "#);
        Ok(())
    }

    // |                    | add               | upsert    |
    #[test]
    fn worktree_add() -> anyhow::Result<()> {
        let test = TestingRepository::open_with_initial_commit(&[]);

        std::fs::write(test.tempdir.path().join("file1.txt"), "content2")?;

        let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;

        insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r#"
        f87e9ef
        └── file1.txt:100644:db00fd6 "content2"
        "#);
        Ok(())
    }

    // | add                | modify            | upsert    |
    #[test]
    fn index_add_worktree_modify() -> anyhow::Result<()> {
        let test = TestingRepository::open_with_initial_commit(&[]);

        std::fs::write(test.tempdir.path().join("file1.txt"), "content1")?;

        let mut index = test.repository.index()?;
        index.add_path(Path::new("file1.txt"))?;
        index.write()?;

        std::fs::write(test.tempdir.path().join("file1.txt"), "content2")?;

        let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;

        insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r#"
        f87e9ef
        └── file1.txt:100644:db00fd6 "content2"
        "#);
        Ok(())
    }

    // | modify             | modify            | upsert    |
    #[test]
    fn index_modify_worktree_modify_racy_git() -> anyhow::Result<()> {
        let test = TestingRepository::open_with_initial_commit(&[("file1.txt", "content1")]);

        let file_path = test.tempdir.path().join("file1.txt");
        std::fs::write(&file_path, "content2")?;

        let mut index = test.repository.index()?;
        index.add_path(Path::new("file1.txt"))?;
        index.write()?;

        // This change is made within the same second, so if racy-git isn't handled correctly,
        // this change won't be seen.
        std::fs::write(file_path, "content3")?;

        let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;

        insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r#"
        d377861
        └── file1.txt:100644:a2b3229 "content3"
        "#);
        Ok(())
    }

    // | modify             |                   | upsert    |
    #[test]
    fn index_modify() -> anyhow::Result<()> {
        let test = TestingRepository::open_with_initial_commit(&[("file1.txt", "content1")]);

        let file_path = test.tempdir.path().join("file1.txt");
        std::fs::write(&file_path, "content2")?;

        let mut index = test.repository.index()?;
        index.add_path(Path::new("file1.txt"))?;
        index.write()?;

        let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;

        insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r#"
        f87e9ef
        └── file1.txt:100644:db00fd6 "content2"
        "#);
        Ok(())
    }
}

#[test]
fn lists_uncommited_changes() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[]);

    std::fs::write(test.tempdir.path().join("file1.txt"), "content1")?;
    std::fs::write(test.tempdir.path().join("file2.txt"), "content2")?;

    let tree = test.repository.create_wd_tree(MAX_SIZE)?;

    insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r#"
    1ae8c21
    ├── file1.txt:100644:dd954e7 "content1"
    └── file2.txt:100644:db00fd6 "content2"
    "#);
    Ok(())
}

#[test]
fn does_not_include_staged_but_deleted_files() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[]);

    std::fs::write(test.tempdir.path().join("file1.txt"), "content1")?;
    std::fs::write(test.tempdir.path().join("file2.txt"), "content2")?;

    std::fs::write(test.tempdir.path().join("file3.txt"), "content2")?;
    let mut index: git2::Index = test.repository.index()?;
    index.add_path(Path::new("file3.txt"))?;
    index.write()?;
    std::fs::remove_file(test.tempdir.path().join("file3.txt"))?;

    let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;

    insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r#"
    1ae8c21
    ├── file1.txt:100644:dd954e7 "content1"
    └── file2.txt:100644:db00fd6 "content2"
    "#);
    Ok(())
}

#[test]
fn should_be_empty_after_checking_out_empty_tree() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[
        ("file1.txt", "content1"),
        ("file2.txt", "content2"),
    ]);

    // Checkout an empty tree
    {
        let tree_oid = test.repository.treebuilder(None)?.write()?;
        let tree = test.repository.find_tree(tree_oid)?;
        test.repository
            .checkout_tree_builder(&tree)
            .force()
            .remove_untracked()
            .checkout()?;
    }

    assert!(!test.tempdir.path().join("file1.txt").exists());
    assert!(!test.tempdir.path().join("file2.txt").exists());

    let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;

    // `create_wd_tree` uses the head commit as the base, and then performs
    // modifications to the tree to match the working tree.
    assert_eq!(tree.len(), 0);
    Ok(())
}

#[test]
fn should_track_deleted_files() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[
        ("file1.txt", "content1"),
        ("file2.txt", "content2"),
    ]);

    // Make sure the index is empty, perhaps the user did this action
    let mut index: git2::Index = test.repository.index()?;
    index.remove_all(["*"], None)?;
    index.write()?;

    std::fs::remove_file(test.tempdir.path().join("file1.txt"))?;

    assert!(!test.tempdir.path().join("file1.txt").exists());
    assert!(test.tempdir.path().join("file2.txt").exists());

    let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;

    insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r#"
    295a2e4
    └── file2.txt:100644:db00fd6 "content2"
    "#);
    Ok(())
}

#[test]
fn should_not_change_index() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[("file1.txt", "content1")]);

    let mut index = test.repository.index()?;
    index.remove_all(["*"], None)?;
    index.write()?;

    let index_tree = index.write_tree()?;
    let index_tree = test.repository.find_tree(index_tree)?;
    assert_eq!(index_tree.len(), 0);

    let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;

    let mut index = test.repository.index()?;
    let index_tree = index.write_tree()?;
    let index_tree = test.repository.find_tree(index_tree)?;
    assert_eq!(index_tree.len(), 0);

    // Tree should match whatever is written on disk
    insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r#"
    7cd1c45
    └── file1.txt:100644:dd954e7 "content1"
    "#);
    Ok(())
}

#[test]
fn tree_behavior() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[
        ("dir1/file1.txt", "content1"),
        ("dir2/file2.txt", "content2"),
    ]);

    // Update a file in a directory
    std::fs::write(test.tempdir.path().join("dir1/file1.txt"), "new1")?;
    // Make a new directory and file
    std::fs::create_dir(test.tempdir.path().join("dir3"))?;
    std::fs::write(test.tempdir.path().join("dir3/file1.txt"), "new2")?;

    let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;

    insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r#"
    c8aa4f7
    ├── dir1:dce0d03 
    │   └── file1.txt:100644:e4a8953 "new1"
    ├── dir2:295a2e4 
    │   └── file2.txt:100644:db00fd6 "content2"
    └── dir3:92e07f7 
        └── file1.txt:100644:1fda1b4 "new2"
    "#);
    Ok(())
}

#[test]
#[cfg(unix)]
fn executable_blobs() -> anyhow::Result<()> {
    use std::{io::Write, os::unix::fs::PermissionsExt as _};

    let test = TestingRepository::open_with_initial_commit(&[]);

    let mut file = File::create(test.tempdir.path().join("file1.txt"))?;
    file.set_permissions(Permissions::from_mode(0o755))?;
    file.write_all(b"content1")?;

    let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;

    // The executable bit is also present in the tree.
    insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r#"
    4cb9de9
    └── file1.txt:100755:dd954e7 "content1"
    "#);
    Ok(())
}

#[test]
#[cfg(unix)]
fn links() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[("target", "helloworld")]);

    std::os::unix::fs::symlink("target", test.tempdir.path().join("link1.txt"))?;

    let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;

    // Links are also present in the tree.
    insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r#"
    0aefe10
    ├── link1.txt:120000:1de5659 "target"
    └── target:100644:620ffd0 "helloworld"
    "#);
    Ok(())
}

#[test]
fn tracked_file_becomes_directory_in_worktree() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[(
        "soon-directory",
        "this tracked file becomes a directory",
    )]);
    let worktree_path = test.tempdir.path().join("soon-directory");
    std::fs::remove_file(&worktree_path)?;
    std::fs::create_dir(&worktree_path)?;
    std::fs::write(worktree_path.join("file"), "content in directory")?;

    let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;
    insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r#"
    8b80519
    └── soon-directory:df6d699 
        └── file:100644:dadf628 "content in directory"
    "#);
    Ok(())
}

#[test]
fn tracked_directory_becomes_file_in_worktree() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[(
        "soon-file/content",
        "this tracked is removed and the parent dir becomes a file",
    )]);
    let worktree_path = test.tempdir.path().join("soon-file");
    std::fs::remove_dir_all(&worktree_path)?;
    std::fs::write(worktree_path, "content")?;

    let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;
    insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r#"
    637be29
    └── soon-file:100644:6b584e8 "content"
    "#);
    Ok(())
}

#[test]
#[cfg(unix)]
fn non_files_are_ignored() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[]);

    let fifo_path = test.tempdir.path().join("fifo");
    assert!(std::process::Command::new("mkfifo")
        .arg(&fifo_path)
        .status()?
        .success());

    let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;
    assert_eq!(
        tree.len(),
        0,
        "It completely ignores non-files, it doesn't see them, just like Git"
    );
    Ok(())
}

#[test]
#[cfg(unix)]
fn tracked_file_swapped_with_non_file() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[("soon-fifo", "actual content")]);

    let fifo_path = test.tempdir.path().join("soon-fifo");
    std::fs::remove_file(&fifo_path)?;
    assert!(std::process::Command::new("mkfifo")
        .arg(&fifo_path)
        .status()?
        .success());

    let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;
    assert_eq!(
        tree.len(),
        0,
        "It completely ignores non-files, it doesn't see them, just like Git, even when previously tracked"
    );
    Ok(())
}

#[test]
fn ignored_files() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[
        ("tracked", "content"),
        (".gitignore", "*.ignored"),
    ]);

    let ignored_path = test.tempdir.path().join("I-am.ignored");
    std::fs::write(&ignored_path, "")?;

    let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;
    // ignored files aren't picked up.
    insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r#"
    38b94c0
    ├── .gitignore:100644:669be81 "*.ignored"
    └── tracked:100644:6b584e8 "content"
    "#);
    Ok(())
}

#[test]
fn can_autotrack_empty_files() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[("soon-empty", "content")]);

    let ignored_path = test.tempdir.path().join("soon-empty");
    std::fs::write(&ignored_path, "")?;

    let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;
    // ignored files aren't picked up.
    insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r#"
    4fe2781
    └── soon-empty:100644:e69de29 ""
    "#);
    Ok(())
}

#[test]
fn intent_to_add_is_picked_up_just_like_untracked() -> anyhow::Result<()> {
    let repo = repo("intent-to-add")?;

    let tree: git2::Tree = repo.create_wd_tree(MAX_SIZE)?;
    // We pick up what's in the worktree, independently of the intent-to-add flag.
    insta::assert_snapshot!(visualize_git2_tree(tree.id(), &repo), @r#"
    d6a22f9
    └── to-be-added:100644:6b584e8 "content"
    "#);
    Ok(())
}

#[test]
fn submodule_in_index_is_picked_up() -> anyhow::Result<()> {
    let repo = repo("with-submodule-in-index")?;

    let tree: git2::Tree = repo.create_wd_tree(MAX_SIZE)?;
    // Everything that is not contending with the worktree that is already in the index
    // is picked up, even if it involves submodules.
    insta::assert_snapshot!(visualize_git2_tree(tree.id(), &repo), @r#"
    de956ee
    ├── .gitmodules:100644:db28142 "[submodule \"sm\"]\n\tpath = sm\n\turl = ../module\n"
    └── sm:160000:2e70126
    "#);
    Ok(())
}

#[test]
fn submodule_change() -> anyhow::Result<()> {
    let repo = repo("with-submodule-new-commit")?;

    let tree: git2::Tree = repo.create_wd_tree(MAX_SIZE)?;

    // Changes to submodule heads are also picked up.
    insta::assert_snapshot!(visualize_git2_tree(tree.id(), &repo), @r#"
    8b0adff
    ├── .gitmodules:100644:db28142 "[submodule \"sm\"]\n\tpath = sm\n\turl = ../module\n"
    └── sm:160000:e8a2d3a
    "#);
    Ok(())
}

#[test]
fn big_files_check_is_disabled_with_zero() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[]);

    std::fs::write(test.tempdir.path().join("empty"), "")?;
    std::fs::write(test.tempdir.path().join("with-content"), "content")?;

    let tree: git2::Tree = test.repository.create_wd_tree(0)?;

    // Everything goes with 0
    insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r#"
    f6e159b
    ├── empty:100644:e69de29 ""
    └── with-content:100644:6b584e8 "content"
    "#);
    Ok(())
}

#[test]
fn big_files_are_ignored_based_on_threshold_in_working_tree() -> anyhow::Result<()> {
    let test =
        TestingRepository::open_with_initial_commit(&[("soon-too-big", "still small enough")]);

    let big_file_path = test.tempdir.path().join("soon-too-big");
    std::fs::write(&big_file_path, "a massive file above the threshold")?;

    let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;

    // It does not pickup the big worktree change.
    insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r#"
    26ea3c5
    └── soon-too-big:100644:7d72316 "still small enough"
    "#);
    Ok(())
}

#[test]
fn big_files_are_fine_when_in_the_index() -> anyhow::Result<()> {
    let test =
        TestingRepository::open_with_initial_commit(&[("soon-too-big", "still small enough")]);

    std::fs::write(
        test.tempdir.path().join("soon-too-big"),
        "a massive file above the threshold",
    )?;
    let mut index = test.repository.index()?;
    index.add_path("soon-too-big".as_ref())?;
    index.write()?;

    let tree: git2::Tree = test.repository.create_wd_tree(MAX_SIZE)?;

    // It keeps files that were already added.
    insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r#"
    bbd82c6
    └── soon-too-big:100644:1799e5a "a massive file above the threshold"
    "#);
    Ok(())
}

fn repo(name: &str) -> anyhow::Result<git2::Repository> {
    let worktree_dir = scripted_fixture_read_only("make_create_wd_tree_repos.sh")
        .map_err(anyhow::Error::from_boxed)?
        .join(name);
    Ok(git2::Repository::open(worktree_dir)?)
}
