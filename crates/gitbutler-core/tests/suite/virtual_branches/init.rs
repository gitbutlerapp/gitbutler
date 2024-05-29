use super::*;

#[tokio::test]
async fn twice() {
    let data_dir = paths::data_dir();
    let projects = projects::Controller::from_path(data_dir.path());
    let users = users::Controller::from_path(data_dir.path());
    let helper = git::credentials::Helper::from_path(data_dir.path());

    let test_project = TestProject::default();

    let controller = Controller::new(projects.clone(), users, helper);

    {
        let project = projects
            .add(test_project.path())
            .expect("failed to add project");
        controller
            .set_base_branch(project.id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();
        assert!(controller
            .list_virtual_branches(project.id)
            .await
            .unwrap()
            .0
            .is_empty());
        projects.delete(project.id).await.unwrap();
        controller
            .list_virtual_branches(project.id)
            .await
            .unwrap_err();
    }

    {
        let project = projects.add(test_project.path()).unwrap();
        controller
            .set_base_branch(project.id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        // even though project is on gitbutler/integration, we should not import it
        assert!(controller
            .list_virtual_branches(project.id)
            .await
            .unwrap()
            .0
            .is_empty());
    }
}

#[tokio::test]
async fn dirty_non_target() {
    // a situation when you initialize project while being on the local verison of the master
    // that has uncommited changes.
    let Test {
        repository,
        project_id,
        controller,
        ..
    } = &Test::default();

    repository.checkout(&"refs/heads/some-feature".parse().unwrap());

    fs::write(repository.path().join("file.txt"), "content").unwrap();

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].files.len(), 1);
    assert_eq!(branches[0].files[0].hunks.len(), 1);
    assert!(branches[0].upstream.is_none());
    assert_eq!(branches[0].name, "some-feature");
}

#[tokio::test]
async fn dirty_target() {
    // a situation when you initialize project while being on the local verison of the master
    // that has uncommited changes.
    let Test {
        repository,
        project_id,
        controller,
        ..
    } = &Test::default();

    fs::write(repository.path().join("file.txt"), "content").unwrap();

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].files.len(), 1);
    assert_eq!(branches[0].files[0].hunks.len(), 1);
    assert!(branches[0].upstream.is_none());
    assert_eq!(branches[0].name, "master");
}

#[tokio::test]
async fn commit_on_non_target_local() {
    let Test {
        repository,
        project_id,
        controller,
        ..
    } = &Test::default();

    repository.checkout(&"refs/heads/some-feature".parse().unwrap());
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    repository.commit_all("commit on target");

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
    assert_eq!(branches.len(), 1);
    assert!(branches[0].files.is_empty());
    assert_eq!(branches[0].commits.len(), 1);
    assert!(branches[0].upstream.is_none());
    assert_eq!(branches[0].name, "some-feature");
}

#[tokio::test]
async fn commit_on_non_target_remote() {
    let Test {
        repository,
        project_id,
        controller,
        ..
    } = &Test::default();

    repository.checkout(&"refs/heads/some-feature".parse().unwrap());
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    repository.commit_all("commit on target");
    repository.push_branch(&"refs/heads/some-feature".parse().unwrap());

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
    assert_eq!(branches.len(), 1);
    assert!(branches[0].files.is_empty());
    assert_eq!(branches[0].commits.len(), 1);
    assert!(branches[0].upstream.is_some());
    assert_eq!(branches[0].name, "some-feature");
}

#[tokio::test]
async fn commit_on_target() {
    let Test {
        repository,
        project_id,
        controller,
        ..
    } = &Test::default();

    fs::write(repository.path().join("file.txt"), "content").unwrap();
    repository.commit_all("commit on target");

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
    assert_eq!(branches.len(), 1);
    assert!(branches[0].files.is_empty());
    assert_eq!(branches[0].commits.len(), 1);
    assert!(branches[0].upstream.is_none());
    assert_eq!(branches[0].name, "master");
}

#[tokio::test]
async fn submodule() {
    let Test {
        repository,
        project_id,
        controller,
        ..
    } = &Test::default();

    let project = TestProject::default();
    let submodule_url: git::Url = project.path().display().to_string().parse().unwrap();
    repository.add_submodule(&submodule_url, path::Path::new("submodule"));

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].files.len(), 1);
    assert_eq!(branches[0].files[0].hunks.len(), 1);
}
