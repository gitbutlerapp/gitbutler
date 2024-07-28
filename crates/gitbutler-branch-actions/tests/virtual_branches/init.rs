use super::*;

#[test]
fn twice() {
    let data_dir = paths::data_dir();
    let projects = projects::Controller::from_path(data_dir.path());

    let test_project = TestProject::default();

    let controller = VirtualBranchActions {};

    {
        let project = projects
            .add(test_project.path())
            .expect("failed to add project");
        controller
            .set_base_branch(&project, &"refs/remotes/origin/master".parse().unwrap())
            .unwrap();
        assert!(controller
            .list_virtual_branches(&project)
            .unwrap()
            .0
            .is_empty());
        projects.delete(project.id).unwrap();
        controller.list_virtual_branches(&project).unwrap_err();
    }

    {
        let project = projects.add(test_project.path()).unwrap();
        controller
            .set_base_branch(&project, &"refs/remotes/origin/master".parse().unwrap())
            .unwrap();

        // even though project is on gitbutler/integration, we should not import it
        assert!(controller
            .list_virtual_branches(&project)
            .unwrap()
            .0
            .is_empty());
    }
}

#[test]
fn dirty_non_target() {
    // a situation when you initialize project while being on the local verison of the master
    // that has uncommited changes.
    let Test {
        repository,
        project,
        controller,
        ..
    } = &Test::default();

    repository.checkout(&"refs/heads/some-feature".parse().unwrap());

    fs::write(repository.path().join("file.txt"), "content").unwrap();

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].files.len(), 1);
    assert_eq!(branches[0].files[0].hunks.len(), 1);
    assert!(branches[0].upstream.is_none());
    assert_eq!(branches[0].name, "some-feature");
}

#[test]
fn dirty_target() {
    // a situation when you initialize project while being on the local verison of the master
    // that has uncommited changes.
    let Test {
        repository,
        project,
        controller,
        ..
    } = &Test::default();

    fs::write(repository.path().join("file.txt"), "content").unwrap();

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].files.len(), 1);
    assert_eq!(branches[0].files[0].hunks.len(), 1);
    assert!(branches[0].upstream.is_none());
    assert_eq!(branches[0].name, "master");
}

#[test]
fn commit_on_non_target_local() {
    let Test {
        repository,
        project,
        controller,
        ..
    } = &Test::default();

    repository.checkout(&"refs/heads/some-feature".parse().unwrap());
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    repository.commit_all("commit on target");

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 1);
    assert!(branches[0].files.is_empty());
    assert_eq!(branches[0].commits.len(), 1);
    assert!(branches[0].upstream.is_none());
    assert_eq!(branches[0].name, "some-feature");
}

#[test]
fn commit_on_non_target_remote() {
    let Test {
        repository,
        project,
        controller,
        ..
    } = &Test::default();

    repository.checkout(&"refs/heads/some-feature".parse().unwrap());
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    repository.commit_all("commit on target");
    repository.push_branch(&"refs/heads/some-feature".parse().unwrap());

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 1);
    assert!(branches[0].files.is_empty());
    assert_eq!(branches[0].commits.len(), 1);
    assert!(branches[0].upstream.is_some());
    assert_eq!(branches[0].name, "some-feature");
}

#[test]
fn commit_on_target() {
    let Test {
        repository,
        project,
        controller,
        ..
    } = &Test::default();

    fs::write(repository.path().join("file.txt"), "content").unwrap();
    repository.commit_all("commit on target");

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 1);
    assert!(branches[0].files.is_empty());
    assert_eq!(branches[0].commits.len(), 1);
    assert!(branches[0].upstream.is_none());
    assert_eq!(branches[0].name, "master");
}

#[test]
fn submodule() {
    let Test {
        repository,
        project,
        controller,
        ..
    } = &Test::default();

    let test_project = TestProject::default();
    let submodule_url: gitbutler_url::Url =
        test_project.path().display().to_string().parse().unwrap();
    repository.add_submodule(&submodule_url, path::Path::new("submodule"));

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].files.len(), 1);
    assert_eq!(branches[0].files[0].hunks.len(), 1);
}
