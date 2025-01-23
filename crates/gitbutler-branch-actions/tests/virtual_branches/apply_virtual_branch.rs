use gitbutler_branch::BranchCreateRequest;
use gitbutler_reference::Refname;

use super::*;

#[test]
fn rebase_commit() {
    let Test {
        repository, ctx, ..
    } = &Test::default();

    // make sure we have an undiscovered commit in the remote branch
    {
        fs::write(repository.path().join("file.txt"), "one").unwrap();
        fs::write(repository.path().join("another_file.txt"), "").unwrap();
        let first_commit_oid = repository.commit_all("first");
        fs::write(repository.path().join("file.txt"), "two").unwrap();
        repository.commit_all("second");
        repository.push();
        repository.reset_hard(Some(first_commit_oid));
    }

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let mut stack_1_id = {
        // create a branch with some commited work
        let stack_entry_1 =
            gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
                .unwrap();
        fs::write(repository.path().join("another_file.txt"), "virtual").unwrap();

        gitbutler_branch_actions::create_commit(ctx, stack_entry_1.id, "virtual commit", None)
            .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, stack_entry_1.id);
        assert!(branches[0].active);
        assert_eq!(branches[0].files.len(), 0);
        assert_eq!(branches[0].series[0].clone().unwrap().patches.len(), 1);

        stack_entry_1.id
    };

    let unapplied_branch = {
        // unapply first vbranch
        let unapplied_branch =
            gitbutler_branch_actions::save_and_unapply_virutal_branch(ctx, stack_1_id).unwrap();

        assert_eq!(
            fs::read_to_string(repository.path().join("another_file.txt")).unwrap(),
            ""
        );
        assert_eq!(
            fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "one"
        );

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 0);

        Refname::from_str(&unapplied_branch).unwrap()
    };

    {
        // fetch remote
        gitbutler_branch_actions::integrate_upstream(ctx, &[], None).unwrap();

        // branch is stil unapplied
        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 0);

        assert_eq!(
            fs::read_to_string(repository.path().join("another_file.txt")).unwrap(),
            ""
        );
        assert_eq!(
            fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "two"
        );
    }

    {
        // apply first vbranch again
        stack_1_id = gitbutler_branch_actions::create_virtual_branch_from_branch(
            ctx,
            &unapplied_branch,
            None,
            None,
        )
        .unwrap();

        // it should be rebased
        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, stack_1_id);
        assert_eq!(branches[0].files.len(), 0);
        assert_eq!(branches[0].series[0].clone().unwrap().patches.len(), 1);
        assert!(branches[0].active);
        assert!(!branches[0].conflicted);

        assert_eq!(
            fs::read_to_string(repository.path().join("another_file.txt")).unwrap(),
            "virtual"
        );

        assert_eq!(
            fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "two"
        );
    }
}

#[test]
fn rebase_work() {
    let Test {
        repository, ctx, ..
    } = &Test::default();

    // make sure we have an undiscovered commit in the remote branch
    {
        let first_commit_oid = repository.commit_all("first");
        fs::write(repository.path().join("file.txt"), "").unwrap();
        repository.commit_all("second");
        repository.push();
        repository.reset_hard(Some(first_commit_oid));
    }

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let mut stack_1_id = {
        // make a branch with some work
        let stack_entry_1 =
            gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
                .unwrap();
        fs::write(repository.path().join("another_file.txt"), "").unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, stack_entry_1.id);
        assert!(branches[0].active);
        assert_eq!(branches[0].files.len(), 1);
        assert_eq!(branches[0].series[0].clone().unwrap().patches.len(), 0);

        stack_entry_1.id
    };

    let unapplied_branch = {
        // unapply first vbranch
        let unapplied_branch =
            gitbutler_branch_actions::save_and_unapply_virutal_branch(ctx, stack_1_id).unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 0);

        assert!(!repository.path().join("another_file.txt").exists());
        assert!(!repository.path().join("file.txt").exists());

        Refname::from_str(&unapplied_branch).unwrap()
    };

    {
        // fetch remote
        gitbutler_branch_actions::integrate_upstream(ctx, &[], None).unwrap();

        // first branch is stil unapplied
        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 0);

        assert!(!repository.path().join("another_file.txt").exists());
        assert!(repository.path().join("file.txt").exists());
    }

    {
        // apply first vbranch again
        stack_1_id = gitbutler_branch_actions::create_virtual_branch_from_branch(
            ctx,
            &unapplied_branch,
            None,
            None,
        )
        .unwrap();

        // workdir should be rebased, and work should be restored
        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, stack_1_id);
        assert_eq!(branches[0].files.len(), 1);
        assert_eq!(branches[0].series[0].clone().unwrap().patches.len(), 0);
        assert!(branches[0].active);
        assert!(!branches[0].conflicted);

        assert!(repository.path().join("another_file.txt").exists());
        assert!(repository.path().join("file.txt").exists());
    }
}
