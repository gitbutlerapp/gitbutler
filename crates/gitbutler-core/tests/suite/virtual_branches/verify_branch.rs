use super::*;

// Ensures that `verify_branch` returns an error when not on the integration branch.
#[tokio::test]
async fn should_fail_on_incorrect_branch() {
    let Test {
        repository,
        project_id,
        controller,
        ..
    } = &Test::default();

    let branch_name: git::LocalRefname = "refs/heads/somebranch".parse().unwrap();
    repository.checkout(&branch_name);
    let result = controller.list_virtual_branches(*project_id).await;

    let err = result.unwrap_err();
    assert_eq!(
        format!("{err:#}"),
        "<verification-failed>: project is on refs/heads/somebranch. Please checkout gitbutler/integration to continue"
    );
}
