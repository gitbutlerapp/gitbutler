use anyhow::{bail, Result};
use gitbutler_oxidize::GixRepositoryExt;

/// Finds the first parent of a given commit.
pub(crate) fn get_first_parent<'repo>(commit: &gix::Commit<'repo>) -> Result<gix::Commit<'repo>> {
    let Some(first_parent) = commit.parent_ids().next() else {
        bail!("Failed to find first parent of {}", commit.id())
    };
    let first_parent = first_parent.object()?.into_commit();
    Ok(first_parent)
}

/// Gets the changes that one commit introduced compared to the base,
/// excluding anything between the commit and the base.
pub(crate) fn get_exclusive_tree(
    repository: &gix::Repository,
    commit_id: gix::ObjectId,
    base_id: gix::ObjectId,
) -> Result<gix::ObjectId> {
    let commit = repository.find_commit(commit_id)?;
    let commit_parent = get_first_parent(&commit)?;
    let base = repository.find_commit(base_id)?;

    let merged_tree = repository
        .merge_trees(
            commit_parent.tree_id()?,
            commit.tree_id()?,
            base.tree_id()?,
            Default::default(),
            repository.merge_options_force_ours()?,
        )?
        .tree
        .write()?;
    Ok(merged_tree.into())
}

#[derive(PartialEq, Debug)]
pub(crate) enum SubsetKind {
    /// The subset_id is not equal to or a subset of superset_id.
    /// superset_id MAY still be a strict subset of subset_id
    NotSubset,
    /// The subset_id is a strict subset of superset_id
    Subset,
    /// The subset_id and superset_id are equivalent commits
    Equal,
}

/// Takes two commits and determines if one is a subset of or equal to the other.
///
/// ### Performance
///
/// `repository` should have been configured [`with_object_memory()`](gix::Repository::with_object_memory())
/// to prevent real objects to be written while probing for set inclusion.
#[allow(dead_code)]
pub(crate) fn is_subset(
    repository: &gix::Repository,
    superset_id: gix::ObjectId,
    subset_id: gix::ObjectId,
    common_base_id: gix::ObjectId,
) -> Result<SubsetKind> {
    let exclusive_superset = get_exclusive_tree(repository, superset_id, common_base_id)?;
    let exclusive_subset = get_exclusive_tree(repository, subset_id, common_base_id)?;

    if exclusive_superset == exclusive_subset {
        return Ok(SubsetKind::Equal);
    }

    let common_base = repository.find_commit(common_base_id)?;

    let (options, unresolved) = repository.merge_options_fail_fast()?;
    let mut merged_exclusives = repository.merge_trees(
        common_base.tree_id()?,
        exclusive_superset,
        exclusive_subset,
        Default::default(),
        options,
    )?;

    if merged_exclusives.has_unresolved_conflicts(unresolved)
        || exclusive_superset != merged_exclusives.tree.write()?
    {
        Ok(SubsetKind::NotSubset)
    } else {
        Ok(SubsetKind::Subset)
    }
}

#[cfg(test)]
mod test {
    use gitbutler_testsupport::testing_repository::TestingRepository;
    mod get_exclusive_tree {
        use gitbutler_oxidize::OidExt;

        use super::super::get_exclusive_tree;
        use super::*;

        #[test]
        fn when_already_exclusive_returns_self() {
            let test_repository = TestingRepository::open();
            let base_commit: git2::Commit =
                test_repository.commit_tree(None, &[("foo.txt", "foo"), ("bar.txt", "bar")]);
            let second_commit: git2::Commit = test_repository.commit_tree(
                Some(&base_commit),
                &[("foo.txt", "bar"), ("bar.txt", "baz")],
            );

            let exclusive_tree = get_exclusive_tree(
                &test_repository.gix_repository(),
                second_commit.id().to_gix(),
                base_commit.id().to_gix(),
            )
            .unwrap();

            assert_eq!(
                second_commit.tree_id().to_gix(),
                exclusive_tree,
                "The tree returned should match the second commit"
            )
        }

        #[test]
        fn when_on_top_of_other_commit_its_changes_are_dropped() {
            let test_repository = TestingRepository::open();
            let base_commit: git2::Commit =
                test_repository.commit_tree(None, &[("foo.txt", "foo")]);
            let second_commit: git2::Commit = test_repository.commit_tree(
                Some(&base_commit),
                &[("foo.txt", "bar"), ("bar.txt", "baz")],
            );
            let third_commit: git2::Commit = test_repository.commit_tree(
                Some(&second_commit),
                &[("foo.txt", "bar"), ("bar.txt", "baz"), ("qux.txt", "bax")],
            );

            // The second commit changed foo.txt, and added bar.txt. We expect
            // foo.txt to be reverted back to foo, and bar.txt should be dropped
            let expected_commit: git2::Commit =
                test_repository.commit_tree(None, &[("foo.txt", "foo"), ("qux.txt", "bax")]);

            let exclusive_tree = get_exclusive_tree(
                &test_repository.gix_repository(),
                third_commit.id().to_gix(),
                base_commit.id().to_gix(),
            )
            .unwrap();

            assert_eq!(expected_commit.tree_id().to_gix(), exclusive_tree,)
        }
    }

    mod is_subset {
        use gitbutler_oxidize::OidExt;

        use crate::commit_ops::SubsetKind;

        use super::super::is_subset;
        use super::*;

        #[test]
        fn a_commit_is_a_subset_of_itself() {
            let test_repository = TestingRepository::open();
            let base_commit: git2::Commit =
                test_repository.commit_tree(None, &[("foo.txt", "foo")]);
            let second_commit: git2::Commit = test_repository.commit_tree(
                Some(&base_commit),
                &[("foo.txt", "bar"), ("bar.txt", "baz")],
            );

            assert_eq!(
                is_subset(
                    &test_repository.gix_repository(),
                    second_commit.id().to_gix(),
                    second_commit.id().to_gix(),
                    base_commit.id().to_gix()
                )
                .unwrap(),
                SubsetKind::Equal
            )
        }

        #[test]
        fn basic_subset() {
            let test_repository = TestingRepository::open();
            let base_commit: git2::Commit =
                test_repository.commit_tree(None, &[("foo.txt", "foo")]);
            let superset: git2::Commit = test_repository.commit_tree(
                Some(&base_commit),
                &[("foo.txt", "bar"), ("bar.txt", "baz"), ("baz.txt", "asdf")],
            );
            let subset: git2::Commit = test_repository.commit_tree(
                Some(&base_commit),
                &[("foo.txt", "bar"), ("bar.txt", "baz")],
            );

            assert_eq!(
                is_subset(
                    &test_repository.gix_repository(),
                    superset.id().to_gix(),
                    subset.id().to_gix(),
                    base_commit.id().to_gix()
                )
                .unwrap(),
                SubsetKind::Subset
            );

            assert_eq!(
                is_subset(
                    &test_repository.gix_repository(),
                    subset.id().to_gix(),
                    superset.id().to_gix(),
                    base_commit.id().to_gix()
                )
                .unwrap(),
                SubsetKind::NotSubset
            );
        }

        #[test]
        fn complex_subset() {
            let test_repository = TestingRepository::open();
            let base_commit: git2::Commit =
                test_repository.commit_tree(None, &[("foo.txt", "foo")]);
            let i1: git2::Commit = test_repository.commit_tree(
                Some(&base_commit),
                &[("foo.txt", "baz"), ("amp.txt", "asfd")],
            );
            let superset: git2::Commit = test_repository.commit_tree(
                Some(&i1),
                &[
                    ("foo.txt", "baz"),
                    ("amp.txt", "asfd"),
                    ("bar.txt", "baz"),
                    ("baz.txt", "asdf"),
                ],
            );
            let i2: git2::Commit = test_repository.commit_tree(
                Some(&base_commit),
                &[("foo.txt", "xxx"), ("fuzz.txt", "asdf")],
            );
            let subset: git2::Commit = test_repository.commit_tree(
                Some(&i2),
                &[("foo.txt", "xxx"), ("fuzz.txt", "asdf"), ("bar.txt", "baz")],
            );

            // This creates two commits "superset" and "subset" which when
            // compared directly don't have a superset-subset relationship,
            // but because we take the changes that the subset/superset commits
            // exclusivly added compared to a common base, we are able to
            // identify that the changes each commit introduced are infact
            // a superset/subset of each other

            assert_eq!(
                is_subset(
                    &test_repository.gix_repository(),
                    superset.id().to_gix(),
                    subset.id().to_gix(),
                    base_commit.id().to_gix()
                )
                .unwrap(),
                SubsetKind::Subset
            );

            assert_eq!(
                is_subset(
                    &test_repository.gix_repository(),
                    subset.id().to_gix(),
                    superset.id().to_gix(),
                    base_commit.id().to_gix()
                )
                .unwrap(),
                SubsetKind::NotSubset
            );
        }
    }
}
