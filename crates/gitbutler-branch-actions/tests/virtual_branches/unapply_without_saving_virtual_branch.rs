use gitbutler_branch::BranchCreateRequest;

use super::*;

#[test]
fn should_unapply_diff() {
    let Test {
        repository, ctx, ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    // write some
    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let list_details = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_details.branches;
    let c =
        gitbutler_branch_actions::create_commit(ctx, branches.first().unwrap().id, "asdf", None);
    assert!(c.is_ok());

    gitbutler_branch_actions::unapply_without_saving_virtual_branch(ctx, branches[0].id).unwrap();

    let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_result.branches;
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
        repository, ctx, ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let id = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            name: Some("name".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    gitbutler_branch_actions::unapply_without_saving_virtual_branch(ctx, id).unwrap();

    let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_result.branches;

    assert_eq!(branches.len(), 0);

    let refnames = repository
        .references()
        .into_iter()
        .filter_map(|reference| reference.name().map(|name| name.to_string()))
        .collect::<Vec<_>>();
    assert!(!refnames.contains(&"refs/gitbutler/name".to_string()));
}
