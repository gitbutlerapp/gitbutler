use super::*;

#[test]
fn should_unapply_diff() {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    // write some
    std::fs::write(repo.path().join("file.txt"), "content").unwrap();

    let list_details = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_details.branches;
    let c =
        gitbutler_branch_actions::create_commit(ctx, branches.first().unwrap().id, "asdf", None);
    assert!(c.is_ok());

    gitbutler_branch_actions::unapply_stack(ctx, branches[0].id).unwrap();

    let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_result.branches;
    assert_eq!(branches.len(), 0);
    assert!(!repo.path().join("file.txt").exists());

    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true);
    let statuses = repo.local_repo.statuses(Some(&mut opts)).unwrap();
    assert!(statuses.is_empty());

    let refnames = repo
        .references()
        .into_iter()
        .filter_map(|reference| reference.name().map(|name| name.to_string()))
        .collect::<Vec<_>>();
    assert!(!refnames.contains(&"refs/gitbutler/name".to_string()));
}
