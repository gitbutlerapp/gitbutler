use but_testsupport::legacy::testing_repository::TestingRepository;
use itertools::Itertools;

fn merge_base_octopussy(repo: &git2::Repository, ids: &[git2::Oid]) -> anyhow::Result<git2::Oid> {
    anyhow::ensure!(
        ids.len() >= 2,
        "Merge base octopussy requires at least two commit ids to operate on"
    );

    ids[1..].iter().try_fold(ids[0], |base, oid| {
        repo.merge_base(base, *oid).map_err(Into::into)
    })
}

#[test]
fn less_than_two_commits() {
    let test_repository = TestingRepository::open();

    let commit = test_repository.commit_tree(None, &[]);

    assert!(merge_base_octopussy(&test_repository.repository, &[commit.id()]).is_err());
}

#[test]
fn merge_base_of_two_linear_commits() {
    // Setup:
    // Base -> (A) -> (B)
    // Expected merge base: A

    let test_repository = TestingRepository::open();

    let base = test_repository.commit_tree(None, &[]);
    let a = test_repository.commit_tree(Some(&base), &[]);
    let b = test_repository.commit_tree(Some(&a), &[]);

    for permutation in [a.id(), b.id()].into_iter().permutations(2) {
        let merge_base = merge_base_octopussy(&test_repository.repository, &permutation).unwrap();

        assert_eq!(merge_base, a.id());
    }
}

#[test]
fn merge_base_of_three_linear_commits() {
    // Setup:
    // Base -> (A) -> (B) -> (C)
    // Expected merge base: A

    let test_repository = TestingRepository::open();

    let base = test_repository.commit_tree(None, &[]);
    let a = test_repository.commit_tree(Some(&base), &[]);
    let b = test_repository.commit_tree(Some(&a), &[]);
    let c = test_repository.commit_tree(Some(&b), &[]);

    for permutation in [a.id(), b.id(), c.id()].into_iter().permutations(3) {
        let merge_base = merge_base_octopussy(&test_repository.repository, &permutation).unwrap();

        assert_eq!(merge_base, a.id());
    }
}

#[test]
fn merge_base_of_two_parallel_commits() {
    // Setup:
    // Base -> (A)
    //     \-> (B)
    // Expected merge base: Base

    let test_repository = TestingRepository::open();

    let base = test_repository.commit_tree(None, &[]);
    let a = test_repository.commit_tree(Some(&base), &[]);
    let b = test_repository.commit_tree(Some(&base), &[]);

    for permutation in [a.id(), b.id()].into_iter().permutations(2) {
        let merge_base = merge_base_octopussy(&test_repository.repository, &permutation).unwrap();

        assert_eq!(merge_base, base.id());
    }
}

#[test]
fn merge_base_of_three_parallel_commits() {
    // Setup:
    // Base -> (A)
    //   \ \-> (B)
    //    \--> (C)
    // Expected merge base: Base

    let test_repository = TestingRepository::open();

    let base = test_repository.commit_tree(None, &[]);
    let a = test_repository.commit_tree(Some(&base), &[]);
    let b = test_repository.commit_tree(Some(&base), &[]);
    let c = test_repository.commit_tree(Some(&base), &[]);

    for permutation in [a.id(), b.id(), c.id()].into_iter().permutations(3) {
        let merge_base = merge_base_octopussy(&test_repository.repository, &permutation).unwrap();

        assert_eq!(merge_base, base.id());
    }
}

#[test]
fn merge_base_of_three_forked_commits() {
    // Setup:
    // Base -> x -> (A)
    //   \     \-> (B)
    //    \--> (C)
    // Expected merge base: Base

    let test_repository = TestingRepository::open();

    let base = test_repository.commit_tree(None, &[]);
    let x = test_repository.commit_tree(Some(&base), &[]);
    let a = test_repository.commit_tree(Some(&x), &[]);
    let b = test_repository.commit_tree(Some(&x), &[]);
    let c = test_repository.commit_tree(Some(&base), &[]);

    for permutation in [a.id(), b.id(), c.id()].into_iter().permutations(3) {
        let merge_base = merge_base_octopussy(&test_repository.repository, &permutation).unwrap();

        assert_eq!(merge_base, base.id());
    }
}
