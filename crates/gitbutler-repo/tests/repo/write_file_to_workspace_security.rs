use std::{fs, path::Path};

use crate::support::test_repository;
use but_ctx::{Context, RepoOpenMode};
use but_settings::AppSettings;
use gitbutler_project as projects;
use gitbutler_repo::RepoCommands;

fn context_for_repo(workdir: &Path) -> Context {
    let project = projects::Project::new_for_gitbutler_repo(workdir.to_path_buf());
    Context::new_from_legacy_project_and_settings_with_repo_open_mode(
        &project,
        AppSettings::default(),
        RepoOpenMode::Isolated,
    )
    .expect("can create context")
    .with_memory_app_cache()
}

#[test]
fn allows_write_inside_worktree_with_relative_path() {
    let (repo, _tmp) = test_repository();
    let workdir = repo.workdir().expect("workdir exists");

    let ctx = context_for_repo(workdir);
    ctx.write_file_to_workspace(Path::new("Assets/Scenes/dealers.unity"), b"scene contents")
        .expect("write inside workspace");

    let written = fs::read_to_string(workdir.join("Assets").join("Scenes").join("dealers.unity"))
        .expect("read written file");
    assert_eq!(written, "scene contents");
}

#[test]
fn rejects_dotdot_traversal() {
    let (repo, _tmp) = test_repository();
    let workdir = repo.workdir().expect("workdir exists");
    let outside_path = workdir
        .parent()
        .expect("workdir has parent")
        .join("gitbutler-outside-write.txt");
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
        .write_file_to_workspace(Path::new(&traversal), b"overwritten")
        .expect_err("traversal must be rejected");

    assert!(
        err.to_string().contains("isn't in the worktree directory"),
        "{err:#}"
    );

    let outside = fs::read_to_string(outside_path).expect("outside file unchanged");
    assert_eq!(outside, "outside");
}
