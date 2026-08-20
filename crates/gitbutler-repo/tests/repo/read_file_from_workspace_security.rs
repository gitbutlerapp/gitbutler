use std::{fs, path::Path};

use crate::support::{repository, test_repository};
use but_ctx::{Context, RepoOpenMode};
use but_settings::AppSettings;
use gitbutler_project as projects;
use gitbutler_repo::RepoCommands;

fn context_for_repo(workdir: &Path) -> Context {
    let project = projects::Project::from_path(workdir).expect("valid test project");
    Context::new_from_legacy_project_and_settings_with_repo_open_mode(
        &project,
        AppSettings::default(),
        RepoOpenMode::Isolated,
    )
    .expect("can create context")
    .with_memory_app_cache()
}

#[test]
fn allows_read_inside_worktree_with_relative_path() {
    let (repo, _tmp) = test_repository();
    let workdir = repo.workdir().expect("workdir exists");
    fs::write(workdir.join("file.txt"), "hello from workspace").expect("write file");

    let ctx = context_for_repo(workdir);
    let info = ctx
        .read_file_from_workspace("file.txt".as_ref())
        .expect("read file in workspace");

    assert_eq!(info.content, Some("hello from workspace".to_owned()));
}

#[test]
fn rejects_untracked_gitignored_file() {
    let (repo, _tmp) = repository("create-wd-tree-ignored-files");
    let workdir = repo.workdir().expect("workdir exists");
    fs::write(workdir.join("secret.ignored"), "SUPER_SECRET").expect("write ignored file");

    let ctx = context_for_repo(workdir);
    ctx.read_file_from_workspace(Path::new("secret.ignored"))
        .expect_err("Git-ignored files must not be readable");
}

#[test]
fn rejects_git_metadata_file() {
    let (repo, _tmp) = test_repository();
    let workdir = repo.workdir().expect("workdir exists");

    let ctx = context_for_repo(workdir);
    ctx.read_file_from_workspace(Path::new(".git/config"))
        .expect_err("Git metadata files must not be readable");
}

#[test]
fn allows_tracked_file_matching_ignore_rule() {
    let (repo, _tmp) = repository("create-wd-tree-ignored-files");
    let workdir = repo.workdir().expect("workdir exists");
    fs::write(workdir.join(".gitignore"), "tracked\n").expect("update ignore file");

    let ctx = context_for_repo(workdir);
    let info = ctx
        .read_file_from_workspace(Path::new("tracked"))
        .expect("tracked files remain readable when they match an ignore rule");

    assert_eq!(info.content, Some("content".to_owned()));
}

#[test]
fn rejects_git_metadata_aliases() {
    let (repo, _tmp) = test_repository();
    let workdir = repo.workdir().expect("workdir exists");

    let ctx = context_for_repo(workdir);
    for path in [
        ".GIT/config",
        "GIT~1/config",
        ".git./config",
        ".git /config",
    ] {
        ctx.read_file_from_workspace(Path::new(path))
            .expect_err("Git metadata aliases must not be readable");
    }
}

#[test]
fn rejects_file_inside_ignored_directory() {
    let (repo, _tmp) = repository("create-wd-tree-ignored-files");
    let workdir = repo.workdir().expect("workdir exists");
    fs::write(workdir.join(".gitignore"), "build/\n").expect("update ignore file");
    fs::create_dir(workdir.join("build")).expect("create ignored directory");
    fs::write(workdir.join("build/out.env"), "SECRET").expect("write file in ignored dir");

    let ctx = context_for_repo(workdir);
    ctx.read_file_from_workspace(Path::new("build/out.env"))
        .expect_err("files inside an ignored directory must not be readable");
}

#[test]
fn allows_ignored_directory_as_empty_placeholder() {
    // Callers depend on directories yielding the placeholder rather than an
    // error (see error-cleanup-checklist.md).
    let (repo, _tmp) = repository("create-wd-tree-ignored-files");
    let workdir = repo.workdir().expect("workdir exists");
    fs::write(workdir.join(".gitignore"), "build/\n").expect("update ignore file");
    fs::create_dir(workdir.join("build")).expect("create ignored directory");

    let ctx = context_for_repo(workdir);
    let info = ctx
        .read_file_from_workspace(Path::new("build"))
        .expect("ignored directory still resolves to the placeholder");

    assert_eq!(info.content, Some(String::new()));
    assert_eq!(info.size, Some(0));
}

#[cfg(target_os = "macos")]
#[test]
fn allows_differently_cased_tracked_file_on_case_insensitive_fs() {
    let (repo, _tmp) = repository("create-wd-tree-ignored-files");
    let workdir = repo.workdir().expect("workdir exists");
    if !repo.filesystem_options().expect("fs options").ignore_case {
        return; // A case-sensitive volume can't exercise this branch.
    }
    fs::write(workdir.join(".gitignore"), "tracked\n").expect("update ignore file");

    let ctx = context_for_repo(workdir);
    let info = ctx
        .read_file_from_workspace(Path::new("Tracked"))
        .expect("differently-cased name of a tracked file remains readable");

    assert_eq!(info.content, Some("content".to_owned()));
}

#[test]
fn rejects_dotdot_traversal() {
    let (repo, _tmp) = test_repository();
    let workdir = repo.workdir().expect("workdir exists");
    let outside_path = workdir
        .parent()
        .expect("workdir has parent")
        .join("gitbutler-outside-secret.txt");
    fs::write(&outside_path, "outside").expect("write outside file");

    let traversal = format!(
        "../{}",
        outside_path
            .file_name()
            .expect("outside filename")
            .to_string_lossy()
    );

    let ctx = context_for_repo(workdir);
    let err = ctx
        .read_file_from_workspace(Path::new(&traversal))
        .expect_err("traversal must be rejected");

    assert!(
        err.to_string().contains("isn't in the worktree directory"),
        "{err:#}"
    );
}

#[cfg(unix)]
#[test]
fn rejects_symlink_escape() {
    let (repo, _tmp) = test_repository();
    let workdir = repo.workdir().expect("workdir exists");
    let outside_path = workdir
        .parent()
        .expect("workdir has parent")
        .join("gitbutler-symlink-target.txt");
    fs::write(&outside_path, "outside via symlink").expect("write outside file");
    gix::fs::symlink::create(&outside_path, &workdir.join("link.txt")).expect("create symlink");

    let ctx = context_for_repo(workdir);
    let err = ctx
        .read_file_from_workspace(Path::new("link.txt"))
        .expect_err("symlink escape must be rejected");

    assert!(
        err.to_string().contains("isn't in the worktree directory"),
        "{err:#}"
    );
}

#[test]
fn reads_deleted_file_from_index() {
    let (repo, _tmp) = repository("tracked-deleted-file");
    let workdir = repo.workdir().expect("workdir exists");
    fs::remove_file(workdir.join("deleted.txt")).expect("delete file from workspace");

    let ctx = context_for_repo(workdir);
    let info = ctx
        .read_file_from_workspace(Path::new("deleted.txt"))
        .expect("deleted tracked file should still be readable from index fallback");

    assert_eq!(info.content, Some("tracked content".to_owned()));
}

#[test]
fn reads_deleted_file_from_head_commit() {
    let (repo, _tmp) = repository("tracked-deleted-file");
    let workdir = repo.workdir().expect("workdir exists");
    fs::remove_file(workdir.join("deleted.txt")).expect("delete file from workspace");
    fs::remove_file(repo.path().join("index")).expect("delete index file");

    let ctx = context_for_repo(workdir);
    let info = ctx
        .read_file_from_workspace(Path::new("deleted.txt"))
        .expect("deleted tracked file should still be readable from head fallback");

    assert_eq!(info.content, Some("tracked content".to_owned()));
}

#[test]
fn returns_empty_for_directory_path() {
    // Directories on disk — including git submodules, which appear as real
    // directories in the worktree but as commit entries in the tree — should
    // not error out. Callers (conflict checks, diff viewers) depend on getting
    // a FileInfo back rather than an exception.
    let (repo, _tmp) = test_repository();
    let workdir = repo.workdir().expect("workdir exists");
    fs::create_dir(workdir.join("subdir")).expect("create directory");

    let ctx = context_for_repo(workdir);
    let info = ctx
        .read_file_from_workspace(Path::new("subdir"))
        .expect("directory path should be readable as empty FileInfo");

    assert_eq!(info.content, Some(String::new()));
    assert_eq!(info.size, Some(0));
    assert_eq!(info.mime_type, None);
}

#[test]
fn keeps_absolute_inside_worktree_behavior() {
    let (repo, _tmp) = test_repository();
    let workdir = repo.workdir().expect("workdir exists");
    let abs_path = workdir.join("absolute.txt");
    fs::write(&abs_path, "absolute read").expect("write file");

    let ctx = context_for_repo(workdir);
    let info = ctx
        .read_file_from_workspace(&abs_path)
        .expect("absolute in-worktree path should be readable");

    assert_eq!(info.content, Some("absolute read".to_owned()));
}

#[cfg(unix)]
#[test]
fn rejects_symlink_to_ignored_file() {
    let (repo, _tmp) = repository("create-wd-tree-ignored-files");
    let workdir = repo.workdir().expect("workdir exists");
    fs::write(workdir.join("secret.ignored"), "SUPER_SECRET").expect("write ignored file");
    gix::fs::symlink::create(Path::new("secret.ignored"), &workdir.join("innocent.txt"))
        .expect("create symlink");

    let ctx = context_for_repo(workdir);
    ctx.read_file_from_workspace(Path::new("innocent.txt"))
        .expect_err("a symlink must not launder an ignored file's content");
}

#[cfg(unix)]
#[test]
fn allows_ignored_symlink_to_non_ignored_file() {
    // Ignore rules match the resolved target, not the link's name: the content
    // behind `link.ignored` is `tracked`'s, which is readable directly anyway.
    let (repo, _tmp) = repository("create-wd-tree-ignored-files");
    let workdir = repo.workdir().expect("workdir exists");
    gix::fs::symlink::create(Path::new("tracked"), &workdir.join("link.ignored"))
        .expect("create symlink");

    let ctx = context_for_repo(workdir);
    let info = ctx
        .read_file_from_workspace(Path::new("link.ignored"))
        .expect("ignored-named symlink to readable content stays readable");

    assert_eq!(info.content, Some("content".to_owned()));
}
