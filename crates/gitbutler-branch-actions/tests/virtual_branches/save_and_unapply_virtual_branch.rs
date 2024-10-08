use super::*;

#[test]
fn unapply_with_data() {
    let Test {
        project,
        repository,
        ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 1);

    gitbutler_branch_actions::save_and_unapply_virutal_branch(project, branches[0].id).unwrap();

    assert!(!repository.path().join("file.txt").exists());

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 0);
}

#[test]
fn delete_if_empty() {
    let Test { project, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    gitbutler_branch_actions::create_virtual_branch(project, &BranchCreateRequest::default())
        .unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 1);

    gitbutler_branch_actions::save_and_unapply_virutal_branch(project, branches[0].id).unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 0);
}
