use gitbutler_stack::BranchCreateRequest;

use super::*;

#[test]
fn should_unapply_diff() {
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

    // write some
    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    let c = gitbutler_branch_actions::create_commit(
        project,
        branches.first().unwrap().id,
        "asdf",
        None,
        false,
    );
    assert!(c.is_ok());

    gitbutler_branch_actions::unapply_without_saving_virtual_branch(project, branches[0].id)
        .unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 0);
    assert!(!repository.path().join("file.txt").exists());

    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true);
    let statuses = repository
        .local_repository
        .statuses(Some(&mut opts))
        .unwrap();
    assert!(statuses.is_empty());

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
        repository,
        ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    let id = gitbutler_branch_actions::create_virtual_branch(
        project,
        &BranchCreateRequest {
            name: Some("name".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    gitbutler_branch_actions::unapply_without_saving_virtual_branch(project, id).unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 0);

    let refnames = repository
        .references()
        .into_iter()
        .filter_map(|reference| reference.name().map(|name| name.to_string()))
        .collect::<Vec<_>>();
    assert!(!refnames.contains(&"refs/gitbutler/name".to_string()));
}
