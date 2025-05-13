use super::*;

mod create_virtual_branch {
    use gitbutler_branch::BranchCreateRequest;

    use super::*;

    #[test]
    fn simple() {
        let Test { repo, ctx, .. } = &Test::default();

        gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            false,
        )
        .unwrap();

        let stack_entry =
            gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
                .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, stack_entry.id);
        assert_eq!(branches[0].name, "Lane");

        let refnames = repo
            .references()
            .into_iter()
            .filter_map(|reference| reference.name().map(|name| name.to_string()))
            .collect::<Vec<_>>();
        assert!(refnames.contains(&"refs/gitbutler/Lane".to_string()));
    }

    #[test]
    fn duplicate_name() {
        let Test { repo, ctx, .. } = &Test::default();

        gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            false,
        )
        .unwrap();

        let stack_entry_1 = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest {
                name: Some("name".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        let stack_entry_2 = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest {
                name: Some("name".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 2);
        assert_eq!(branches[0].id, stack_entry_1.id);
        assert_eq!(branches[0].name, "name");
        assert_eq!(branches[1].id, stack_entry_2.id);
        assert_eq!(branches[1].name, "name-1");

        let refnames = repo
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
        let Test { repo, ctx, .. } = &Test::default();

        gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            false,
        )
        .unwrap();

        let stack_entry = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest {
                name: Some("name".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        gitbutler_branch_actions::update_virtual_branch(
            ctx,
            BranchUpdateRequest {
                id: stack_entry.id,
                name: Some("new name".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, stack_entry.id);
        assert_eq!(branches[0].name, "new name");

        let refnames = repo
            .references()
            .into_iter()
            .filter_map(|reference| reference.name().map(|name| name.to_string()))
            .collect::<Vec<_>>();
        assert!(!refnames.contains(&"refs/gitbutler/name".to_string()));
        assert!(refnames.contains(&"refs/gitbutler/new-name".to_string()));
    }

    #[test]
    fn duplicate_name() {
        let Test { repo, ctx, .. } = &Test::default();

        gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            false,
        )
        .unwrap();

        let stack_entry_1 = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest {
                name: Some("name".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        let stack_entry_2 = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest {
                ..Default::default()
            },
        )
        .unwrap();

        gitbutler_branch_actions::update_virtual_branch(
            ctx,
            BranchUpdateRequest {
                id: stack_entry_2.id,
                name: Some("name".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 2);
        assert_eq!(branches[0].id, stack_entry_1.id);
        assert_eq!(branches[0].name, "name");
        assert_eq!(branches[1].id, stack_entry_2.id);
        assert_eq!(branches[1].name, "name-1");

        let refnames = repo
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
        let Test { repo, ctx, .. } = &Test::default();

        gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            false,
        )
        .unwrap();

        let stack_entry_1 = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest {
                name: Some("name".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        fs::write(repo.path().join("file.txt"), "content").unwrap();

        gitbutler_branch_actions::create_commit(ctx, stack_entry_1.id, "test", None).unwrap();
        #[allow(deprecated)]
        gitbutler_branch_actions::push_virtual_branch(ctx, stack_entry_1.id, false, None).unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, stack_entry_1.id);
        assert_eq!(branches[0].name, "name");
        assert_eq!(
            branches[0].upstream.as_ref().unwrap().name.to_string(),
            "refs/remotes/origin/name"
        );

        let refnames = repo
            .references()
            .into_iter()
            .filter_map(|reference| reference.name().map(|name| name.to_string()))
            .collect::<Vec<_>>();
        assert!(refnames.contains(&branches[0].upstream.clone().unwrap().name.to_string()));
    }

    #[test]
    fn duplicate_names() {
        let Test { repo, ctx, .. } = &Test::default();

        gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            false,
        )
        .unwrap();

        let stack_entry = {
            // create and push branch with some work
            let stack_entry_1 = gitbutler_branch_actions::create_virtual_branch(
                ctx,
                &BranchCreateRequest {
                    name: Some("name".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();
            fs::write(repo.path().join("file.txt"), "content").unwrap();
            gitbutler_branch_actions::create_commit(ctx, stack_entry_1.id, "test", None).unwrap();
            #[allow(deprecated)]
            gitbutler_branch_actions::push_virtual_branch(ctx, stack_entry_1.id, false, None)
                .unwrap();
            stack_entry_1
        };

        // rename first branch
        gitbutler_branch_actions::update_virtual_branch(
            ctx,
            BranchUpdateRequest {
                id: stack_entry.id,
                name: Some("updated name".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        let stack_entry_2 = {
            // create another branch with first branch's old name and push it
            let stack_entry_2 = gitbutler_branch_actions::create_virtual_branch(
                ctx,
                &BranchCreateRequest {
                    name: Some("name".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();
            fs::write(repo.path().join("file.txt"), "updated content").unwrap();
            gitbutler_branch_actions::create_commit(ctx, stack_entry_2.id, "test", None).unwrap();
            #[allow(deprecated)]
            gitbutler_branch_actions::push_virtual_branch(ctx, stack_entry_2.id, false, None)
                .unwrap();
            stack_entry_2
        };

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 2);
        // first branch is pushing to old ref remotely
        assert_eq!(branches[0].id, stack_entry.id);
        assert_eq!(branches[0].name, "updated name");
        assert_eq!(
            branches[0].upstream.as_ref().unwrap().name,
            "refs/remotes/origin/name".parse().unwrap()
        );
        // new branch is pushing to new ref remotely
        assert_eq!(branches[1].id, stack_entry_2.id);
        assert_eq!(branches[1].name, "name");
        assert_eq!(
            branches[1].upstream.as_ref().unwrap().name,
            "refs/remotes/origin/name-1".parse().unwrap()
        );

        let refnames = repo
            .references()
            .into_iter()
            .filter_map(|reference| reference.name().map(|name| name.to_string()))
            .collect::<Vec<_>>();
        assert!(refnames.contains(&branches[0].upstream.clone().unwrap().name.to_string()));
        assert!(refnames.contains(&branches[1].upstream.clone().unwrap().name.to_string()));
    }
}
