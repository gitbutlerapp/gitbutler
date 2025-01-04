use std::{
    fs::{File, Permissions},
    path::Path,
};

use gitbutler_repo::RepositoryExt as _;
use gitbutler_testsupport::testing_repository::TestingRepository;
use gitbutler_testsupport::visualize_git2_tree;

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

        let tree: git2::Tree = test.repository.create_wd_tree()?;

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

        let tree: git2::Tree = test.repository.create_wd_tree()?;

        assert_eq!(tree.len(), 0, "Tree should end up empty");
        Ok(())
    }

    // |                    | delete            | remove    |
    #[test]
    fn worktree_delete() -> anyhow::Result<()> {
        let test = TestingRepository::open_with_initial_commit(&[("file1.txt", "content1")]);

        std::fs::remove_file(test.tempdir.path().join("file1.txt"))?;

        let tree: git2::Tree = test.repository.create_wd_tree()?;

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

        let tree: git2::Tree = test.repository.create_wd_tree()?;

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

        let tree: git2::Tree = test.repository.create_wd_tree()?;

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

        let tree: git2::Tree = test.repository.create_wd_tree()?;

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

        let tree: git2::Tree = test.repository.create_wd_tree()?;

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

        let tree: git2::Tree = test.repository.create_wd_tree()?;

        insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r#"
        f87e9ef
        └── file1.txt:100644:db00fd6 "content2"
        "#);
        Ok(())
    }

    // | modify             | modify            | upsert    |
    #[test]
    fn index_modify_worktree_modify() -> anyhow::Result<()> {
        let test = TestingRepository::open_with_initial_commit(&[("file1.txt", "content1")]);

        std::fs::write(test.tempdir.path().join("file1.txt"), "content2")?;

        let mut index = test.repository.index()?;
        index.add_path(Path::new("file1.txt"))?;
        index.write()?;

        std::fs::write(test.tempdir.path().join("file1.txt"), "content3")?;

        let tree: git2::Tree = test.repository.create_wd_tree()?;

        insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r#"
        d377861
        └── file1.txt:100644:a2b3229 "content3"
        "#);
        Ok(())
    }
}

#[test]
fn lists_uncommited_changes() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[]);

    std::fs::write(test.tempdir.path().join("file1.txt"), "content1")?;
    std::fs::write(test.tempdir.path().join("file2.txt"), "content2")?;

    let tree = test.repository.create_wd_tree()?;

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

    let tree: git2::Tree = test.repository.create_wd_tree()?;

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

    let tree: git2::Tree = test.repository.create_wd_tree()?;

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

    let tree: git2::Tree = test.repository.create_wd_tree()?;

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

    let tree: git2::Tree = test.repository.create_wd_tree()?;

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

    let tree: git2::Tree = test.repository.create_wd_tree()?;

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

    let tree: git2::Tree = test.repository.create_wd_tree()?;

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

    let tree: git2::Tree = test.repository.create_wd_tree()?;

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

    let tree: git2::Tree = test.repository.create_wd_tree().unwrap();
    insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r"");
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

    let tree: git2::Tree = test.repository.create_wd_tree().unwrap();
    insta::assert_snapshot!(visualize_git2_tree(tree.id(), &test.repository), @r"");
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

    let tree: git2::Tree = test.repository.create_wd_tree().unwrap();
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

    let tree: git2::Tree = test.repository.create_wd_tree().unwrap();
    assert_eq!(
        tree.len(),
        0,
        "It completely ignores non-files, it doesn't see them, just like Git, even when previously tracked"
    );
    Ok(())
}
