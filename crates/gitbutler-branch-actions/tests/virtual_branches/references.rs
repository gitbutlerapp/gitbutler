use super::*;

mod create_virtual_branch {
    use gitbutler_branch::BranchCreateRequest;

    use super::*;

    #[test]
    fn simple() {
        let Test {
            project,

            repository,
            ..
        } = &Test::default();

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let branch_id = gitbutler_branch_actions::create_virtual_branch(
            project,
            &BranchCreateRequest::default(),
        )
        .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch_id);
        assert_eq!(branches[0].name, "Lane");

        let refnames = repository
            .references()
            .into_iter()
            .filter_map(|reference| reference.name().map(|name| name.to_string()))
            .collect::<Vec<_>>();
        assert!(refnames.contains(&"refs/gitbutler/Lane".to_string()));
    }

    #[test]
    fn duplicate_name() {
        let Test {
            project,

            repository,
            ..
        } = &Test::default();

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let branch1_id = gitbutler_branch_actions::create_virtual_branch(
            project,
            &BranchCreateRequest {
                name: Some("name".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        let branch2_id = gitbutler_branch_actions::create_virtual_branch(
            project,
            &BranchCreateRequest {
                name: Some("name".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
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
    use gitbutler_branch::{BranchCreateRequest, BranchUpdateRequest};

    use super::*;

    #[test]
    fn simple() {
        let Test {
            project,

            repository,
            ..
        } = &Test::default();

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let branch_id = gitbutler_branch_actions::create_virtual_branch(
            project,
            &BranchCreateRequest {
                name: Some("name".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        gitbutler_branch_actions::update_virtual_branch(
            project,
            BranchUpdateRequest {
                id: branch_id,
                name: Some("new name".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
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

            repository,
            ..
        } = &Test::default();

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let branch1_id = gitbutler_branch_actions::create_virtual_branch(
            project,
            &BranchCreateRequest {
                name: Some("name".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        let branch2_id = gitbutler_branch_actions::create_virtual_branch(
            project,
            &BranchCreateRequest {
                ..Default::default()
            },
        )
        .unwrap();

        gitbutler_branch_actions::update_virtual_branch(
            project,
            BranchUpdateRequest {
                id: branch2_id,
                name: Some("name".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
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
    use gitbutler_branch::{BranchCreateRequest, BranchUpdateRequest};

    use super::*;

    #[test]
    fn simple() {
        let Test {
            project,

            repository,
            ..
        } = &Test::default();

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let branch1_id = gitbutler_branch_actions::create_virtual_branch(
            project,
            &BranchCreateRequest {
                name: Some("name".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        fs::write(repository.path().join("file.txt"), "content").unwrap();

        gitbutler_branch_actions::create_commit(project, branch1_id, "test", None, false).unwrap();
        #[allow(deprecated)]
        gitbutler_branch_actions::push_virtual_branch(project, branch1_id, false, None).unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
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

            repository,
            ..
        } = &Test::default();

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let branch1_id = {
            // create and push branch with some work
            let branch1_id = gitbutler_branch_actions::create_virtual_branch(
                project,
                &BranchCreateRequest {
                    name: Some("name".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();
            fs::write(repository.path().join("file.txt"), "content").unwrap();
            gitbutler_branch_actions::create_commit(project, branch1_id, "test", None, false)
                .unwrap();
            #[allow(deprecated)]
            gitbutler_branch_actions::push_virtual_branch(project, branch1_id, false, None)
                .unwrap();
            branch1_id
        };

        // rename first branch
        gitbutler_branch_actions::update_virtual_branch(
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
            let branch2_id = gitbutler_branch_actions::create_virtual_branch(
                project,
                &BranchCreateRequest {
                    name: Some("name".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();
            fs::write(repository.path().join("file.txt"), "updated content").unwrap();
            gitbutler_branch_actions::create_commit(project, branch2_id, "test", None, false)
                .unwrap();
            #[allow(deprecated)]
            gitbutler_branch_actions::push_virtual_branch(project, branch2_id, false, None)
                .unwrap();
            branch2_id
        };

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
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
