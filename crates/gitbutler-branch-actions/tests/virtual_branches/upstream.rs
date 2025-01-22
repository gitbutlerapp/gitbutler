use gitbutler_branch::BranchCreateRequest;

use super::*;

#[test]
fn detect_upstream_commits() {
    let Test {
        repository, ctx, ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let branch1_id =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();

    let oid1 = {
        // create first commit
        fs::write(repository.path().join("file.txt"), "content").unwrap();
        gitbutler_branch_actions::create_commit(ctx, branch1_id, "commit", None).unwrap()
    };

    let oid2 = {
        // create second commit
        fs::write(repository.path().join("file.txt"), "content2").unwrap();
        gitbutler_branch_actions::create_commit(ctx, branch1_id, "commit", None).unwrap()
    };

    // push
    gitbutler_branch_actions::stack::push_stack(ctx, branch1_id, false).unwrap();

    let oid3 = {
        // create third commit
        fs::write(repository.path().join("file.txt"), "content3").unwrap();
        gitbutler_branch_actions::create_commit(ctx, branch1_id, "commit", None).unwrap()
    };

    {
        // should correctly detect pushed commits
        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert_eq!(branches[0].series[0].clone().unwrap().patches.len(), 3);
        assert_eq!(branches[0].series[0].clone().unwrap().patches[0].id, oid3);
        assert!(!branches[0].series[0].clone().unwrap().patches[0].is_local_and_remote);
        assert_eq!(branches[0].series[0].clone().unwrap().patches[1].id, oid2);
        assert!(branches[0].series[0].clone().unwrap().patches[1].is_local_and_remote);
        assert_eq!(branches[0].series[0].clone().unwrap().patches[2].id, oid1);
        assert!(branches[0].series[0].clone().unwrap().patches[2].is_local_and_remote);
    }
}

#[test]
fn detect_integrated_commits() {
    let Test {
        repository, ctx, ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let branch1_id =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();

    let oid1 = {
        // create first commit
        fs::write(repository.path().join("file.txt"), "content").unwrap();
        gitbutler_branch_actions::create_commit(ctx, branch1_id, "commit", None).unwrap()
    };

    let oid2 = {
        // create second commit
        fs::write(repository.path().join("file.txt"), "content2").unwrap();
        gitbutler_branch_actions::create_commit(ctx, branch1_id, "commit", None).unwrap()
    };

    // push
    #[allow(deprecated)]
    gitbutler_branch_actions::push_virtual_branch(ctx, branch1_id, false, None).unwrap();

    {
        // merge branch upstream
        let branch = gitbutler_branch_actions::list_virtual_branches(ctx)
            .unwrap()
            .branches
            .into_iter()
            .find(|b| b.id == branch1_id)
            .unwrap();
        repository
            .merge(&branch.upstream.as_ref().unwrap().name)
            .unwrap();
        repository.fetch();
    }

    let oid3 = {
        // create third commit
        fs::write(repository.path().join("file.txt"), "content3").unwrap();
        gitbutler_branch_actions::create_commit(ctx, branch1_id, "commit", None).unwrap()
    };

    {
        // should correctly detect pushed commits
        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;

        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert_eq!(branches[0].series[0].clone().unwrap().patches.len(), 3);
        assert_eq!(branches[0].series[0].clone().unwrap().patches[0].id, oid3);
        assert!(!branches[0].series[0].clone().unwrap().patches[0].is_integrated);
        assert_eq!(branches[0].series[0].clone().unwrap().patches[1].id, oid2);
        assert!(branches[0].series[0].clone().unwrap().patches[1].is_integrated);
        assert_eq!(branches[0].series[0].clone().unwrap().patches[2].id, oid1);
        assert!(branches[0].series[0].clone().unwrap().patches[2].is_integrated);
    }
}
