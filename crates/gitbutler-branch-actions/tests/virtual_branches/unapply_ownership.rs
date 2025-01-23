use std::fs;

use gitbutler_branch::BranchCreateRequest;
use gitbutler_stack::BranchOwnershipClaims;

use super::Test;

#[test]
fn should_unapply_with_commits() {
    let Test {
        repository, ctx, ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let stack_entry =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();

    fs::write(
        repository.path().join("file.txt"),
        "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n",
    )
    .unwrap();
    gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "test", None).unwrap();

    // change in the committed hunks leads to hunk locking
    fs::write(
        repository.path().join("file.txt"),
        "_\n2\n3\n4\n5\n6\n7\n8\n9\n_\n",
    )
    .unwrap();

    gitbutler_branch_actions::unapply_ownership(
        ctx,
        &"file.txt:1-5,7-11"
            .parse::<BranchOwnershipClaims>()
            .unwrap(),
    )
    .unwrap_or_else(|err| panic!("{err:?}"));

    let branch = gitbutler_branch_actions::list_virtual_branches(ctx)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == stack_entry.id)
        .unwrap();
    assert!(branch.files.is_empty());
}
