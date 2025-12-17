use gitbutler_testsupport::paths;

#[test]
fn current_directory_dot() -> anyhow::Result<()> {
    let tmp = paths::data_dir();
    let repository = gitbutler_testsupport::TestProject::default();
    let repo_path = repository.path();

    // Change to the repository directory and add "." as the path
    std::env::set_current_dir(repo_path)?;

    let project = gitbutler_project::add_at_app_data_dir(tmp.path(), ".")?.unwrap_project();

    let expected_title = repo_path.file_name().unwrap().to_str().unwrap();
    assert_eq!(
        project.title, expected_title,
        "Project title should be the actual directory name, not '.'"
    );
    Ok(())
}
