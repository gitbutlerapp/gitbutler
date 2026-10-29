use std::fs::{File, Permissions};

use crate::support::testing_repository::TestingRepository;
use but_core::RepositoryExt as _;
use but_testsupport::{
    gix_testtools::scripted_fixture_read_only, open_repo, visualize_tree as visualize_tree_inner,
};
use gix::prelude::ObjectIdExt as _;
use snapbox::IntoData;

const MAX_FILE_SIZE_BYTES: u64 = 20;

fn tree_entry_count(tree: gix::Id<'_>) -> anyhow::Result<usize> {
    Ok(tree.object()?.peel_to_tree()?.iter().count())
}

fn visualize_tree(tree: gix::Id<'_>) -> String {
    visualize_tree_inner(tree).to_string()
}

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

    // | add                | delete            | no-action |
    #[test]
    fn index_new_worktree_delete() -> anyhow::Result<()> {
        let test = TestingRepository::from_fixture("create-wd-tree-index-new-worktree-delete");

        let repo = gix_repo(&test)?;
        let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;

        assert_eq!(tree_entry_count(tree)?, 0, "Tree should end up empty");
        Ok(())
    }

    // | modify             | delete            | remove    |
    #[test]
    fn index_modify_worktree_delete() -> anyhow::Result<()> {
        let test = TestingRepository::from_fixture("create-wd-tree-index-modify-worktree-delete");

        let repo = gix_repo(&test)?;
        let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;

        assert_eq!(tree_entry_count(tree)?, 0, "Tree should end up empty");
        Ok(())
    }

    // |                    | delete            | remove    |
    #[test]
    fn worktree_delete() -> anyhow::Result<()> {
        let test = TestingRepository::open_with_initial_commit(&[("file1.txt", "content1")]);

        std::fs::remove_file(test.fixture_dir.path().join("file1.txt"))?;

        let repo = gix_repo(&test)?;
        let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;

        assert_eq!(tree_entry_count(tree)?, 0, "Tree should end up empty");
        Ok(())
    }

    // | delete             |                   | remove    |
    #[test]
    fn index_delete() -> anyhow::Result<()> {
        let test = TestingRepository::from_fixture("create-wd-tree-index-delete");

        let repo = gix_repo(&test)?;
        let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;

        // We should ignore whatever happens to the index - the current worktree state matters.
        snapbox::assert_data_eq!(
            visualize_tree(tree),
            snapbox::str![[r#"
7cd1c45
└── file1.txt:100644:dd954e7 "content1"

"#]]
            .raw()
        );
        Ok(())
    }

    // | delete             | add               | upsert    |
    #[test]
    fn index_delete_worktree_add() -> anyhow::Result<()> {
        let test = TestingRepository::from_fixture("create-wd-tree-index-delete-worktree-add");

        let repo = gix_repo(&test)?;
        let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;

        // Tree should match whatever is written on disk
        snapbox::assert_data_eq!(
            visualize_tree(tree),
            snapbox::str![[r#"
f87e9ef
└── file1.txt:100644:db00fd6 "content2"

"#]]
            .raw()
        );
        Ok(())
    }

    // | add                |                   | upsert    |
    #[test]
    fn index_add() -> anyhow::Result<()> {
        let test = TestingRepository::from_fixture("create-wd-tree-index-add");

        let repo = gix_repo(&test)?;
        let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;

        snapbox::assert_data_eq!(
            visualize_tree(tree),
            snapbox::str![[r#"
f87e9ef
└── file1.txt:100644:db00fd6 "content2"

"#]]
            .raw()
        );
        Ok(())
    }

    // |                    | add               | upsert    |
    #[test]
    fn worktree_add() -> anyhow::Result<()> {
        let test = TestingRepository::open_with_initial_commit(&[]);

        std::fs::write(test.fixture_dir.path().join("file1.txt"), "content2")?;

        let repo = gix_repo(&test)?;
        let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;

        snapbox::assert_data_eq!(
            visualize_tree(tree),
            snapbox::str![[r#"
f87e9ef
└── file1.txt:100644:db00fd6 "content2"

"#]]
            .raw()
        );
        Ok(())
    }

    // | add                | modify            | upsert    |
    #[test]
    fn index_add_worktree_modify() -> anyhow::Result<()> {
        let test = TestingRepository::from_fixture("create-wd-tree-index-add-worktree-modify");

        let repo = gix_repo(&test)?;
        let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;

        snapbox::assert_data_eq!(
            visualize_tree(tree),
            snapbox::str![[r#"
f87e9ef
└── file1.txt:100644:db00fd6 "content2"

"#]]
            .raw()
        );
        Ok(())
    }

    // | modify             | modify            | upsert    |
    #[test]
    fn index_modify_worktree_modify_racy_git() -> anyhow::Result<()> {
        let test = TestingRepository::from_fixture("create-wd-tree-index-modify-worktree-modify");

        let repo = gix_repo(&test)?;
        let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;

        snapbox::assert_data_eq!(
            visualize_tree(tree),
            snapbox::str![[r#"
d377861
└── file1.txt:100644:a2b3229 "content3"

"#]]
            .raw()
        );
        Ok(())
    }

    // | modify             |                   | upsert    |
    #[test]
    fn index_modify() -> anyhow::Result<()> {
        let test = TestingRepository::from_fixture("create-wd-tree-index-modify");

        let repo = gix_repo(&test)?;
        let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;

        snapbox::assert_data_eq!(
            visualize_tree(tree),
            snapbox::str![[r#"
f87e9ef
└── file1.txt:100644:db00fd6 "content2"

"#]]
            .raw()
        );
        Ok(())
    }
}

#[test]
fn lists_uncommited_changes() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[]);

    std::fs::write(test.fixture_dir.path().join("file1.txt"), "content1")?;
    std::fs::write(test.fixture_dir.path().join("file2.txt"), "content2")?;

    let repo = gix_repo(&test)?;
    let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;

    snapbox::assert_data_eq!(
        visualize_tree(tree),
        snapbox::str![[r#"
1ae8c21
├── file1.txt:100644:dd954e7 "content1"
└── file2.txt:100644:db00fd6 "content2"

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn does_not_include_staged_but_deleted_files() -> anyhow::Result<()> {
    let test = TestingRepository::from_fixture("create-wd-tree-staged-deleted-file");

    let repo = gix_repo(&test)?;
    let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;

    snapbox::assert_data_eq!(
        visualize_tree(tree),
        snapbox::str![[r#"
1ae8c21
├── file1.txt:100644:dd954e7 "content1"
└── file2.txt:100644:db00fd6 "content2"

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn should_be_empty_after_checking_out_empty_tree() -> anyhow::Result<()> {
    let test = TestingRepository::from_fixture("create-wd-tree-empty-worktree-from-two-files");

    assert!(!test.fixture_dir.path().join("file1.txt").exists());
    assert!(!test.fixture_dir.path().join("file2.txt").exists());

    let repo = gix_repo(&test)?;
    let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;

    // `create_wd_tree` uses the head commit as the base, and then performs
    // modifications to the tree to match the working tree.
    assert_eq!(tree_entry_count(tree)?, 0);
    Ok(())
}

#[test]
fn should_track_deleted_files() -> anyhow::Result<()> {
    let test = TestingRepository::from_fixture("create-wd-tree-empty-index-delete-file");

    assert!(!test.fixture_dir.path().join("file1.txt").exists());
    assert!(test.fixture_dir.path().join("file2.txt").exists());

    let repo = gix_repo(&test)?;
    let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;

    snapbox::assert_data_eq!(
        visualize_tree(tree),
        snapbox::str![[r#"
295a2e4
└── file2.txt:100644:db00fd6 "content2"

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn should_not_change_index() -> anyhow::Result<()> {
    let test = TestingRepository::from_fixture("create-wd-tree-empty-index-file1");

    let repo = gix_repo(&test)?;
    assert_eq!(
        repo.index()?.entries().len(),
        0,
        "it starts with an empty index"
    );
    let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;

    // Tree should match whatever is written on disk
    snapbox::assert_data_eq!(
        visualize_tree(tree),
        snapbox::str![[r#"
7cd1c45
└── file1.txt:100644:dd954e7 "content1"

"#]]
        .raw()
    );
    assert_eq!(repo.index()?.entries().len(), 0, "the index is untouched");
    Ok(())
}

#[test]
fn tree_behavior() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[
        ("dir1/file1.txt", "content1"),
        ("dir2/file2.txt", "content2"),
    ]);

    // Update a file in a directory
    std::fs::write(test.fixture_dir.path().join("dir1/file1.txt"), "new1")?;
    // Make a new directory and file
    std::fs::create_dir(test.fixture_dir.path().join("dir3"))?;
    std::fs::write(test.fixture_dir.path().join("dir3/file1.txt"), "new2")?;

    let repo = gix_repo(&test)?;
    let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;

    snapbox::assert_data_eq!(
        visualize_tree(tree),
        snapbox::str![[r#"
c8aa4f7
├── dir1:dce0d03 
│   └── file1.txt:100644:e4a8953 "new1"
├── dir2:295a2e4 
│   └── file2.txt:100644:db00fd6 "content2"
└── dir3:92e07f7 
    └── file1.txt:100644:1fda1b4 "new2"

"#]]
        .raw()
    );
    Ok(())
}

#[test]
#[cfg(unix)]
fn executable_blobs() -> anyhow::Result<()> {
    use std::{io::Write, os::unix::fs::PermissionsExt as _};

    let test = TestingRepository::open_with_initial_commit(&[]);

    let mut file = File::create(test.fixture_dir.path().join("file1.txt"))?;
    file.set_permissions(Permissions::from_mode(0o755))?;
    file.write_all(b"content1")?;

    let repo = gix_repo(&test)?;
    let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;

    // The executable bit is also present in the tree.
    snapbox::assert_data_eq!(
        visualize_tree(tree),
        snapbox::str![[r#"
4cb9de9
└── file1.txt:100755:dd954e7 "content1"

"#]]
        .raw()
    );
    Ok(())
}

#[test]
#[cfg(unix)]
fn links() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[("target", "helloworld")]);

    std::os::unix::fs::symlink("target", test.fixture_dir.path().join("link1.txt"))?;

    let repo = gix_repo(&test)?;
    let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;

    // Links are also present in the tree.
    snapbox::assert_data_eq!(
        visualize_tree(tree),
        snapbox::str![[r#"
0aefe10
├── link1.txt:120000:1de5659 "target"
└── target:100644:620ffd0 "helloworld"

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn tracked_file_becomes_directory_in_worktree() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[(
        "soon-directory",
        "this tracked file becomes a directory",
    )]);
    let worktree_path = test.fixture_dir.path().join("soon-directory");
    std::fs::remove_file(&worktree_path)?;
    std::fs::create_dir(&worktree_path)?;
    std::fs::write(worktree_path.join("file"), "content in directory")?;

    let repo = gix_repo(&test)?;
    let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;
    snapbox::assert_data_eq!(
        visualize_tree(tree),
        snapbox::str![[r#"
8b80519
└── soon-directory:df6d699 
    └── file:100644:dadf628 "content in directory"

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn tracked_directory_becomes_file_in_worktree() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[(
        "soon-file/content",
        "this tracked is removed and the parent dir becomes a file",
    )]);
    let worktree_path = test.fixture_dir.path().join("soon-file");
    std::fs::remove_dir_all(&worktree_path)?;
    std::fs::write(worktree_path, "content")?;

    let repo = gix_repo(&test)?;
    let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;
    snapbox::assert_data_eq!(
        visualize_tree(tree),
        snapbox::str![[r#"
637be29
└── soon-file:100644:6b584e8 "content"

"#]]
        .raw()
    );
    Ok(())
}

#[test]
#[cfg(unix)]
fn non_files_are_ignored() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[]);

    let fifo_path = test.fixture_dir.path().join("fifo");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo_path)
            .status()?
            .success()
    );

    let repo = gix_repo(&test)?;
    let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;
    assert_eq!(
        tree_entry_count(tree)?,
        0,
        "It completely ignores non-files, it doesn't see them, just like Git"
    );
    Ok(())
}

#[test]
#[cfg(unix)]
fn tracked_file_swapped_with_non_file() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[("soon-fifo", "actual content")]);

    let fifo_path = test.fixture_dir.path().join("soon-fifo");
    std::fs::remove_file(&fifo_path)?;
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo_path)
            .status()?
            .success()
    );

    let repo = gix_repo(&test)?;
    let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;
    assert_eq!(
        tree_entry_count(tree)?,
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

    let ignored_path = test.fixture_dir.path().join("I-am.ignored");
    std::fs::write(&ignored_path, "")?;

    let repo = gix_repo(&test)?;
    let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;
    // ignored files aren't picked up.
    snapbox::assert_data_eq!(
        visualize_tree(tree),
        snapbox::str![[r#"
38b94c0
├── .gitignore:100644:669be81 "*.ignored"
└── tracked:100644:6b584e8 "content"

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn can_autotrack_empty_files() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[("soon-empty", "content")]);

    let ignored_path = test.fixture_dir.path().join("soon-empty");
    std::fs::write(&ignored_path, "")?;

    let repo = gix_repo(&test)?;
    let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;
    // ignored files aren't picked up.
    snapbox::assert_data_eq!(
        visualize_tree(tree),
        snapbox::str![[r#"
4fe2781
└── soon-empty:100644:e69de29 ""

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn intent_to_add_is_picked_up_just_like_untracked() -> anyhow::Result<()> {
    let repo = repo("intent-to-add")?;

    let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;
    // We pick up what's in the worktree, independently of the intent-to-add flag.
    snapbox::assert_data_eq!(
        visualize_tree(tree),
        snapbox::str![[r#"
d6a22f9
└── to-be-added:100644:6b584e8 "content"

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn submodule_in_index_is_picked_up() -> anyhow::Result<()> {
    let repo = repo("with-submodule-in-index")?;

    let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;
    // Everything that is not contending with the worktree that is already in the index
    // is picked up, even if it involves submodules.
    snapbox::assert_data_eq!(
        visualize_tree(tree),
        snapbox::str![[r#"
de956ee
├── .gitmodules:100644:db28142 "[submodule \"sm\"]\n\tpath = sm\n\turl = ../module\n"
└── sm:160000:2e70126 

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn submodule_change() -> anyhow::Result<()> {
    let repo = repo("with-submodule-new-commit")?;

    let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;

    // Changes to submodule heads are also picked up.
    snapbox::assert_data_eq!(
        visualize_tree(tree),
        snapbox::str![[r#"
8b0adff
├── .gitmodules:100644:db28142 "[submodule \"sm\"]\n\tpath = sm\n\turl = ../module\n"
└── sm:160000:e8a2d3a 

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn big_files_check_is_disabled_with_zero() -> anyhow::Result<()> {
    let test = TestingRepository::open_with_initial_commit(&[]);

    std::fs::write(test.fixture_dir.path().join("empty"), "")?;
    std::fs::write(test.fixture_dir.path().join("with-content"), "content")?;

    let repo = gix_repo(&test)?;
    let tree = create_wd_tree(&repo, 0)?;

    // Everything goes with 0
    snapbox::assert_data_eq!(
        visualize_tree(tree),
        snapbox::str![[r#"
f6e159b
├── empty:100644:e69de29 ""
└── with-content:100644:6b584e8 "content"

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn big_files_are_ignored_based_on_threshold_in_working_tree() -> anyhow::Result<()> {
    let test =
        TestingRepository::open_with_initial_commit(&[("soon-too-big", "still small enough")]);

    let big_file_path = test.fixture_dir.path().join("soon-too-big");
    std::fs::write(&big_file_path, "a massive file above the threshold")?;

    let repo = gix_repo(&test)?;
    let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;

    // It does not pickup the big worktree change.
    snapbox::assert_data_eq!(
        visualize_tree(tree),
        snapbox::str![[r#"
26ea3c5
└── soon-too-big:100644:7d72316 "still small enough"

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn big_files_are_fine_when_in_the_index() -> anyhow::Result<()> {
    let test = TestingRepository::from_fixture("create-wd-tree-big-file-staged");

    let repo = gix_repo(&test)?;
    let tree = create_wd_tree(&repo, MAX_FILE_SIZE_BYTES)?;

    // It keeps files that were already added.
    snapbox::assert_data_eq!(
        visualize_tree(tree),
        snapbox::str![[r#"
bbd82c6
└── soon-too-big:100644:1799e5a "a massive file above the threshold"

"#]]
        .raw()
    );
    Ok(())
}

fn gix_repo(test: &TestingRepository) -> anyhow::Result<gix::Repository> {
    open_repo(test.fixture_dir.path())
}

fn repo(name: &str) -> anyhow::Result<gix::Repository> {
    let worktree_dir = scripted_fixture_read_only("make_create_wd_tree_repos.sh")
        .map_err(anyhow::Error::from_boxed)?
        .join(name);
    open_repo(&worktree_dir)
}

fn create_wd_tree<'repo>(
    repo: &'repo gix::Repository,
    untracked_limit_in_bytes: u64,
) -> anyhow::Result<gix::Id<'repo>> {
    #[expect(deprecated)]
    Ok(repo.create_wd_tree(untracked_limit_in_bytes)?.attach(repo))
}
