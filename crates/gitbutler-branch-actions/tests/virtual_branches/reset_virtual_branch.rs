use std::fs;

use gitbutler_branch::BranchCreateRequest;

use super::Test;

#[test]
fn to_head() {
    let Test {
        repository, ctx, ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let stack_entry =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();

    let oid = {
        fs::write(repository.path().join("file.txt"), "content").unwrap();

        // commit changes
        let oid =
            gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit", None).unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, stack_entry.id);
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
        gitbutler_branch_actions::reset_virtual_branch(ctx, stack_entry.id, oid).unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, stack_entry.id);
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
        repository, ctx, ..
    } = &Test::default();

    let base_branch = gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    let stack_entry_1 =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();

    {
        fs::write(repository.path().join("file.txt"), "content").unwrap();

        // commit changes
        let oid =
            gitbutler_branch_actions::create_commit(ctx, stack_entry_1.id, "commit", None).unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, stack_entry_1.id);
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
        gitbutler_branch_actions::reset_virtual_branch(ctx, stack_entry_1.id, base_branch.base_sha)
            .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, stack_entry_1.id);
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
        repository, ctx, ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let stack_entry_1 =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();

    let first_commit_oid = {
        // commit some changes

        fs::write(repository.path().join("file.txt"), "content").unwrap();

        let oid =
            gitbutler_branch_actions::create_commit(ctx, stack_entry_1.id, "commit", None).unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, stack_entry_1.id);
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
            gitbutler_branch_actions::create_commit(ctx, stack_entry_1.id, "commit", None).unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, stack_entry_1.id);
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
        gitbutler_branch_actions::reset_virtual_branch(ctx, stack_entry_1.id, first_commit_oid)
            .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, stack_entry_1.id);
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
        repository, ctx, ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let stack_entry_1 =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();

    {
        fs::write(repository.path().join("file.txt"), "content").unwrap();

        // commit changes
        let oid =
            gitbutler_branch_actions::create_commit(ctx, stack_entry_1.id, "commit", None).unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, stack_entry_1.id);
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
            ctx,
            stack_entry_1.id,
            "fe14df8c66b73c6276f7bb26102ad91da680afcb".parse().unwrap()
        )
        .unwrap_err()
        .to_string(),
        "commit fe14df8c66b73c6276f7bb26102ad91da680afcb not in the branch"
    );
}
