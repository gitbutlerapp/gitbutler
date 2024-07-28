use gitbutler_branch::BranchCreateRequest;

use super::*;

#[test]
fn should_unapply_diff() {
    let Test {
        project,
        controller,
        repository,
        ..
    } = &Test::default();

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    // write some
    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _) = controller.list_virtual_branches(project).unwrap();

    controller
        .delete_virtual_branch(project, branches[0].id)
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 0);
    assert!(!repository.path().join("file.txt").exists());

    let refnames = repository
        .references()
        .into_iter()
        .filter_map(|reference| reference.name().map(|name| name.to_string()))
        .collect::<Vec<_>>();
    assert!(!refnames.contains(&"refs/gitbutler/name".to_string()));
}

#[test]
fn should_remove_reference() {
    let Test {
        project,
        controller,
        repository,
        ..
    } = &Test::default();

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let id = controller
        .create_virtual_branch(
            project,
            &BranchCreateRequest {
                name: Some("name".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

    controller.delete_virtual_branch(project, id).unwrap();

    let (branches, _) = controller.list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 0);

    let refnames = repository
        .references()
        .into_iter()
        .filter_map(|reference| reference.name().map(|name| name.to_string()))
        .collect::<Vec<_>>();
    assert!(!refnames.contains(&"refs/gitbutler/name".to_string()));
}
