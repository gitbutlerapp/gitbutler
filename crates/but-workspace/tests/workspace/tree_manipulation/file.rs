use but_testsupport::{CommandExt, git, git_status, visualize_disk_tree_skip_dot_git};
use but_workspace::discard_workspace_changes;
use util::{file_to_spec, renamed_file_to_spec, worktree_changes_to_discard_specs};

use crate::utils::{CONTEXT_LINES, visualize_index, writable_scenario, writable_scenario_slow};

#[test]
fn all_file_types_from_unborn() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("unborn-untracked-all-file-types");
    insta::assert_snapshot!(git_status(&repo)?, @r"
?? link
?? untracked
?? untracked-exe
");

    let dropped = discard_workspace_changes(
        &repo,
        worktree_changes_to_discard_specs(&repo),
        CONTEXT_LINES,
    )?;
    assert!(dropped.is_empty());

    insta::assert_snapshot!(git_status(&repo)?, @"");
    Ok(())
}

#[test]
fn all_file_types_added_to_index() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("unborn-untracked-all-file-types");
    git(&repo).args(["add", "."]).run();
    insta::assert_snapshot!(git_status(&repo)?, @r"
A  link
A  untracked
A  untracked-exe
");

    let dropped = discard_workspace_changes(
        &repo,
        worktree_changes_to_discard_specs(&repo),
        CONTEXT_LINES,
    )?;
    assert!(dropped.is_empty());

    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"");
    Ok(())
}

#[test]
#[cfg(unix)]
fn all_file_types_deleted_in_worktree() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("delete-all-file-types-valid-submodule");
    insta::assert_snapshot!(git_status(&repo)?, @r"
D .gitmodules
D executable
D link
D submodule
");

    let dropped = discard_workspace_changes(
        &repo,
        worktree_changes_to_discard_specs(&repo),
        CONTEXT_LINES,
    )?;
    assert!(dropped.is_empty());

    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
100644:51f8807 .gitmodules
160000:a047f81 embedded-repository
100755:86daf54 executable
100644:d95f3ad file-to-remain
120000:b158162 link
160000:a047f81 submodule
");

    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
.
├── .git:40755
├── .gitmodules:100644
├── embedded-repository:40755
│   ├── .git:40755
│   └── file:100644
├── executable:100755
├── file-to-remain:100644
├── link:120755
└── submodule:40755
    ├── .git:100644
    └── file:100644
");
    Ok(())
}

#[test]
#[cfg(unix)]
fn replace_dir_with_file_discard_all_in_order_in_worktree() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("replace-dir-with-submodule-with-file");
    insta::assert_snapshot!(git_status(&repo)?, @r"
 D dir/executable
 D dir/file-to-remain
 D dir/link
 D dir/submodule
?? dir
");

    let dropped = discard_workspace_changes(
        &repo,
        worktree_changes_to_discard_specs(&repo),
        CONTEXT_LINES,
    )?;
    assert!(dropped.is_empty());

    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
100644:566c83a .gitmodules
100755:86daf54 dir/executable
100644:d95f3ad dir/file-to-remain
120000:b158162 dir/link
160000:a047f81 dir/submodule
160000:a047f81 embedded-repository
");

    // Here we managed to check out the submodule as the order of worktree changes is `dir` first,
    // followed by all the individual items in the directory. One of these restores the submodule, which
    // starts out as empty directory.
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
.
├── .git:40755
├── .gitmodules:100644
├── dir:40755
│   ├── executable:100755
│   ├── file-to-remain:100644
│   ├── link:120755
│   └── submodule:40755
│       ├── .git:100644
│       └── file:100644
└── embedded-repository:40755
    ├── .git:40755
    └── file:100644
");
    Ok(())
}

#[test]
#[cfg(unix)]
fn replace_dir_with_file_discard_all_in_order_in_index() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("replace-dir-with-submodule-with-file");
    git(&repo).args(["add", "."]).run();
    insta::assert_snapshot!(git_status(&repo)?, @r"
A  dir
D  dir/executable
D  dir/file-to-remain
D  dir/link
D  dir/submodule
");

    let dropped = discard_workspace_changes(
        &repo,
        worktree_changes_to_discard_specs(&repo),
        CONTEXT_LINES,
    )?;
    assert!(dropped.is_empty());

    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
100644:566c83a .gitmodules
100755:86daf54 dir/executable
100644:d95f3ad dir/file-to-remain
120000:b158162 dir/link
160000:a047f81 dir/submodule
160000:a047f81 embedded-repository
");

    // Here we managed to check out the submodule as the order of worktree changes is `dir` first,
    // followed by all the individual items in the directory. One of these restores the submodule, which
    // starts out as empty directory.
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
.
├── .git:40755
├── .gitmodules:100644
├── dir:40755
│   ├── executable:100755
│   ├── file-to-remain:100644
│   ├── link:120755
│   └── submodule:40755
│       ├── .git:100644
│       └── file:100644
└── embedded-repository:40755
    ├── .git:40755
    └── file:100644
");
    Ok(())
}

#[test]
#[cfg(unix)]
fn replace_dir_with_file_discard_just_the_file_in_worktree() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("replace-dir-with-submodule-with-file");
    insta::assert_snapshot!(git_status(&repo)?, @r"
 D dir/executable
 D dir/file-to-remain
 D dir/link
 D dir/submodule
?? dir
");

    let dropped = discard_workspace_changes(&repo, Some(file_to_spec("dir")), CONTEXT_LINES)?;
    assert!(dropped.is_empty());

    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
100644:566c83a .gitmodules
100755:86daf54 dir/executable
100644:d95f3ad dir/file-to-remain
120000:b158162 dir/link
160000:a047f81 dir/submodule
160000:a047f81 embedded-repository
");

    // It's a known shortcoming that submodules aren't re-populated during checkout.
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
    .
    ├── .git:40755
    ├── .gitmodules:100644
    ├── dir:40755
    │   ├── executable:100755
    │   ├── file-to-remain:100644
    │   ├── link:120755
    │   └── submodule:40755
    └── embedded-repository:40755
        ├── .git:40755
        └── file:100644
    ");
    Ok(())
}

#[test]
#[cfg(unix)]
// TODO: probably there should be a way to undo them, but it's not super clear what that means.
//       Is it picking theirs, or ours? Better gracefully reject it until there is UX for it.
fn conflicts_are_invisible() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("merge-with-two-branches-conflict");
    insta::assert_snapshot!(git_status(&repo)?, @"UU file");
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
    100644:e69de29 file:1
    100644:e6c4914 file:2
    100644:e33f5e9 file:3
    ");

    let dropped = discard_workspace_changes(&repo, Some(file_to_spec("file")), CONTEXT_LINES)?;
    assert_eq!(
        dropped,
        vec![file_to_spec("file")],
        "The file spec didn't match a worktree change, was dropped"
    );

    // Nothing was changed
    insta::assert_snapshot!(git_status(&repo)?, @"UU file");
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
    100644:e69de29 file:1
    100644:e6c4914 file:2
    100644:e33f5e9 file:3
    ");
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
.
├── .git:40755
└── file:100644
");
    Ok(())
}

#[test]
#[cfg(unix)]
fn replace_dir_with_file_discard_just_the_file_in_index() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("replace-dir-with-submodule-with-file");
    git(&repo).args(["add", "."]).run();
    insta::assert_snapshot!(git_status(&repo)?, @r"
A  dir
D  dir/executable
D  dir/file-to-remain
D  dir/link
D  dir/submodule
");

    let dropped = discard_workspace_changes(&repo, Some(file_to_spec("dir")), CONTEXT_LINES)?;
    assert!(dropped.is_empty());

    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
100644:566c83a .gitmodules
100755:86daf54 dir/executable
100644:d95f3ad dir/file-to-remain
120000:b158162 dir/link
160000:a047f81 dir/submodule
160000:a047f81 embedded-repository
");

    // It's a known shortcoming that submodules aren't re-populated during checkout.
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
    .
    ├── .git:40755
    ├── .gitmodules:100644
    ├── dir:40755
    │   ├── executable:100755
    │   ├── file-to-remain:100644
    │   ├── link:120755
    │   └── submodule:40755
    └── embedded-repository:40755
        ├── .git:40755
        └── file:100644
    ");
    Ok(())
}

#[test]
#[cfg(unix)]
fn all_file_types_modified_in_worktree() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("all-file-types-changed");
    insta::assert_snapshot!(git_status(&repo)?, @r"
M soon-executable
T soon-file-not-link
M soon-not-executable
");
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
.
├── .git:40755
├── fifo-should-be-ignored:10644
├── soon-executable:100755
├── soon-file-not-link:100644
└── soon-not-executable:100644
");

    let dropped = discard_workspace_changes(
        &repo,
        worktree_changes_to_discard_specs(&repo),
        CONTEXT_LINES,
    )?;
    assert!(dropped.is_empty());

    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
100644:d95f3ad soon-executable
120000:c4c364c soon-file-not-link
100755:86daf54 soon-not-executable
");

    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
.
├── .git:40755
├── fifo-should-be-ignored:10644
├── soon-executable:100644
├── soon-file-not-link:120755
└── soon-not-executable:100755
");
    Ok(())
}

#[test]
#[cfg(unix)]
fn all_file_types_modified_in_index() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("all-file-types-changed");
    git(&repo).args(["add", "."]).run();
    insta::assert_snapshot!(git_status(&repo)?, @r"
M  soon-executable
T  soon-file-not-link
M  soon-not-executable
");
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
.
├── .git:40755
├── fifo-should-be-ignored:10644
├── soon-executable:100755
├── soon-file-not-link:100644
└── soon-not-executable:100644
");

    let dropped = discard_workspace_changes(
        &repo,
        worktree_changes_to_discard_specs(&repo),
        CONTEXT_LINES,
    )?;
    assert!(dropped.is_empty());

    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
100644:d95f3ad soon-executable
120000:c4c364c soon-file-not-link
100755:86daf54 soon-not-executable
");

    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
.
├── .git:40755
├── fifo-should-be-ignored:10644
├── soon-executable:100644
├── soon-file-not-link:120755
└── soon-not-executable:100755
");
    Ok(())
}

#[test]
#[cfg(unix)]
fn modified_submodule_and_embedded_repo_in_worktree() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("modified-submodule-and-embedded-repo");
    insta::assert_snapshot!(git_status(&repo)?, @r"
M embedded-repository
M submodule
");
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
.
├── .git:40755
├── .gitmodules:100644
├── embedded-repository:40755
│   ├── .git:40755
│   └── file:100644
└── submodule:40755
    ├── .git:100644
    ├── file:100644
    └── untracked:100644
");
    // The submdule has changed its state, but not what the parent-repository thinks about it as it wasn't added ot the index
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
100644:51f8807 .gitmodules
160000:a047f81 embedded-repository
160000:a047f81 submodule
");

    let dropped = discard_workspace_changes(
        &repo,
        worktree_changes_to_discard_specs(&repo),
        CONTEXT_LINES,
    )?;
    assert!(dropped.is_empty());

    // The embedded repository we don't currently see due to a `gix` shortcoming - it ignores embedded repos
    // when doing a status even though it should treat it like an 'anonymous submodule'.
    // However, the submodule itself is reset.
    insta::assert_snapshot!(git_status(&repo)?, @" M embedded-repository");
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
100644:51f8807 .gitmodules
160000:a047f81 embedded-repository
160000:a047f81 submodule
");

    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
.
├── .git:40755
├── .gitmodules:100644
├── embedded-repository:40755
│   ├── .git:40755
│   └── file:100644
└── submodule:40755
    ├── .git:100644
    └── file:100644
");
    Ok(())
}

// See `modified_submodule_and_embedded_repo_in_worktree` for details
#[test]
#[cfg(unix)]
fn modified_submodule_and_embedded_repo_in_index() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("modified-submodule-and-embedded-repo");
    git(&repo).args(["add", "."]).run();
    insta::assert_snapshot!(git_status(&repo)?, @r"
M  embedded-repository
MM submodule
");
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
100644:51f8807 .gitmodules
160000:6d5e0a5 embedded-repository
160000:6d5e0a5 submodule
");

    let dropped = discard_workspace_changes(
        &repo,
        worktree_changes_to_discard_specs(&repo),
        CONTEXT_LINES,
    )?;
    assert!(dropped.is_empty());

    // `gix status` is able to see the 'embedded-repository' if it's in the index, and we can reset it as well.
    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
100644:51f8807 .gitmodules
160000:a047f81 embedded-repository
160000:a047f81 submodule
");

    Ok(())
}

#[test]
#[cfg(unix)]
fn all_file_types_renamed_and_modified_in_worktree() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("all-file-types-renamed-and-modified");
    // Git doesn't detect renames between index/worktree, but we do.
    insta::assert_snapshot!(git_status(&repo)?, @r"
 D executable
 D file
 D link
?? executable-renamed
?? file-renamed
?? link-renamed
");
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
.
├── .git:40755
├── executable-renamed:100755
├── fifo-should-be-ignored:10644
├── file-renamed:100644
└── link-renamed:120755
");

    let dropped = discard_workspace_changes(
        &repo,
        worktree_changes_to_discard_specs(&repo),
        CONTEXT_LINES,
    )?;
    assert!(dropped.is_empty());

    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
100755:01e79c3 executable
100644:3aac70f file
120000:c4c364c link
");

    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
.
├── .git:40755
├── executable:100755
├── fifo-should-be-ignored:10644
├── file:100644
└── link:120755
");
    Ok(())
}

#[test]
#[cfg(unix)]
fn all_file_types_renamed_modified_in_index() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("all-file-types-renamed-and-modified");
    git(&repo).args(["add", "."]).run();
    insta::assert_snapshot!(git_status(&repo)?, @r"
R  executable -> executable-renamed
R  file -> file-renamed
D  link
A  link-renamed
");
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
    100755:8a1218a executable-renamed
    100644:c5c4315 file-renamed
    120000:94e4e07 link-renamed
    ");
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
.
├── .git:40755
├── executable-renamed:100755
├── fifo-should-be-ignored:10644
├── file-renamed:100644
└── link-renamed:120755
");

    let dropped = discard_workspace_changes(
        &repo,
        worktree_changes_to_discard_specs(&repo),
        CONTEXT_LINES,
    )?;
    assert!(dropped.is_empty());

    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
100755:01e79c3 executable
100644:3aac70f file
120000:c4c364c link
");

    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
.
├── .git:40755
├── executable:100755
├── fifo-should-be-ignored:10644
├── file:100644
└── link:120755
");
    Ok(())
}

#[test]
#[cfg(unix)]
fn all_file_types_renamed_overwriting_existing_and_modified_in_worktree() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("all-file-types-renamed-and-overwriting-existing");
    // This is actually misleading as `file-to-be-dir` seems missing even though it's now
    // a directory. It's untracked-state isn't visible.
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

    // `gix status` shows it like one would expect, but it can't detect renames here due to a shortcoming
    // inherited from Git.
    //   R executable → dir-to-be-file
    //   D dir-to-be-file/content
    //   D file-to-be-dir
    //   R file → file-to-be-dir/file
    //   D link
    //   ? link-renamed
    //   D other-file
    //   M to-be-overwritten
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
.
├── .git:40755
├── dir-to-be-file:100755
├── file-to-be-dir:40755
│   └── file:100644
├── link-renamed:120755
└── to-be-overwritten:100644
");

    let dropped = discard_workspace_changes(
        &repo,
        worktree_changes_to_discard_specs(&repo),
        CONTEXT_LINES,
    )?;
    assert!(dropped.is_empty());

    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
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
├── dir-to-be-file:40755
│   └── content:100644
├── executable:100755
├── file:100644
├── file-to-be-dir:100644
├── link:120755
├── other-file:100644
└── to-be-overwritten:100644
");
    Ok(())
}

#[test]
#[cfg(unix)]
fn all_file_types_renamed_overwriting_existing_and_modified_in_index() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("all-file-types-renamed-and-overwriting-existing");
    git(&repo).args(["add", "."]).run();
    // This is actually misleading as `file-to-be-dir` seems missing even though it's now
    // a directory. It's untracked-state isn't visible.
    insta::assert_snapshot!(git_status(&repo)?, @r"
R  executable -> dir-to-be-file
D  dir-to-be-file/content
D  file-to-be-dir
R  file -> file-to-be-dir/file
D  link
A  link-renamed
D  other-file
M  to-be-overwritten
");

    // `gix status` shows it like one would expect, but it can't detect renames here due to a shortcoming
    // inherited from Git.
    //  M  to-be-overwritten
    //  R  file → file-to-be-dir/file
    //  R  executable → dir-to-be-file
    //  D  dir-to-be-file/content
    //  D  file-to-be-dir
    //  D  link
    //  A  link-renamed
    //  D  other-file
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
.
├── .git:40755
├── dir-to-be-file:100755
├── file-to-be-dir:40755
│   └── file:100644
├── link-renamed:120755
└── to-be-overwritten:100644
");

    let dropped = discard_workspace_changes(
        &repo,
        worktree_changes_to_discard_specs(&repo),
        CONTEXT_LINES,
    )?;
    assert!(dropped.is_empty());

    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
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
├── dir-to-be-file:40755
│   └── content:100644
├── executable:100755
├── file:100644
├── file-to-be-dir:100644
├── link:120755
├── other-file:100644
└── to-be-overwritten:100644
");
    Ok(())
}

/// like `all_file_types_renamed_overwriting_existing_and_modified_in_worktree()`, but discards
/// only the files the user currently sees in the worktree
#[test]
#[cfg(unix)]
fn all_file_types_renamed_overwriting_existing_and_modified_in_worktree_discard_selectively()
-> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("all-file-types-renamed-and-overwriting-existing");
    // This is actually misleading as `file-to-be-dir` seems missing even though it's now
    // a directory. It's untracked-state isn't visible.
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

    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
.
├── .git:40755
├── dir-to-be-file:100755
├── file-to-be-dir:40755
│   └── file:100644
├── link-renamed:120755
└── to-be-overwritten:100644
");

    let dropped = discard_workspace_changes(
        &repo,
        [
            file_to_spec("link-renamed"),
            file_to_spec("link"),
            file_to_spec("other-file"),
            file_to_spec("to-be-overwritten"),
            renamed_file_to_spec("executable", "dir-to-be-file"),
            renamed_file_to_spec("file", "file-to-be-dir/file"),
        ],
        CONTEXT_LINES,
    )?;
    assert_eq!(dropped, []);

    // This is a shortcoming of the data we have available, which effectively prevents us
    // from undoing the rename in one go.
    // What we did is purge the destination of the rename, which had no corresponding tree or index entry,
    // leaving an empty directory.
    // In case of the executable, we see only what would be a directory in the index, so we end up restoring
    // nothing either.
    // This could be improved at some cost, so let's go with the two-step process for now.
    insta::assert_snapshot!(git_status(&repo)?, @r"
D dir-to-be-file/content
D file-to-be-dir
");
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
.
├── .git:40755
├── executable:100755
├── file:100644
├── file-to-be-dir:40755
├── link:120755
├── other-file:100644
└── to-be-overwritten:100644
");

    // Try again with what remains, something that the user will likely do as well, not really knowing
    // why that is.
    let dropped = discard_workspace_changes(
        &repo,
        worktree_changes_to_discard_specs(&repo),
        CONTEXT_LINES,
    )?;
    assert!(dropped.is_empty());

    insta::assert_snapshot!(git_status(&repo)?, @r"");

    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
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
├── dir-to-be-file:40755
│   └── content:100644
├── executable:100755
├── file:100644
├── file-to-be-dir:100644
├── link:120755
├── other-file:100644
└── to-be-overwritten:100644
");
    Ok(())
}

#[test]
#[cfg(unix)]
fn folder_with_all_file_types_moved_upwards_in_worktree() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("move-directory-into-sibling-file");
    insta::assert_snapshot!(git_status(&repo)?, @r"
D a/b/executable
D a/b/file
D a/b/link
D a/sibling
");
    // For `gitoxide` this looks like this:
    //   D a/sibling
    //   R a/b/executable → a/sibling/executable
    //   R a/b/file → a/sibling/file
    //   R a/b/link → a/sibling/link

    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
.
├── .git:40755
└── a:40755
    └── sibling:40755
        ├── executable:100755
        ├── file:100644
        └── link:120755
");

    // This naturally starts with `a/sibling`
    let dropped = discard_workspace_changes(
        &repo,
        worktree_changes_to_discard_specs(&repo),
        CONTEXT_LINES,
    )?;
    assert!(dropped.is_empty());

    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
100755:01e79c3 a/b/executable
100644:3aac70f a/b/file
120000:c4c364c a/b/link
100644:a0d4277 a/sibling
");
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
.
├── .git:40755
└── a:40755
    ├── b:40755
    │   ├── executable:100755
    │   ├── file:100644
    │   └── link:120755
    └── sibling:100644
");
    Ok(())
}

#[test]
#[cfg(unix)]
fn folder_with_all_file_types_moved_upwards_in_worktree_discard_selected() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("move-directory-into-sibling-file");
    insta::assert_snapshot!(git_status(&repo)?, @r"
D a/b/executable
D a/b/file
D a/b/link
D a/sibling
");
    // For `gitoxide` this looks like this:
    //   D a/sibling
    //   R a/b/executable → a/sibling/executable
    //   R a/b/file → a/sibling/file
    //   R a/b/link → a/sibling/link

    // Discard in inverse order
    let dropped = discard_workspace_changes(
        &repo,
        [
            renamed_file_to_spec("a/b/executable", "a/sibling/executable"),
            renamed_file_to_spec("a/b/file", "a/sibling/file"),
            renamed_file_to_spec("a/b/link", "a/sibling/link"),
            file_to_spec("a/sibling"),
        ],
        CONTEXT_LINES,
    )?;
    assert!(dropped.is_empty());

    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
100755:01e79c3 a/b/executable
100644:3aac70f a/b/file
120000:c4c364c a/b/link
100644:a0d4277 a/sibling
");
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
.
├── .git:40755
└── a:40755
    ├── b:40755
    │   ├── executable:100755
    │   ├── file:100644
    │   └── link:120755
    └── sibling:100644
");
    Ok(())
}

#[test]
#[cfg(unix)]
fn folder_with_all_file_types_moved_upwards_in_index() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("move-directory-into-sibling-file");
    git(&repo).args(["add", "."]).run();
    insta::assert_snapshot!(git_status(&repo)?, @r"
D  a/sibling
R  a/b/executable -> a/sibling/executable
R  a/b/file -> a/sibling/file
R  a/b/link -> a/sibling/link
");

    let dropped = discard_workspace_changes(
        &repo,
        worktree_changes_to_discard_specs(&repo),
        CONTEXT_LINES,
    )?;
    assert!(dropped.is_empty());

    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
100755:01e79c3 a/b/executable
100644:3aac70f a/b/file
120000:c4c364c a/b/link
100644:a0d4277 a/sibling
");
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
.
├── .git:40755
└── a:40755
    ├── b:40755
    │   ├── executable:100755
    │   ├── file:100644
    │   └── link:120755
    └── sibling:100644
");
    Ok(())
}

// Copy of `all_file_types_deleted_in_worktree`, could also be a loop but insta::allow_duplicates!() isn't pretty.
#[test]
#[cfg(unix)]
fn all_file_types_deleted_in_index() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("delete-all-file-types-valid-submodule");
    insta::assert_snapshot!(git_status(&repo)?, @r"
D .gitmodules
D executable
D link
D submodule
");
    git(&repo).args(["add", "."]).run();

    let dropped = discard_workspace_changes(
        &repo,
        worktree_changes_to_discard_specs(&repo),
        CONTEXT_LINES,
    )?;
    assert!(dropped.is_empty());

    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
100644:51f8807 .gitmodules
160000:a047f81 embedded-repository
100755:86daf54 executable
100644:d95f3ad file-to-remain
120000:b158162 link
160000:a047f81 submodule
");

    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
.
├── .git:40755
├── .gitmodules:100644
├── embedded-repository:40755
│   ├── .git:40755
│   └── file:100644
├── executable:100755
├── file-to-remain:100644
├── link:120755
└── submodule:40755
    ├── .git:100644
    └── file:100644
");
    insta::assert_snapshot!(
        std::fs::read_to_string(repo.workdir_path("submodule/.git").unwrap())
            .expect("file can be read"),
        @"gitdir: ../.git/modules/submodule"
    );
    Ok(())
}

mod util {
    use crate::utils::to_change_specs_whole_file;
    use but_core::DiffSpec;

    pub fn file_to_spec(name: &str) -> DiffSpec {
        DiffSpec {
            previous_path: None,
            path: name.into(),
            hunk_headers: vec![],
        }
    }

    pub fn renamed_file_to_spec(previous: &str, name: &str) -> DiffSpec {
        DiffSpec {
            previous_path: Some(previous.into()),
            path: name.into(),
            hunk_headers: vec![],
        }
    }

    pub fn worktree_changes_to_discard_specs(
        repo: &gix::Repository,
    ) -> impl Iterator<Item = DiffSpec> {
        to_change_specs_whole_file(
            but_core::diff::worktree_changes(repo).expect("worktree changes never fail"),
        )
        .into_iter()
    }
}
