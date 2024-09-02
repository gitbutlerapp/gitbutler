use gitbutler_reference::LocalRefname;

use super::*;

// Ensures that `verify_branch` returns an error when not on the workspace branch.
#[test]
fn should_fail_on_incorrect_branch() {
    let Test {
        repository,
        project,
        controller,
        ..
    } = &Test::default();

    let branch_name: LocalRefname = "refs/heads/somebranch".parse().unwrap();
    repository.checkout(&branch_name);
    let result = controller.list_virtual_branches(project);

    let err = result.unwrap_err();
    assert_eq!(
        format!("{err:#}"),
        "<verification-failed>: project is on refs/heads/somebranch. Please checkout gitbutler/workspace to continue"
    );
}
