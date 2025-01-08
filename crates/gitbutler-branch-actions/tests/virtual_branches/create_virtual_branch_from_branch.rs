use gitbutler_branch::BranchCreateRequest;
use gitbutler_reference::LocalRefname;

use super::*;

#[test]
fn integration() {
    let Test {
        repository,
        project,
        ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    let branch_name = {
        // make a remote branch

        let branch_id = gitbutler_branch_actions::create_virtual_branch(
            project,
            &BranchCreateRequest::default(),
        )
        .unwrap();

        std::fs::write(repository.path().join("file.txt"), "first\n").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "first", None).unwrap();
        gitbutler_branch_actions::push_virtual_branch(project, branch_id, false, None).unwrap();

        let branch = gitbutler_branch_actions::list_virtual_branches(project)
            .unwrap()
            .branches
            .into_iter()
            .find(|branch| branch.id == branch_id)
            .unwrap();

        let name = branch.upstream.unwrap().name;

        gitbutler_branch_actions::unapply_without_saving_virtual_branch(project, branch_id)
            .unwrap();

        name
    };

    // checkout a existing remote branch
    let branch_id = gitbutler_branch_actions::create_virtual_branch_from_branch(
        project,
        &branch_name,
        None,
        Some(123),
    )
    .unwrap();

    {
        // add a commit
        std::fs::write(repository.path().join("file.txt"), "first\nsecond").unwrap();

        gitbutler_branch_actions::create_commit(project, branch_id, "second", None).unwrap();
    }

    {
        // meanwhile, there is a new commit on master
        repository.checkout(&"refs/heads/master".parse().unwrap());
        std::fs::write(repository.path().join("another.txt"), "").unwrap();
        repository.commit_all("another");
        repository.push_branch(&"refs/heads/master".parse().unwrap());
        repository.checkout(&"refs/heads/gitbutler/workspace".parse().unwrap());
    }

    {
        // merge branch into master
        gitbutler_branch_actions::push_virtual_branch(project, branch_id, false, None).unwrap();

        let branch = gitbutler_branch_actions::list_virtual_branches(project)
            .unwrap()
            .branches
            .into_iter()
            .find(|branch| branch.id == branch_id)
            .unwrap();

        assert!(branch.series[0].clone().unwrap().patches[0].is_local_and_remote);
        assert!(!branch.series[0].clone().unwrap().patches[0].is_integrated);
        assert!(branch.series[0].clone().unwrap().patches[1].is_local_and_remote);
        assert!(!branch.series[0].clone().unwrap().patches[1].is_integrated);

        repository.rebase_and_merge(&branch_name);
    }

    {
        // should mark commits as integrated
        gitbutler_branch_actions::fetch_from_remotes(project, None).unwrap();

        let branch = gitbutler_branch_actions::list_virtual_branches(project)
            .unwrap()
            .branches
            .into_iter()
            .find(|branch| branch.id == branch_id)
            .unwrap();

        assert_eq!(
            branch.series.first().unwrap().clone().unwrap().pr_number,
            Some(123)
        );

        assert!(branch.series[0].clone().unwrap().patches[0].is_local_and_remote);
        assert!(branch.series[0].clone().unwrap().patches[0].is_integrated);
        assert!(branch.series[0].clone().unwrap().patches[1].is_local_and_remote);
        assert!(branch.series[0].clone().unwrap().patches[1].is_integrated);
    }
}

#[test]
fn no_conflicts() {
    let Test {
        repository,
        project,
        ..
    } = &Test::default();

    {
        // create a remote branch
        let branch_name: LocalRefname = "refs/heads/branch".parse().unwrap();
        repository.checkout(&branch_name);
        fs::write(repository.path().join("file.txt"), "first").unwrap();
        repository.commit_all("first");
        repository.push_branch(&branch_name);
        repository.checkout(&"refs/heads/master".parse().unwrap());
    }

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    let branches = list_result.branches;

    assert!(branches.is_empty());

    let branch_id = gitbutler_branch_actions::create_virtual_branch_from_branch(
        project,
        &"refs/remotes/origin/branch".parse().unwrap(),
        None,
        None,
    )
    .unwrap();

    let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    let branches = list_result.branches;
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].id, branch_id);
    assert_eq!(branches[0].series[0].clone().unwrap().patches.len(), 1);
    assert_eq!(
        branches[0].series[0].clone().unwrap().patches[0].description,
        "first"
    );
}

#[test]
fn conflicts_with_uncommited() {
    let Test {
        repository,
        project,
        ..
    } = &Test::default();

    {
        // create a remote branch
        let branch_name: LocalRefname = "refs/heads/branch".parse().unwrap();
        repository.checkout(&branch_name);
        fs::write(repository.path().join("file.txt"), "first").unwrap();
        repository.commit_all("first");
        repository.push_branch(&branch_name);
        repository.checkout(&"refs/heads/master".parse().unwrap());
    }

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    // create a local branch that conflicts with remote
    {
        std::fs::write(repository.path().join("file.txt"), "conflict").unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
    };

    // branch should be created unapplied, because of the conflict

    let new_branch_id = gitbutler_branch_actions::create_virtual_branch_from_branch(
        project,
        &"refs/remotes/origin/branch".parse().unwrap(),
        None,
        None,
    )
    .unwrap();
    let new_branch = gitbutler_branch_actions::list_virtual_branches(project)
        .unwrap()
        .branches
        .into_iter()
        .find(|branch| branch.id == new_branch_id)
        .unwrap();
    assert_eq!(new_branch_id, new_branch.id);
    assert_eq!(new_branch.series[0].clone().unwrap().patches.len(), 1);
    assert!(new_branch.upstream.is_some());
}

#[test]
fn conflicts_with_commited() {
    let Test {
        repository,
        project,
        ..
    } = &Test::default();

    {
        // create a remote branch
        let branch_name: LocalRefname = "refs/heads/branch".parse().unwrap();
        repository.checkout(&branch_name);
        fs::write(repository.path().join("file.txt"), "first").unwrap();
        repository.commit_all("first");
        repository.push_branch(&branch_name);
        repository.checkout(&"refs/heads/master".parse().unwrap());
    }

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    // create a local branch that conflicts with remote
    {
        std::fs::write(repository.path().join("file.txt"), "conflict").unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);

        gitbutler_branch_actions::create_commit(project, branches[0].id, "hej", None).unwrap();
    };

    // branch should be created unapplied, because of the conflict

    let new_branch_id = gitbutler_branch_actions::create_virtual_branch_from_branch(
        project,
        &"refs/remotes/origin/branch".parse().unwrap(),
        None,
        None,
    )
    .unwrap();
    let new_branch = gitbutler_branch_actions::list_virtual_branches(project)
        .unwrap()
        .branches
        .into_iter()
        .find(|branch| branch.id == new_branch_id)
        .unwrap();
    assert_eq!(new_branch_id, new_branch.id);
    assert_eq!(new_branch.series[0].clone().unwrap().patches.len(), 1);
    assert!(new_branch.upstream.is_some());
}

#[test]
fn from_default_target() {
    let Test { project, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    // branch should be created unapplied, because of the conflict

    assert_eq!(
        gitbutler_branch_actions::create_virtual_branch_from_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
            None,
            None,
        )
        .unwrap_err()
        .to_string(),
        "cannot create a branch from default target"
    );
}

#[test]
fn from_non_existent_branch() {
    let Test { project, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    // branch should be created unapplied, because of the conflict

    assert_eq!(
        gitbutler_branch_actions::create_virtual_branch_from_branch(
            project,
            &"refs/remotes/origin/branch".parse().unwrap(),
            None,
            None,
        )
        .unwrap_err()
        .to_string(),
        "branch refs/remotes/origin/branch was not found"
    );
}

#[test]
fn from_state_remote_branch() {
    let Test {
        repository,
        project,
        ..
    } = &Test::default();

    {
        // create a remote branch
        let branch_name: LocalRefname = "refs/heads/branch".parse().unwrap();
        repository.checkout(&branch_name);
        fs::write(repository.path().join("file.txt"), "branch commit").unwrap();
        repository.commit_all("branch commit");
        repository.push_branch(&branch_name);
        repository.checkout(&"refs/heads/master".parse().unwrap());

        // make remote branch stale
        std::fs::write(repository.path().join("antoher_file.txt"), "master commit").unwrap();
        repository.commit_all("master commit");
        repository.push();
    }

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    let branch_id = gitbutler_branch_actions::create_virtual_branch_from_branch(
        project,
        &"refs/remotes/origin/branch".parse().unwrap(),
        None,
        None,
    )
    .unwrap();

    let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    let branches = list_result.branches;
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].id, branch_id);
    assert_eq!(branches[0].series[0].clone().unwrap().patches.len(), 1);
    assert!(branches[0].files.is_empty());
    assert_eq!(
        branches[0].series[0].clone().unwrap().patches[0].description,
        "branch commit"
    );
}

#[cfg(test)]
mod conflict_cases {
    use bstr::ByteSlice as _;
    use gitbutler_testsupport::testing_repository::{
        assert_commit_tree_matches, assert_tree_matches,
    };

    use super::*;

    /// Same setup as above, but with fearless rebasing, so we should end up
    /// with some conflicted commits.
    #[test]
    fn apply_mergable_but_not_rebasable_branch_with_fearless() {
        let Test {
            repository,
            project,
            ..
        } = &Test::default();

        let git_repository = &repository.local_repository;
        let signature = git2::Signature::now("caleb", "caleb@gitbutler.com").unwrap();

        let head_commit = git_repository.head().unwrap().peel_to_commit().unwrap();

        git_repository
            .reference("refs/remotes/origin/master", head_commit.id(), true, ":D")
            .unwrap();

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        // Make A and B and unapply them.
        fs::write(repository.path().join("foo.txt"), "a").unwrap();
        repository.commit_all("A");
        fs::remove_file(repository.path().join("foo.txt")).unwrap();
        fs::write(repository.path().join("bar.txt"), "b").unwrap();
        repository.commit_all("B");

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
        let branch = branches[0].clone();

        let branch_refname =
            gitbutler_branch_actions::save_and_unapply_virutal_branch(project, branch.id).unwrap();

        // Make X and set base branch to X
        let mut tree_builder = git_repository
            .treebuilder(Some(
                &git_repository.head().unwrap().peel_to_tree().unwrap(),
            ))
            .unwrap();
        let blob_oid = git_repository.blob("x".as_bytes()).unwrap();
        tree_builder
            .insert("foo.txt", blob_oid, git2::FileMode::Blob.into())
            .unwrap();

        git_repository
            .commit(
                Some("refs/remotes/origin/master"),
                &signature,
                &signature,
                "X",
                &git_repository
                    .find_tree(tree_builder.write().unwrap())
                    .unwrap(),
                &[&head_commit],
            )
            .unwrap();

        gitbutler_branch_actions::integrate_upstream(project, &[], None).unwrap();

        // Apply B

        gitbutler_branch_actions::create_virtual_branch_from_branch(
            project,
            &Refname::from_str(&branch_refname).unwrap(),
            None,
            None,
        )
        .unwrap();

        // We should see a merge commit
        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
        let branch = branches[0].clone();

        assert_eq!(
            branch.series[0].clone().unwrap().patches.len(),
            2,
            "Should have B' and A'"
        );

        assert_eq!(
            branch.series[0].clone().unwrap().patches[0]
                .description
                .to_str()
                .unwrap(),
            "B"
        );
        assert!(branch.series[0].clone().unwrap().patches[0].conflicted);
        let tree = repository
            .find_commit(branch.series[0].clone().unwrap().patches[0].id)
            .unwrap()
            .tree()
            .unwrap();
        assert_eq!(tree.len(), 6, "Five trees and the readme");
        assert_tree_matches(
            git_repository,
            &tree,
            &[
                (".auto-resolution/foo.txt", b"x"), // Has "ours" foo content
                (".auto-resolution/bar.txt", b"b"), // Has unconflicted "theirs" content
                (".conflict-base-0/foo.txt", b"a"), // A is base
                (".conflict-side-0/foo.txt", b"x"), // "Ours" is A'
                (".conflict-side-1/bar.txt", b"b"), // "Theirs" is B
            ],
        );

        assert_eq!(
            branch.series[0].clone().unwrap().patches[1]
                .description
                .to_str()
                .unwrap(),
            "A"
        );
        assert!(branch.series[0].clone().unwrap().patches[1].conflicted);
        assert_commit_tree_matches(
            git_repository,
            &repository
                .find_commit(branch.series[0].clone().unwrap().patches[1].id)
                .unwrap(),
            &[
                (".auto-resolution/foo.txt", b"x"), // Auto-resolves to X
                (".conflict-side-0/foo.txt", b"x"), // "Ours" is X
                (".conflict-side-1/foo.txt", b"a"), // "Theirs" is A
            ],
        );
    }
}
