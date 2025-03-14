use crate::discard::util::worktree_changes_to_discard_specs;
use crate::utils::{visualize_index, writable_scenario, writable_scenario_slow};
use but_testsupport::{CommandExt, git, git_status, visualize_disk_tree_skip_dot_git};
use but_workspace::discard_workspace_changes;

#[test]
fn all_file_types_from_unborn() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("unborn-untracked-all-file-types");
    insta::assert_snapshot!(git_status(&repo)?, @r"
    ?? link
    ?? untracked
    ?? untracked-exe
    ");

    let dropped = discard_workspace_changes(&repo, worktree_changes_to_discard_specs(&repo))?;
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

    let dropped = discard_workspace_changes(&repo, worktree_changes_to_discard_specs(&repo))?;
    assert!(dropped.is_empty());

    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"");
    Ok(())
}

#[test]
#[cfg(unix)]
fn all_file_types_deleted_in_worktree() -> anyhow::Result<()> {
    util::control_umask();
    let (repo, _tmp) = writable_scenario("delete-all-file-types-valid-submodule");
    insta::assert_snapshot!(git_status(&repo)?, @r"
    D .gitmodules
    D executable
    D link
    D submodule
    ");

    let dropped = discard_workspace_changes(&repo, worktree_changes_to_discard_specs(&repo))?;
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
    util::control_umask();
    let (repo, _tmp) = writable_scenario("replace-dir-with-submodule-with-file");
    insta::assert_snapshot!(git_status(&repo)?, @r"
     D dir/executable
     D dir/file-to-remain
     D dir/link
     D dir/submodule
    ?? dir
    ");

    let dropped = discard_workspace_changes(&repo, worktree_changes_to_discard_specs(&repo))?;
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
    util::control_umask();
    let (repo, _tmp) = writable_scenario("replace-dir-with-submodule-with-file");
    git(&repo).args(["add", "."]).run();
    insta::assert_snapshot!(git_status(&repo)?, @r"
    A  dir
    D  dir/executable
    D  dir/file-to-remain
    D  dir/link
    D  dir/submodule
    ");

    let dropped = discard_workspace_changes(&repo, worktree_changes_to_discard_specs(&repo))?;
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
    util::control_umask();
    let (repo, _tmp) = writable_scenario("replace-dir-with-submodule-with-file");
    insta::assert_snapshot!(git_status(&repo)?, @r"
     D dir/executable
     D dir/file-to-remain
     D dir/link
     D dir/submodule
    ?? dir
    ");

    let dropped = discard_workspace_changes(&repo, Some(util::file_to_spec("dir")))?;
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
fn replace_dir_with_file_discard_just_the_file_in_index() -> anyhow::Result<()> {
    util::control_umask();
    let (repo, _tmp) = writable_scenario("replace-dir-with-submodule-with-file");
    git(&repo).args(["add", "."]).run();
    insta::assert_snapshot!(git_status(&repo)?, @r"
    A  dir
    D  dir/executable
    D  dir/file-to-remain
    D  dir/link
    D  dir/submodule
    ");

    let dropped = discard_workspace_changes(&repo, Some(util::file_to_spec("dir")))?;
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
    util::control_umask();
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

    let dropped = discard_workspace_changes(&repo, worktree_changes_to_discard_specs(&repo))?;
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
    util::control_umask();
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

    let dropped = discard_workspace_changes(&repo, worktree_changes_to_discard_specs(&repo))?;
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
    util::control_umask();
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

    let dropped = discard_workspace_changes(&repo, worktree_changes_to_discard_specs(&repo))?;
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

#[test]
#[cfg(unix)]
fn all_file_types_renamed_and_modified_in_worktree() -> anyhow::Result<()> {
    util::control_umask();
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

    let dropped = discard_workspace_changes(&repo, worktree_changes_to_discard_specs(&repo))?;
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
fn all_file_types_renamed_modified_in_index() -> anyhow::Result<()> {
    util::control_umask();
    let (repo, _tmp) = writable_scenario_slow("all-file-types-renamed-and-modified");
    git(&repo).args(["add", "."]).run();
    // Git can detect tree/index renames, and so can we.
    insta::assert_snapshot!(git_status(&repo)?, @r"
    R  executable -> executable-renamed
    R  file -> file-renamed
    D  link
    A  link-renamed
    ");
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
    .
    ├── .git:40755
    ├── executable-renamed:100755
    ├── fifo-should-be-ignored:10644
    ├── file-renamed:100644
    └── link-renamed:120755
    ");

    let dropped = discard_workspace_changes(&repo, worktree_changes_to_discard_specs(&repo))?;
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
#[ignore = "TBD - make sure it overwrites a dir with a file and a file with a dir"]
fn all_file_types_renamed_overwriting_existing_and_modified_in_worktree() -> anyhow::Result<()> {
    todo!()
}

#[test]
#[ignore = "TBD"]
fn all_file_types_renamed_overwriting_existing_and_modified_in_index() -> anyhow::Result<()> {
    todo!()
}

// See `modified_submodule_and_embedded_repo_in_worktree` for details
#[test]
#[cfg(unix)]
fn modified_submodule_and_embedded_repo_in_index() -> anyhow::Result<()> {
    util::control_umask();
    let (repo, _tmp) = writable_scenario("modified-submodule-and-embedded-repo");
    git(&repo).args(["add", "."]).run();
    insta::assert_snapshot!(git_status(&repo)?, @r"
    M  embedded-repository
    MM submodule
    ");

    let dropped = discard_workspace_changes(&repo, worktree_changes_to_discard_specs(&repo))?;
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

// Copy of `all_file_types_deleted_in_worktree`, but can't be loop due to `insta`.
#[test]
#[cfg(unix)]
fn all_file_types_deleted_in_index() -> anyhow::Result<()> {
    util::control_umask();
    let (repo, _tmp) = writable_scenario("delete-all-file-types-valid-submodule");
    insta::assert_snapshot!(git_status(&repo)?, @r"
    D .gitmodules
    D executable
    D link
    D submodule
    ");
    git(&repo).args(["add", "."]).run();

    let dropped = discard_workspace_changes(&repo, worktree_changes_to_discard_specs(&repo))?;
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
    use but_workspace::commit_engine::DiffSpec;
    use but_workspace::discard::DiscardSpec;

    pub fn file_to_spec(name: &str) -> DiscardSpec {
        DiffSpec {
            previous_path: None,
            path: name.into(),
            hunk_headers: vec![],
        }
        .into()
    }

    /// Set the process umask to a known value so filesystem listings will be as we expect on all machines.
    #[cfg(unix)]
    pub fn control_umask() {
        use rustix::fs::Mode;
        rustix::process::umask(Mode::from_bits(0o022).unwrap());
    }

    pub fn worktree_changes_to_discard_specs(
        repo: &gix::Repository,
    ) -> impl Iterator<Item = DiscardSpec> {
        to_change_specs_whole_file(
            but_core::diff::worktree_changes(repo).expect("worktree changes never fail"),
        )
        .into_iter()
        .map(Into::into)
    }
}
