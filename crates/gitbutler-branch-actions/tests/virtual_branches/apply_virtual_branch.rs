use gitbutler_branch::BranchCreateRequest;
use gitbutler_reference::Refname;
use gitbutler_testsupport::stack_details;

use super::*;

#[test]
fn rebase_commit() {
    let Test { repo, ctx, .. } = &Test::default();

    // make sure we have an undiscovered commit in the remote branch
    {
        fs::write(repo.path().join("file.txt"), "one").unwrap();
        fs::write(repo.path().join("another_file.txt"), "").unwrap();
        let first_commit_oid = repo.commit_all("first");
        fs::write(repo.path().join("file.txt"), "two").unwrap();
        repo.commit_all("second");
        repo.push();
        repo.reset_hard(Some(first_commit_oid));
    }

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let mut stack_1_id = {
        // create a branch with some commited work
        let stack_entry_1 = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest::default(),
            ctx.project().exclusive_worktree_access().write_permission(),
        )
        .unwrap();
        fs::write(repo.path().join("another_file.txt"), "virtual").unwrap();

        gitbutler_branch_actions::create_commit(ctx, stack_entry_1.id, "virtual commit", None)
            .unwrap();

        let stacks = stack_details(ctx);
        assert_eq!(stacks.len(), 1);
        assert_eq!(stacks[0].0, stack_entry_1.id);
        assert_eq!(stacks[0].1.branch_details[0].commits.len(), 1);

        stack_entry_1.id
    };

    let unapplied_branch = {
        // unapply first vbranch
        let unapplied_branch =
            gitbutler_branch_actions::unapply_stack(ctx, stack_1_id, Vec::new()).unwrap();

        assert_eq!(
            fs::read_to_string(repo.path().join("another_file.txt")).unwrap(),
            ""
        );
        assert_eq!(
            fs::read_to_string(repo.path().join("file.txt")).unwrap(),
            "one"
        );

        let stacks = stack_details(ctx);
        assert_eq!(stacks.len(), 0);

        Refname::from_str(&unapplied_branch).unwrap()
    };

    {
        // fetch remote
        gitbutler_branch_actions::integrate_upstream(ctx, &[], None, &Default::default()).unwrap();

        // branch is stil unapplied
        let stacks = stack_details(ctx);
        assert_eq!(stacks.len(), 0);

        assert_eq!(
            fs::read_to_string(repo.path().join("another_file.txt")).unwrap(),
            ""
        );
        assert_eq!(
            fs::read_to_string(repo.path().join("file.txt")).unwrap(),
            "two"
        );
    }

    {
        // apply first vbranch again
        let outcome = gitbutler_branch_actions::create_virtual_branch_from_branch(
            ctx,
            &unapplied_branch,
            None,
            None,
        )
        .unwrap();

        stack_1_id = outcome.0;

        // it should be rebased
        let stacks = stack_details(ctx);
        assert_eq!(stacks.len(), 1);
        assert_eq!(stacks[0].0, stack_1_id);
        assert_eq!(stacks[0].1.branch_details[0].commits.len(), 1);
        assert!(!stacks[0].1.branch_details[0].is_conflicted);

        assert_eq!(
            fs::read_to_string(repo.path().join("another_file.txt")).unwrap(),
            "virtual"
        );

        assert_eq!(
            fs::read_to_string(repo.path().join("file.txt")).unwrap(),
            "two"
        );
    }
}
