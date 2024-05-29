use super::*;

mod create_virtual_branch {
    use super::*;

    #[tokio::test]
    async fn simple() {
        let Test {
            project_id,
            controller,
            repository,
            ..
        } = &Test::default();

        controller
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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

    #[tokio::test]
    async fn duplicate_name() {
        let Test {
            project_id,
            controller,
            repository,
            ..
        } = &Test::default();

        controller
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch1_id = controller
            .create_virtual_branch(
                *project_id,
                &gitbutler_core::virtual_branches::branch::BranchCreateRequest {
                    name: Some("name".to_string()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let branch2_id = controller
            .create_virtual_branch(
                *project_id,
                &gitbutler_core::virtual_branches::branch::BranchCreateRequest {
                    name: Some("name".to_string()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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

    #[tokio::test]
    async fn simple() {
        let Test {
            project_id,
            controller,
            repository,
            ..
        } = &Test::default();

        controller
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(
                *project_id,
                &branch::BranchCreateRequest {
                    name: Some("name".to_string()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        controller
            .update_virtual_branch(
                *project_id,
                branch::BranchUpdateRequest {
                    id: branch_id,
                    name: Some("new name".to_string()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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

    #[tokio::test]
    async fn duplicate_name() {
        let Test {
            project_id,
            controller,
            repository,
            ..
        } = &Test::default();

        controller
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch1_id = controller
            .create_virtual_branch(
                *project_id,
                &branch::BranchCreateRequest {
                    name: Some("name".to_string()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let branch2_id = controller
            .create_virtual_branch(
                *project_id,
                &branch::BranchCreateRequest {
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        controller
            .update_virtual_branch(
                *project_id,
                branch::BranchUpdateRequest {
                    id: branch2_id,
                    name: Some("name".to_string()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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

    #[tokio::test]
    async fn simple() {
        let Test {
            project_id,
            controller,
            repository,
            ..
        } = &Test::default();

        controller
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch1_id = controller
            .create_virtual_branch(
                *project_id,
                &branch::BranchCreateRequest {
                    name: Some("name".to_string()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        fs::write(repository.path().join("file.txt"), "content").unwrap();

        controller
            .create_commit(*project_id, branch1_id, "test", None, false)
            .await
            .unwrap();
        controller
            .push_virtual_branch(*project_id, branch1_id, false, None)
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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

    #[tokio::test]
    async fn duplicate_names() {
        let Test {
            project_id,
            controller,
            repository,
            ..
        } = &Test::default();

        controller
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch1_id = {
            // create and push branch with some work
            let branch1_id = controller
                .create_virtual_branch(
                    *project_id,
                    &branch::BranchCreateRequest {
                        name: Some("name".to_string()),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            fs::write(repository.path().join("file.txt"), "content").unwrap();
            controller
                .create_commit(*project_id, branch1_id, "test", None, false)
                .await
                .unwrap();
            controller
                .push_virtual_branch(*project_id, branch1_id, false, None)
                .await
                .unwrap();
            branch1_id
        };

        // rename first branch
        controller
            .update_virtual_branch(
                *project_id,
                branch::BranchUpdateRequest {
                    id: branch1_id,
                    name: Some("updated name".to_string()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let branch2_id = {
            // create another branch with first branch's old name and push it
            let branch2_id = controller
                .create_virtual_branch(
                    *project_id,
                    &branch::BranchCreateRequest {
                        name: Some("name".to_string()),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            fs::write(repository.path().join("file.txt"), "updated content").unwrap();
            controller
                .create_commit(*project_id, branch2_id, "test", None, false)
                .await
                .unwrap();
            controller
                .push_virtual_branch(*project_id, branch2_id, false, None)
                .await
                .unwrap();
            branch2_id
        };

        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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
