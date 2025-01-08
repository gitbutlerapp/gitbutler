use std::fs;

use gitbutler_branch::BranchCreateRequest;

use super::Test;

#[test]
fn to_head() {
    let Test {
        repository,
        project,
        ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    let branch1_id =
        gitbutler_branch_actions::create_virtual_branch(project, &BranchCreateRequest::default())
            .unwrap();

    let oid = {
        fs::write(repository.path().join("file.txt"), "content").unwrap();

        // commit changes
        let oid =
            gitbutler_branch_actions::create_commit(project, branch1_id, "commit", None).unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert_eq!(branches[0].series[0].clone().unwrap().patches.len(), 1);
        assert_eq!(branches[0].series[0].clone().unwrap().patches[0].id, oid);
        assert_eq!(branches[0].files.len(), 0);
        assert_eq!(
            fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "content"
        );

        oid
    };

    {
        // reset changes to head
        gitbutler_branch_actions::reset_virtual_branch(project, branch1_id, oid).unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert_eq!(branches[0].series[0].clone().unwrap().patches.len(), 1);
        assert_eq!(branches[0].series[0].clone().unwrap().patches[0].id, oid);
        assert_eq!(branches[0].files.len(), 0);
        assert_eq!(
            fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "content"
        );
    }
}

#[test]
fn to_target() {
    let Test {
        repository,
        project,
        ..
    } = &Test::default();

    let base_branch = gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    let branch1_id =
        gitbutler_branch_actions::create_virtual_branch(project, &BranchCreateRequest::default())
            .unwrap();

    {
        fs::write(repository.path().join("file.txt"), "content").unwrap();

        // commit changes
        let oid =
            gitbutler_branch_actions::create_commit(project, branch1_id, "commit", None).unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert_eq!(branches[0].series[0].clone().unwrap().patches.len(), 1);
        assert_eq!(branches[0].series[0].clone().unwrap().patches[0].id, oid);
        assert_eq!(branches[0].files.len(), 0);
        assert_eq!(
            fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "content"
        );
    }

    {
        // reset changes to head
        gitbutler_branch_actions::reset_virtual_branch(project, branch1_id, base_branch.base_sha)
            .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert_eq!(branches[0].series[0].clone().unwrap().patches.len(), 0);
        assert_eq!(branches[0].files.len(), 1);
        assert_eq!(
            fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "content"
        );
    }
}

#[test]
fn to_commit() {
    let Test {
        repository,
        project,
        ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    let branch1_id =
        gitbutler_branch_actions::create_virtual_branch(project, &BranchCreateRequest::default())
            .unwrap();

    let first_commit_oid = {
        // commit some changes

        fs::write(repository.path().join("file.txt"), "content").unwrap();

        let oid =
            gitbutler_branch_actions::create_commit(project, branch1_id, "commit", None).unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert_eq!(branches[0].series[0].clone().unwrap().patches.len(), 1);
        assert_eq!(branches[0].series[0].clone().unwrap().patches[0].id, oid);
        assert_eq!(branches[0].files.len(), 0);
        assert_eq!(
            fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "content"
        );

        oid
    };

    {
        // commit some more
        fs::write(repository.path().join("file.txt"), "more content").unwrap();

        let second_commit_oid =
            gitbutler_branch_actions::create_commit(project, branch1_id, "commit", None).unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert_eq!(branches[0].series[0].clone().unwrap().patches.len(), 2);
        assert_eq!(
            branches[0].series[0].clone().unwrap().patches[0].id,
            second_commit_oid
        );
        assert_eq!(
            branches[0].series[0].clone().unwrap().patches[1].id,
            first_commit_oid
        );
        assert_eq!(branches[0].files.len(), 0);
        assert_eq!(
            fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "more content"
        );
    }

    {
        // reset changes to the first commit
        gitbutler_branch_actions::reset_virtual_branch(project, branch1_id, first_commit_oid)
            .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert_eq!(branches[0].series[0].clone().unwrap().patches.len(), 1);
        assert_eq!(
            branches[0].series[0].clone().unwrap().patches[0].id,
            first_commit_oid
        );
        assert_eq!(branches[0].files.len(), 1);
        assert_eq!(
            fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "more content"
        );
    }
}

#[test]
fn to_non_existing() {
    let Test {
        repository,
        project,
        ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    let branch1_id =
        gitbutler_branch_actions::create_virtual_branch(project, &BranchCreateRequest::default())
            .unwrap();

    {
        fs::write(repository.path().join("file.txt"), "content").unwrap();

        // commit changes
        let oid =
            gitbutler_branch_actions::create_commit(project, branch1_id, "commit", None).unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert_eq!(branches[0].series[0].clone().unwrap().patches.len(), 1);
        assert_eq!(branches[0].series[0].clone().unwrap().patches[0].id, oid);
        assert_eq!(branches[0].files.len(), 0);
        assert_eq!(
            fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "content"
        );

        oid
    };

    assert_eq!(
        gitbutler_branch_actions::reset_virtual_branch(
            project,
            branch1_id,
            "fe14df8c66b73c6276f7bb26102ad91da680afcb".parse().unwrap()
        )
        .unwrap_err()
        .to_string(),
        "commit fe14df8c66b73c6276f7bb26102ad91da680afcb not in the branch"
    );
}
