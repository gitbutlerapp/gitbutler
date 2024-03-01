use crate::{
    gb_repository, git, project_repository, projects,
    tests::common::{paths, TestProject},
};
use std::path;

mod init {
    use super::*;

    #[test]
    fn handle_file_symlink() {
        let test_project = TestProject::default();

        let data_dir = paths::data_dir();
        let projects = projects::Controller::try_from(&data_dir).unwrap();

        let project = projects
            .add(test_project.path())
            .expect("failed to add project");

        std::fs::write(project.path.join("file"), "content").unwrap();
        std::fs::hard_link(project.path.join("file"), project.path.join("link")).unwrap();

        let project_repository = project_repository::Repository::open(&project).unwrap();

        gb_repository::Repository::open(&data_dir, &project_repository, None).unwrap();
    }

    #[test]
    fn handle_dir_symlink() {
        let test_project = TestProject::default();

        let data_dir = paths::data_dir();
        let projects = projects::Controller::try_from(&data_dir).unwrap();

        let project = projects
            .add(test_project.path())
            .expect("failed to add project");

        std::fs::create_dir_all(project.path.join("dir")).unwrap();
        std::fs::write(project.path.join("dir/file"), "content").unwrap();
        std::os::unix::fs::symlink(project.path.join("dir"), project.path.join("dir_link"))
            .unwrap();

        let project_repository = project_repository::Repository::open(&project).unwrap();

        gb_repository::Repository::open(&data_dir, &project_repository, None).unwrap();
    }

    #[test]
    fn handle_dir_symlink_symlink() {
        let test_project = TestProject::default();

        let data_dir = paths::data_dir();
        let projects = projects::Controller::try_from(&data_dir).unwrap();

        let project = projects
            .add(test_project.path())
            .expect("failed to add project");

        std::fs::create_dir_all(project.path.join("dir")).unwrap();
        std::fs::write(project.path.join("dir/file"), "content").unwrap();
        std::os::unix::fs::symlink(project.path.join("dir"), project.path.join("dir_link"))
            .unwrap();
        std::os::unix::fs::symlink(
            project.path.join("dir_link"),
            project.path.join("link_link"),
        )
        .unwrap();

        let project_repository = project_repository::Repository::open(&project).unwrap();

        gb_repository::Repository::open(&data_dir, &project_repository, None).unwrap();
    }
}

mod flush {
    use super::*;

    #[test]
    fn handle_file_symlink() {
        let test_project = TestProject::default();

        let data_dir = paths::data_dir();
        let projects = projects::Controller::try_from(&data_dir).unwrap();

        let project = projects
            .add(test_project.path())
            .expect("failed to add project");

        let project_repository = project_repository::Repository::open(&project).unwrap();

        let gb_repo =
            gb_repository::Repository::open(&data_dir, &project_repository, None).unwrap();

        std::fs::write(project.path.join("file"), "content").unwrap();
        std::fs::hard_link(project.path.join("file"), project.path.join("link")).unwrap();

        gb_repo.flush(&project_repository, None).unwrap();
    }

    #[test]
    fn handle_dir_symlink() {
        let test_project = TestProject::default();

        let data_dir = paths::data_dir();
        let projects = projects::Controller::try_from(&data_dir).unwrap();

        let project = projects
            .add(test_project.path())
            .expect("failed to add project");

        let project_repository = project_repository::Repository::open(&project).unwrap();

        let gb_repo =
            gb_repository::Repository::open(&data_dir, &project_repository, None).unwrap();

        std::fs::create_dir_all(project.path.join("dir")).unwrap();
        std::fs::write(project.path.join("dir/file"), "content").unwrap();
        std::os::unix::fs::symlink(project.path.join("dir"), project.path.join("dir_link"))
            .unwrap();

        gb_repo.flush(&project_repository, None).unwrap();
    }

    #[test]
    fn handle_submodules() {
        let test_project = TestProject::default();

        let data_dir = paths::data_dir();
        let projects = projects::Controller::try_from(&data_dir).unwrap();

        let project = projects
            .add(test_project.path())
            .expect("failed to add project");

        let project_repository = project_repository::Repository::open(&project).unwrap();

        let gb_repo =
            gb_repository::Repository::open(&data_dir, &project_repository, None).unwrap();

        let submodule_url: git::Url = TestProject::default()
            .path()
            .display()
            .to_string()
            .parse()
            .unwrap();
        test_project.add_submodule(&submodule_url, path::Path::new("submodule"));

        gb_repo.flush(&project_repository, None).unwrap();
    }
}
