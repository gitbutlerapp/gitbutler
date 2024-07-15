use std::fs;

use gitbutler_branch::{BranchCreateRequest, BranchOwnershipClaims};

use super::Test;

#[tokio::test]
async fn should_unapply_with_commits() {
    let Test {
        project,
        controller,
        repository,
        ..
    } = &Test::default();

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let branch_id = controller
        .create_virtual_branch(project, &BranchCreateRequest::default())
        .await
        .unwrap();

    fs::write(
        repository.path().join("file.txt"),
        "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n",
    )
    .unwrap();
    controller
        .create_commit(project, branch_id, "test", None, false)
        .await
        .unwrap();

    // change in the committed hunks leads to hunk locking
    fs::write(
        repository.path().join("file.txt"),
        "_\n2\n3\n4\n5\n6\n7\n8\n9\n_\n",
    )
    .unwrap();

    controller
        .unapply_ownership(
            project,
            &"file.txt:1-5,7-11"
                .parse::<BranchOwnershipClaims>()
                .unwrap(),
        )
        .await
        .unwrap_or_else(|err| panic!("{err:?}"));

    let branch = controller
        .list_virtual_branches(project)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == branch_id)
        .unwrap();
    assert!(branch.files.is_empty());
}
