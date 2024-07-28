use super::*;

mod create_virtual_branch {
    use super::*;
    use gitbutler_branch::BranchCreateRequest;

    #[test]
    fn simple() {
        let Test {
            project,
            controller,
            repository,
            ..
        } = &Test::default();

        controller
            .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(project, &BranchCreateRequest::default())
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(project).unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch_id);
        assert_eq!(branches[0].name, "Virtual branch");

        let refnames = repository
            .references()
            .into_iter()
            .filter_map(|reference| reference.name().map(|name| name.to_string()))
            .collect::<Vec<_>>();
        assert!(refnames.contains(&"refs/gitbutler/Virtual-branch".to_string()));
    }

    #[test]
    fn duplicate_name() {
        let Test {
            project,
            controller,
            repository,
            ..
        } = &Test::default();

        controller
            .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
            .unwrap();

        let branch1_id = controller
            .create_virtual_branch(
                project,
                &BranchCreateRequest {
                    name: Some("name".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();

        let branch2_id = controller
            .create_virtual_branch(
                project,
                &BranchCreateRequest {
                    name: Some("name".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(project).unwrap();
        assert_eq!(branches.len(), 2);
        assert_eq!(branches[0].id, branch1_id);
        assert_eq!(branches[0].name, "name");
        assert_eq!(branches[1].id, branch2_id);
        assert_eq!(branches[1].name, "name 1");

        let refnames = repository
            .references()
            .into_iter()
            .filter_map(|reference| reference.name().map(|name| name.to_string()))
            .collect::<Vec<_>>();
        assert!(refnames.contains(&"refs/gitbutler/name".to_string()));
        assert!(refnames.contains(&"refs/gitbutler/name-1".to_string()));
    }
}

mod update_virtual_branch {
    use super::*;
    use gitbutler_branch::{BranchCreateRequest, BranchUpdateRequest};

    #[test]
    fn simple() {
        let Test {
            project,
            controller,
            repository,
            ..
        } = &Test::default();

        controller
            .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(
                project,
                &BranchCreateRequest {
                    name: Some("name".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();

        controller
            .update_virtual_branch(
                project,
                BranchUpdateRequest {
                    id: branch_id,
                    name: Some("new name".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(project).unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch_id);
        assert_eq!(branches[0].name, "new name");

        let refnames = repository
            .references()
            .into_iter()
            .filter_map(|reference| reference.name().map(|name| name.to_string()))
            .collect::<Vec<_>>();
        assert!(!refnames.contains(&"refs/gitbutler/name".to_string()));
        assert!(refnames.contains(&"refs/gitbutler/new-name".to_string()));
    }

    #[test]
    fn duplicate_name() {
        let Test {
            project,
            controller,
            repository,
            ..
        } = &Test::default();

        controller
            .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
            .unwrap();

        let branch1_id = controller
            .create_virtual_branch(
                project,
                &BranchCreateRequest {
                    name: Some("name".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();

        let branch2_id = controller
            .create_virtual_branch(
                project,
                &BranchCreateRequest {
                    ..Default::default()
                },
            )
            .unwrap();

        controller
            .update_virtual_branch(
                project,
                BranchUpdateRequest {
                    id: branch2_id,
                    name: Some("name".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(project).unwrap();
        assert_eq!(branches.len(), 2);
        assert_eq!(branches[0].id, branch1_id);
        assert_eq!(branches[0].name, "name");
        assert_eq!(branches[1].id, branch2_id);
        assert_eq!(branches[1].name, "name 1");

        let refnames = repository
            .references()
            .into_iter()
            .filter_map(|reference| reference.name().map(|name| name.to_string()))
            .collect::<Vec<_>>();
        assert!(refnames.contains(&"refs/gitbutler/name".to_string()));
        assert!(refnames.contains(&"refs/gitbutler/name-1".to_string()));
    }
}

mod push_virtual_branch {
    use super::*;
    use gitbutler_branch::{BranchCreateRequest, BranchUpdateRequest};

    #[test]
    fn simple() {
        let Test {
            project,
            controller,
            repository,
            ..
        } = &Test::default();

        controller
            .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
            .unwrap();

        let branch1_id = controller
            .create_virtual_branch(
                project,
                &BranchCreateRequest {
                    name: Some("name".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();

        fs::write(repository.path().join("file.txt"), "content").unwrap();

        controller
            .create_commit(project, branch1_id, "test", None, false)
            .unwrap();
        controller
            .push_virtual_branch(project, branch1_id, false, None)
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(project).unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert_eq!(branches[0].name, "name");
        assert_eq!(
            branches[0].upstream.as_ref().unwrap().name.to_string(),
            "refs/remotes/origin/name"
        );

        let refnames = repository
            .references()
            .into_iter()
            .filter_map(|reference| reference.name().map(|name| name.to_string()))
            .collect::<Vec<_>>();
        assert!(refnames.contains(&branches[0].upstream.clone().unwrap().name.to_string()));
    }

    #[test]
    fn duplicate_names() {
        let Test {
            project,
            controller,
            repository,
            ..
        } = &Test::default();

        controller
            .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
            .unwrap();

        let branch1_id = {
            // create and push branch with some work
            let branch1_id = controller
                .create_virtual_branch(
                    project,
                    &BranchCreateRequest {
                        name: Some("name".to_string()),
                        ..Default::default()
                    },
                )
                .unwrap();
            fs::write(repository.path().join("file.txt"), "content").unwrap();
            controller
                .create_commit(project, branch1_id, "test", None, false)
                .unwrap();
            controller
                .push_virtual_branch(project, branch1_id, false, None)
                .unwrap();
            branch1_id
        };

        // rename first branch
        controller
            .update_virtual_branch(
                project,
                BranchUpdateRequest {
                    id: branch1_id,
                    name: Some("updated name".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();

        let branch2_id = {
            // create another branch with first branch's old name and push it
            let branch2_id = controller
                .create_virtual_branch(
                    project,
                    &BranchCreateRequest {
                        name: Some("name".to_string()),
                        ..Default::default()
                    },
                )
                .unwrap();
            fs::write(repository.path().join("file.txt"), "updated content").unwrap();
            controller
                .create_commit(project, branch2_id, "test", None, false)
                .unwrap();
            controller
                .push_virtual_branch(project, branch2_id, false, None)
                .unwrap();
            branch2_id
        };

        let (branches, _) = controller.list_virtual_branches(project).unwrap();
        assert_eq!(branches.len(), 2);
        // first branch is pushing to old ref remotely
        assert_eq!(branches[0].id, branch1_id);
        assert_eq!(branches[0].name, "updated name");
        assert_eq!(
            branches[0].upstream.as_ref().unwrap().name,
            "refs/remotes/origin/name".parse().unwrap()
        );
        // new branch is pushing to new ref remotely
        assert_eq!(branches[1].id, branch2_id);
        assert_eq!(branches[1].name, "name");
        assert_eq!(
            branches[1].upstream.as_ref().unwrap().name,
            "refs/remotes/origin/name-1".parse().unwrap()
        );

        let refnames = repository
            .references()
            .into_iter()
            .filter_map(|reference| reference.name().map(|name| name.to_string()))
            .collect::<Vec<_>>();
        assert!(refnames.contains(&branches[0].upstream.clone().unwrap().name.to_string()));
        assert!(refnames.contains(&branches[1].upstream.clone().unwrap().name.to_string()));
    }
}
