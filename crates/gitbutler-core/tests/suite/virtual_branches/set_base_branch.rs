use super::*;

#[tokio::test]
async fn success() {
    let Test {
        project_id,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();
}

mod error {
    use super::*;

    #[tokio::test]
    async fn missing() {
        let Test {
            project_id,
            controller,
            ..
        } = &Test::default();

        assert_eq!(
            controller
                .set_base_branch(
                    *project_id,
                    &git::RemoteRefname::from_str("refs/remotes/origin/missing").unwrap(),
                )
                .await
                .unwrap_err()
                .to_string(),
            "remote branch 'refs/remotes/origin/missing' not found"
        );
    }
}

mod go_back_to_integration {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn should_preserve_applied_vbranches() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = &Test::default();

        std::fs::write(repository.path().join("file.txt"), "one").unwrap();
        let oid_one = repository.commit_all("one");
        std::fs::write(repository.path().join("file.txt"), "two").unwrap();
        repository.commit_all("two");
        repository.push();

        controller
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let vbranch_id = controller
            .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        std::fs::write(repository.path().join("another file.txt"), "content").unwrap();
        controller
            .create_commit(*project_id, vbranch_id, "one", None, false)
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
        assert_eq!(branches.len(), 1);

        repository.checkout_commit(oid_one);

        controller
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, vbranch_id);
        assert!(branches[0].active);
    }

    #[tokio::test]
    async fn from_target_branch_index_conflicts() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = &Test::default();

        std::fs::write(repository.path().join("file.txt"), "one").unwrap();
        let oid_one = repository.commit_all("one");
        std::fs::write(repository.path().join("file.txt"), "two").unwrap();
        repository.commit_all("two");
        repository.push();

        controller
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
        assert!(branches.is_empty());

        repository.checkout_commit(oid_one);
        std::fs::write(repository.path().join("file.txt"), "tree").unwrap();

        assert!(matches!(
            controller
                .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap_err()
                .downcast_ref(),
            Some(Marker::ProjectConflict)
        ));
    }

    #[tokio::test]
    async fn from_target_branch_with_uncommited() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = &Test::default();

        std::fs::write(repository.path().join("file.txt"), "one").unwrap();
        let oid_one = repository.commit_all("one");
        std::fs::write(repository.path().join("file.txt"), "two").unwrap();
        repository.commit_all("two");
        repository.push();

        controller
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
        assert!(branches.is_empty());

        repository.checkout_commit(oid_one);
        std::fs::write(repository.path().join("another file.txt"), "tree").unwrap();

        assert!(matches!(
            controller
                .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap_err()
                .downcast_ref(),
            Some(Marker::ProjectConflict)
        ));
    }

    #[tokio::test]
    async fn from_target_branch_with_commit() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = &Test::default();

        std::fs::write(repository.path().join("file.txt"), "one").unwrap();
        let oid_one = repository.commit_all("one");
        std::fs::write(repository.path().join("file.txt"), "two").unwrap();
        repository.commit_all("two");
        repository.push();

        let base = controller
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
        assert!(branches.is_empty());

        repository.checkout_commit(oid_one);
        std::fs::write(repository.path().join("another file.txt"), "tree").unwrap();
        repository.commit_all("three");

        let base_two = controller
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
        assert_eq!(branches.len(), 0);
        assert_eq!(base_two, base);
    }

    #[tokio::test]
    async fn from_target_branch_without_any_changes() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = &Test::default();

        std::fs::write(repository.path().join("file.txt"), "one").unwrap();
        let oid_one = repository.commit_all("one");
        std::fs::write(repository.path().join("file.txt"), "two").unwrap();
        repository.commit_all("two");
        repository.push();

        let base = controller
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
        assert!(branches.is_empty());

        repository.checkout_commit(oid_one);

        let base_two = controller
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
        assert_eq!(branches.len(), 0);
        assert_eq!(base_two, base);
    }
}
