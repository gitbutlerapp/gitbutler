#![deny(rust_2018_idioms)]
use std::collections::HashMap;

use anyhow::{bail, Context as _, Result};
use gitbutler_command_context::gix_repository_for_merging;
use gitbutler_commit::commit_ext::CommitExt as _;
use gitbutler_oxidize::{git2_to_gix_object_id, GixRepositoryExt as _, ObjectIdExt as _, OidExt};
use gitbutler_repo::{signature, GixRepositoryExt};
use itertools::Itertools as _;

use crate::commit_ops::{get_exclusive_tree, get_first_parent, is_subset, SubsetKind};

#[derive(Debug, PartialEq)]
enum SquashCollectionStatus {
    Complete,
    Incomplete,
}

#[derive(Debug, PartialEq)]
struct SquashCollection {
    /// The squahsed commit that we have found squashee commits for
    target: gix::ObjectId,
    /// The commits that we have found commits that look like
    /// they have been squahsed into the target
    /// commits are ordered from child-most to parent-most
    squashed: Vec<gix::ObjectId>,
    /// Whether the combined squashed commits exactly match the target
    /// commit
    status: SquashCollectionStatus,
}

impl SquashCollection {
    fn complete_match(&self) -> bool {
        matches!(self.status, SquashCollectionStatus::Complete)
    }

    fn is_squash(&self) -> bool {
        self.squashed.len() > 1
    }
}

/// Takes a list of commits and produces a combination of all of their exclusive diffs
///
/// Commits ordered from child-most to parent-most
fn combine_commits(
    repository: &gix::Repository,
    commits: &[gix::ObjectId],
    base_id: gix::ObjectId,
) -> Result<gix::ObjectId> {
    if commits.is_empty() {
        bail!("Combine commits should only be run on one or more commits")
    }
    let base = repository.find_commit(base_id)?;
    let base_tree_id = base.tree_id()?;
    let parent_most_commit = repository.find_commit(*commits.last().unwrap())?;
    let result_parent = get_first_parent(&parent_most_commit)?;

    let mut result = base;

    for commit in commits.iter().rev() {
        let exclusive_tree = get_exclusive_tree(repository, *commit, base_id)?;
        let merged_tree = repository
            .merge_trees(
                base_tree_id,
                exclusive_tree,
                result.tree_id()?,
                Default::default(),
                repository.merge_options_force_ours()?,
            )?
            .tree
            .write()?;

        let author = signature(gitbutler_repo::SignaturePurpose::Author)?;
        let committer = signature(gitbutler_repo::SignaturePurpose::Committer)?;
        let result_id = repository.commit_with_signature(
            None,
            &author,
            &committer,
            "",
            merged_tree.detach(),
            &[result_parent.id().detach()],
            None,
        )?;
        result = repository.find_commit(result_id)?;
    }

    Ok(result.id().detach())
}

/// This function returns a list of objects which coorespond to some of the
/// commits in the `rights` array. These objects describe which commits in
/// the `lefts` array are either equal to, or squashed into commits on the
/// right.
///
/// Commits ordered from child-most to parent-most and must be based off
/// of the base commit.
fn find_related_commits(
    repository: &gix::Repository,
    lefts: &[gix::ObjectId],
    rights: &[gix::ObjectId],
    base: gix::ObjectId,
) -> Result<Vec<SquashCollection>> {
    // For the comments inside this
    let mut squashes: Vec<SquashCollection> = vec![];

    for left in lefts {
        let mut found_supersets: Vec<SquashCollection> = vec![];

        for right in rights {
            // is left a subset of right
            match is_subset(repository, *right, *left, base)? {
                SubsetKind::Subset => found_supersets.push(SquashCollection {
                    target: *right,
                    squashed: vec![*left],
                    status: SquashCollectionStatus::Incomplete,
                }),
                SubsetKind::Equal => {
                    found_supersets.push(SquashCollection {
                        target: *right,
                        squashed: vec![*left],
                        status: SquashCollectionStatus::Complete,
                    });
                }
                _ => {}
            }
        }

        // Loop over all the existing found entries and see if the
        // commit can be part of those squash collections.
        for squash in squashes.iter_mut() {
            let commits = [squash.squashed.clone(), vec![*left]].concat();
            let combined_commit = combine_commits(repository, &commits, base)?;

            match is_subset(repository, squash.target, combined_commit, base)? {
                SubsetKind::Equal => {
                    squash.squashed = commits;
                    squash.status = SquashCollectionStatus::Complete;
                }
                SubsetKind::Subset => {
                    squash.squashed = commits;
                }
                _ => {}
            }
        }

        squashes.append(&mut found_supersets);
    }

    Ok(squashes)
}

#[derive(Debug, PartialEq)]
pub(crate) enum IntegrationStatus {
    Integrated {
        was_squashed: bool,
        is_squash: bool,
        complete_match: bool,
    },
    NotIntegrated,
}

impl IntegrationStatus {
    pub(crate) fn is_integrated(&self) -> bool {
        matches!(self, Self::Integrated { .. })
    }
}

pub(crate) type IntegrationStatuses = HashMap<gix::ObjectId, IntegrationStatus>;

pub(crate) trait IntegrationStatusesExt {
    fn is_integrated(&self, oid: gix::ObjectId) -> bool;
}

impl IntegrationStatusesExt for IntegrationStatuses {
    fn is_integrated(&self, oid: gix::ObjectId) -> bool {
        self.get(&oid)
            .map_or(false, |status| status.is_integrated())
    }
}

pub(crate) fn find_integrated_commits(
    repository: &gix::Repository,
    left: gix::ObjectId,
    right: gix::ObjectId,
) -> Result<IntegrationStatuses> {
    let repository = gix_repository_for_merging(repository.git_dir())?.with_object_memory();
    let base = repository.merge_base(left, right)?.detach();

    let lefts = repository
        .rev_walk([base, left])
        .first_parent_only()
        .with_pruned([base])
        .all()?
        .filter_map(|info| Some(info.ok()?.id))
        .collect::<Vec<_>>();

    let rights = repository
        .rev_walk([base, right])
        .first_parent_only()
        .with_pruned([base])
        .all()?
        .filter_map(|info| Some(info.ok()?.id))
        .collect::<Vec<_>>();

    dbg!(&lefts, &rights);

    let mut integration_statuses = lefts
        .iter()
        .map(|oid| (*oid, IntegrationStatus::NotIntegrated))
        .collect::<IntegrationStatuses>();

    let right_descriptions = find_related_commits(&repository, &lefts, &rights, base)?;
    let left_descriptions = find_related_commits(&repository, &rights, &lefts, base)?;

    for description in &right_descriptions {
        for commit in &description.squashed {
            if let Some(status) = integration_statuses.get_mut(commit) {
                if !status.is_integrated() {
                    *status = IntegrationStatus::Integrated {
                        was_squashed: false,
                        is_squash: false,
                        complete_match: false,
                    };
                }

                if let IntegrationStatus::Integrated {
                    was_squashed: in_squash,
                    complete_match,
                    ..
                } = status
                {
                    *in_squash = *in_squash || description.is_squash();
                    *complete_match = *complete_match || description.complete_match();
                }
            }
        }
    }

    for description in left_descriptions {
        // if !(/*description.complete_match()||*/description.is_squash()) {
        //     continue;
        // }

        if let Some(status) = integration_statuses.get_mut(&description.target) {
            *status = IntegrationStatus::Integrated {
                was_squashed: false,
                is_squash: true,
                complete_match: true,
            }
        }
    }

    Ok(integration_statuses)
}

type MergeBaseCommitGraph<'repo, 'cache> = gix::revwalk::Graph<
    'repo,
    'cache,
    gix::revision::plumbing::graph::Commit<gix::revision::plumbing::merge_base::Flags>,
>;

pub(crate) struct IsCommitIntegrated<'repo, 'cache, 'graph> {
    gix_repo: &'repo gix::Repository,
    graph: &'graph mut MergeBaseCommitGraph<'repo, 'cache>,
    target_commit_id: gix::ObjectId,
    upstream_tree_id: gix::ObjectId,
    upstream_commits: Vec<git2::Oid>,
    upstream_change_ids: Vec<String>,
}

impl<'repo, 'cache, 'graph> IsCommitIntegrated<'repo, 'cache, 'graph> {
    fn new_basic(
        gix_repository: &'repo gix::Repository,
        repository: &'repo git2::Repository,
        graph: &'graph mut MergeBaseCommitGraph<'repo, 'cache>,
        target_commit_id: gix::ObjectId,
        upstream_tree_id: gix::ObjectId,
        upstream_commits: Vec<gix::ObjectId>,
    ) -> Self {
        // Ensure upstream commits are sorted for binary search
        let mut upstream_commits = upstream_commits
            .into_iter()
            .map(|oid| oid.to_git2())
            .collect::<Vec<_>>();
        upstream_commits.sort();
        let upstream_change_ids = upstream_commits
            .iter()
            .filter_map(|oid| {
                let commit = repository.find_commit(*oid).ok()?;
                commit.change_id()
            })
            .sorted()
            .collect();
        Self {
            gix_repo: gix_repository,
            graph,
            target_commit_id,
            upstream_tree_id,
            upstream_commits,
            upstream_change_ids,
        }
    }
}

impl IsCommitIntegrated<'_, '_, '_> {
    fn is_integrated(&mut self, commit: &git2::Commit<'_>) -> Result<bool> {
        if self.target_commit_id == git2_to_gix_object_id(commit.id()) {
            // could not be integrated if heads are the same.
            return Ok(false);
        }

        if self.upstream_commits.is_empty() {
            // could not be integrated - there is nothing new upstream.
            return Ok(false);
        }

        if let Some(change_id) = commit.change_id() {
            if self.upstream_change_ids.binary_search(&change_id).is_ok() {
                return Ok(true);
            }
        }

        if self.upstream_commits.binary_search(&commit.id()).is_ok() {
            return Ok(true);
        }

        let merge_base_id = self.gix_repo.merge_base_with_graph(
            self.target_commit_id,
            commit.id().to_gix(),
            self.graph,
        )?;
        if merge_base_id.detach().to_git2().eq(&commit.id()) {
            // if merge branch is the same as branch head and there are upstream commits
            // then it's integrated
            return Ok(true);
        }

        let merge_base_tree_id = self.gix_repo.find_commit(merge_base_id)?.tree_id()?;
        if merge_base_tree_id == self.upstream_tree_id {
            // if merge base is the same as upstream tree, then it's integrated
            return Ok(true);
        }

        // try to merge our tree into the upstream tree
        let (merge_options, conflict_kind) = self.gix_repo.merge_options_no_rewrites_fail_fast()?;
        let mut merge_output = self
            .gix_repo
            .merge_trees(
                merge_base_tree_id,
                commit.tree_id().to_gix(),
                self.upstream_tree_id,
                Default::default(),
                merge_options,
            )
            .context("failed to merge trees")?;

        if merge_output.has_unresolved_conflicts(conflict_kind) {
            return Ok(false);
        }

        let merge_tree_id = merge_output.tree.write()?.detach();

        // if the merge_tree is the same as the new_target_tree and there are no files (uncommitted changes)
        // then the vbranch is fully merged
        Ok(merge_tree_id == self.upstream_tree_id)
    }
}

pub(crate) fn compat_find_integrated_commits<'repo>(
    gix_repository: &'repo gix::Repository,
    repository: &'repo git2::Repository,
    graph: &'_ mut MergeBaseCommitGraph<'repo, '_>,
    target_commit_id: gix::ObjectId,
    upstream_commit_id: gix::ObjectId,
    stack_head: gix::ObjectId,
    use_new: bool,
) -> Result<IntegrationStatuses> {
    if use_new {
        find_integrated_commits(gix_repository, stack_head, target_commit_id)
    } else {
        let upstream_commit = gix_repository.find_commit(upstream_commit_id)?;
        let base = gix_repository
            .merge_base(stack_head, upstream_commit_id)?
            .detach();

        let upstream_commits = gix_repository
            .rev_walk([upstream_commit_id])
            .with_pruned([base])
            .all()?
            .filter_map(|info| Some(info.ok()?.id))
            .collect::<Vec<_>>();

        let mut is_commit_integrated = IsCommitIntegrated::new_basic(
            gix_repository,
            repository,
            graph,
            target_commit_id,
            upstream_commit.tree_id()?.detach(),
            upstream_commits,
        );

        let stack_commits = gix_repository
            .rev_walk([stack_head])
            .first_parent_only()
            .with_pruned([base])
            .all()?
            .filter_map(|info| Some(info.ok()?.id))
            .collect::<Vec<_>>();

        let mut integration_statuses = stack_commits
            .iter()
            .map(|oid| (*oid, IntegrationStatus::NotIntegrated))
            .collect::<IntegrationStatuses>();

        for commit_id in stack_commits {
            let commit = repository.find_commit(commit_id.to_git2())?;
            if is_commit_integrated.is_integrated(&commit)? {
                if let Some(integration_status) = integration_statuses.get_mut(&commit_id) {
                    *integration_status = IntegrationStatus::Integrated {
                        was_squashed: false,
                        is_squash: false,
                        // We don't really know here, but this information is
                        // not actually used so I'm just setting it to false.
                        complete_match: false,
                    }
                }
            }
        }

        Ok(integration_statuses)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use gitbutler_testsupport::testing_repository::TestingRepository;
    mod find_related_commits {
        use super::*;
        #[test]
        fn unrelated_commits_dont_have_matches() {
            let test_repository = TestingRepository::open();
            let base_commit: git2::Commit<'_> =
                test_repository.commit_tree(None, &[("foo.txt", "foo")]);
            let a: git2::Commit<'_> =
                test_repository.commit_tree(Some(&base_commit), &[("foo.txt", "bar")]);
            let b: git2::Commit<'_> = test_repository.commit_tree(Some(&a), &[("foo.txt", "baz")]);
            let x: git2::Commit<'_> =
                test_repository.commit_tree(Some(&base_commit), &[("foo.txt", "qux")]);
            let y: git2::Commit<'_> = test_repository.commit_tree(Some(&x), &[("foo.txt", "biz")]);

            assert_eq!(
                find_related_commits(
                    &test_repository.gix_repository(),
                    &[a.id().to_gix(), b.id().to_gix()],
                    &[x.id().to_gix(), y.id().to_gix()],
                    base_commit.id().to_gix()
                )
                .unwrap(),
                vec![]
            )
        }

        #[test]
        fn directly_related_commits_align() {
            let test_repository = TestingRepository::open();
            let base_commit: git2::Commit<'_> =
                test_repository.commit_tree(None, &[("foo.txt", "foo")]);
            let a: git2::Commit<'_> =
                test_repository.commit_tree(Some(&base_commit), &[("foo.txt", "bar")]);
            let b: git2::Commit<'_> = test_repository.commit_tree(Some(&a), &[("foo.txt", "baz")]);
            let x: git2::Commit<'_> =
                test_repository.commit_tree(Some(&base_commit), &[("foo.txt", "bar")]);
            let y: git2::Commit<'_> = test_repository.commit_tree(Some(&x), &[("foo.txt", "baz")]);

            assert_eq!(
                find_related_commits(
                    &test_repository.gix_repository(),
                    &[b.id().to_gix(), a.id().to_gix()],
                    &[y.id().to_gix(), x.id().to_gix()],
                    base_commit.id().to_gix()
                )
                .unwrap(),
                vec![
                    SquashCollection {
                        target: y.id().to_gix(),
                        // A could have ben squashed into Y
                        squashed: vec![b.id().to_gix(), a.id().to_gix()],
                        status: SquashCollectionStatus::Complete
                    },
                    SquashCollection {
                        target: x.id().to_gix(),
                        squashed: vec![a.id().to_gix()],
                        status: SquashCollectionStatus::Complete
                    },
                ]
            )
        }

        #[test]
        fn out_of_order_still_found() {
            let test_repository = TestingRepository::open();
            let base_commit: git2::Commit<'_> =
                test_repository.commit_tree(None, &[("foo.txt", "foo")]);
            let a: git2::Commit<'_> =
                test_repository.commit_tree(Some(&base_commit), &[("foo.txt", "bar")]);
            let b: git2::Commit<'_> = test_repository.commit_tree(Some(&a), &[("foo.txt", "baz")]);
            let x: git2::Commit<'_> =
                test_repository.commit_tree(Some(&base_commit), &[("foo.txt", "baz")]);
            let y: git2::Commit<'_> = test_repository.commit_tree(Some(&x), &[("foo.txt", "bar")]);

            assert_eq!(
                find_related_commits(
                    &test_repository.gix_repository(),
                    &[b.id().to_gix(), a.id().to_gix()],
                    &[y.id().to_gix(), x.id().to_gix()],
                    base_commit.id().to_gix()
                )
                .unwrap(),
                vec![
                    SquashCollection {
                        target: x.id().to_gix(),
                        // A could have also been squashed into X
                        squashed: vec![b.id().to_gix(), a.id().to_gix()],
                        status: SquashCollectionStatus::Complete
                    },
                    SquashCollection {
                        target: y.id().to_gix(),
                        squashed: vec![a.id().to_gix()],
                        status: SquashCollectionStatus::Complete
                    },
                ]
            )
        }

        #[test]
        fn related_on_different_bases() {
            let test_repository = TestingRepository::open();
            let base_commit: git2::Commit<'_> =
                test_repository.commit_tree(None, &[("foo.txt", "foo")]);
            let a: git2::Commit<'_> =
                test_repository.commit_tree(Some(&base_commit), &[("foo.txt", "qux")]);
            let b: git2::Commit<'_> = test_repository.commit_tree(Some(&a), &[("foo.txt", "baz")]);
            let x: git2::Commit<'_> =
                test_repository.commit_tree(Some(&base_commit), &[("foo.txt", "fux")]);
            let y: git2::Commit<'_> = test_repository.commit_tree(Some(&x), &[("foo.txt", "baz")]);

            assert_eq!(
                find_related_commits(
                    &test_repository.gix_repository(),
                    &[b.id().to_gix(), a.id().to_gix()],
                    &[y.id().to_gix(), x.id().to_gix()],
                    base_commit.id().to_gix()
                )
                .unwrap(),
                vec![SquashCollection {
                    target: y.id().to_gix(),
                    // A is considered squashed, because it's changes are superceded by B
                    squashed: vec![b.id().to_gix(), a.id().to_gix()],
                    status: SquashCollectionStatus::Complete
                },]
            )
        }

        #[test]
        fn squashed_commits() {
            let test_repository = TestingRepository::open();
            let base_commit: git2::Commit<'_> =
                test_repository.commit_tree(None, &[("foo.txt", "foo")]);
            let a: git2::Commit<'_> =
                test_repository.commit_tree(Some(&base_commit), &[("foo.txt", "bar")]);
            let b: git2::Commit<'_> = test_repository.commit_tree(Some(&a), &[("foo.txt", "baz")]);
            let x: git2::Commit<'_> =
                test_repository.commit_tree(Some(&base_commit), &[("foo.txt", "baz")]);

            assert_eq!(
                find_related_commits(
                    &test_repository.gix_repository(),
                    &[b.id().to_gix(), a.id().to_gix()],
                    &[x.id().to_gix()],
                    base_commit.id().to_gix()
                )
                .unwrap(),
                vec![SquashCollection {
                    target: x.id().to_gix(),
                    squashed: vec![b.id().to_gix(), a.id().to_gix()],
                    status: SquashCollectionStatus::Complete
                }]
            )
        }

        #[test]
        fn not_squashed_commits() {
            let test_repository = TestingRepository::open();
            let base_commit: git2::Commit<'_> =
                test_repository.commit_tree(None, &[("foo.txt", "foo")]);
            let a: git2::Commit<'_> = test_repository.commit_tree(
                Some(&base_commit),
                &[("foo.txt", "bar"), ("bar.txt", "asdf")],
            );
            let b: git2::Commit<'_> = test_repository.commit_tree(Some(&a), &[("foo.txt", "baz")]);
            let x: git2::Commit<'_> =
                test_repository.commit_tree(Some(&base_commit), &[("foo.txt", "baz")]);

            assert_eq!(
                find_related_commits(
                    &test_repository.gix_repository(),
                    &[b.id().to_gix(), a.id().to_gix()],
                    &[x.id().to_gix()],
                    base_commit.id().to_gix()
                )
                .unwrap(),
                vec![SquashCollection {
                    target: x.id().to_gix(),
                    squashed: vec![b.id().to_gix()],
                    status: SquashCollectionStatus::Complete
                }]
            )
        }

        #[test]
        fn complex() {
            let test_repository = TestingRepository::open();
            let base_commit = test_repository.commit_tree(None, &[("foo.txt", "foo")]);
            let p = test_repository.commit_tree(Some(&base_commit), &[("p", "p")]);
            let a = test_repository.commit_tree(Some(&p), &[("p", "p"), ("a", "a")]);
            let b = test_repository.commit_tree(Some(&a), &[("p", "p"), ("a", "1"), ("b", "b")]);
            let q = test_repository
                .commit_tree(Some(&b), &[("p", "p"), ("a", "2"), ("b", "b"), ("q", "q")]);
            let r = test_repository.commit_tree(
                Some(&q),
                &[("p", "p"), ("a", "3"), ("b", "b"), ("q", "q"), ("r", "r")],
            );
            let c = test_repository.commit_tree(
                Some(&r),
                &[
                    ("p", "p"),
                    ("a", "4"),
                    ("b", "b"),
                    ("q", "q"),
                    ("r", "r"),
                    ("c", "c"),
                ],
            );
            let d = test_repository.commit_tree(
                Some(&c),
                &[
                    ("p", "p"),
                    ("a", "5"),
                    ("b", "b"),
                    ("q", "q"),
                    ("r", "r"),
                    ("c", "c"),
                    ("d", "d"),
                ],
            );
            let e = test_repository.commit_tree(
                Some(&d),
                &[
                    ("p", "p"),
                    ("a", "6"),
                    ("b", "b"),
                    ("q", "q"),
                    ("r", "r"),
                    ("c", "c"),
                    ("d", "d"),
                    ("e", "e"),
                ],
            );

            let x = test_repository.commit_tree(Some(&base_commit), &[("x", "x")]);
            let ab = test_repository.commit_tree(Some(&x), &[("x", "x"), ("a", "1"), ("b", "b")]);
            let y = test_repository.commit_tree(
                Some(&ab),
                &[("x", "x"), ("a", "420"), ("b", "b"), ("y", "y")],
            );
            let cde = test_repository.commit_tree(
                Some(&y),
                &[
                    ("x", "x"),
                    ("a", "6"),
                    ("b", "b"),
                    ("y", "y"),
                    ("c", "c"),
                    ("d", "d"),
                    ("e", "e"),
                ],
            );

            assert_eq!(
                dbg!(find_related_commits(
                    &test_repository.gix_repository(),
                    &[
                        e.id().to_gix(),
                        d.id().to_gix(),
                        c.id().to_gix(),
                        r.id().to_gix(),
                        q.id().to_gix(),
                        b.id().to_gix(),
                        a.id().to_gix(),
                        p.id().to_gix(),
                    ],
                    &[
                        cde.id().to_gix(),
                        y.id().to_gix(),
                        ab.id().to_gix(),
                        x.id().to_gix()
                    ],
                    base_commit.id().to_gix()
                )
                .unwrap()),
                dbg!(vec![
                    SquashCollection {
                        target: cde.id().to_gix(),
                        // A *could* have also been squashed into CDE
                        squashed: vec![
                            e.id().to_gix(),
                            d.id().to_gix(),
                            c.id().to_gix(),
                            a.id().to_gix()
                        ],
                        status: SquashCollectionStatus::Complete
                    },
                    SquashCollection {
                        target: ab.id().to_gix(),
                        squashed: vec![b.id().to_gix(), a.id().to_gix()],
                        status: SquashCollectionStatus::Complete
                    }
                ])
            )
        }
    }

    mod find_integrated_commits {
        use super::*;

        #[test]
        fn squashy_squashy() {
            let test_repository = TestingRepository::open();
            let base_commit: git2::Commit<'_> =
                test_repository.commit_tree(None, &[("foo.txt", "foo")]);
            let a: git2::Commit<'_> =
                test_repository.commit_tree(Some(&base_commit), &[("foo.txt", "bar")]);
            let bc: git2::Commit<'_> = test_repository.commit_tree(Some(&a), &[("foo.txt", "qux")]);

            let ab: git2::Commit<'_> =
                test_repository.commit_tree(Some(&base_commit), &[("foo.txt", "baz")]);
            let c: git2::Commit<'_> = test_repository.commit_tree(Some(&ab), &[("foo.txt", "qux")]);

            let integration_statuses = find_integrated_commits(
                &test_repository.gix_repository(),
                bc.id().to_gix(),
                c.id().to_gix(),
            )
            .unwrap();

            assert_eq!(integration_statuses.len(), 2);
            assert_eq!(
                *integration_statuses.get(&a.id().to_gix()).unwrap(),
                IntegrationStatus::Integrated {
                    was_squashed: true,
                    is_squash: false,
                    complete_match: true
                }
            );
            assert_eq!(
                *integration_statuses.get(&bc.id().to_gix()).unwrap(),
                IntegrationStatus::Integrated {
                    was_squashed: false,
                    is_squash: true,
                    complete_match: true
                }
            );
        }
    }
}
