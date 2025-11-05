use gitbutler_repo::RepositoryExt as _;
use gitbutler_testsupport::testing_repository::TestingRepository;
use itertools::Itertools;

#[test]
fn less_than_two_commits() {
    let test_repository = TestingRepository::open();

    let commit = test_repository.commit_tree(None, &[]);

    assert!(
        test_repository
            .repository
            .merge_base_octopussy(&[commit.id()])
            .is_err()
    );
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
        let merge_base = test_repository
            .repository
            .merge_base_octopussy(&permutation)
            .unwrap();

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
        let merge_base = test_repository
            .repository
            .merge_base_octopussy(&permutation)
            .unwrap();

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
        let merge_base = test_repository
            .repository
            .merge_base_octopussy(&permutation)
            .unwrap();

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
        let merge_base = test_repository
            .repository
            .merge_base_octopussy(&permutation)
            .unwrap();

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
        let merge_base = test_repository
            .repository
            .merge_base_octopussy(&permutation)
            .unwrap();

        assert_eq!(merge_base, base.id());
    }
}
