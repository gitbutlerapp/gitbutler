use anyhow::{anyhow, bail, Result};
use gitbutler_command_context::CommandContext;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_repo::{
    rebase::{cherry_rebase_group, gitbutler_merge_commits},
    LogUntil, RepositoryExt as _,
};
use gitbutler_stack::commit_by_oid_or_change_id;
use gitbutler_stack::StackId;
use gitbutler_workspace::{
    checkout_branch_trees, compute_updated_branch_head_for_commits, BranchHeadAndTree,
};

use crate::{conflicts, VirtualBranchesExt as _};

pub fn integrate_upstream_commits_for_series(
    ctx: &CommandContext,
    stack_id: StackId,
    perm: &mut WorktreeWritePermission,
    series_name: String,
) -> Result<()> {
    conflicts::is_conflicting(ctx, None)?;

    let repo = ctx.repository();
    let vb_state = ctx.project().virtual_branches();

    let stack = vb_state.get_stack_in_workspace(stack_id)?;
    let branches = stack.branches();

    let default_target = vb_state.get_default_target()?;
    let remote = default_target.push_remote_name();

    let subject_branch = branches
        .iter()
        .find(|branch| branch.name == series_name)
        .ok_or(anyhow!("Series not found"))?;
    let upstream_reference = subject_branch.remote_reference(remote.as_str())?;
    let remote_head = repo.find_reference(&upstream_reference)?.peel_to_commit()?;

    let stack_merge_base = repo.merge_base(stack.head(), default_target.sha)?;
    let series_head = commit_by_oid_or_change_id(
        &subject_branch.head,
        repo,
        remote_head.id(),
        stack_merge_base,
    )?
    .head;

    let do_rebease = stack.allow_rebasing
        || Some(subject_branch.name.clone()) != branches.first().map(|b| b.name.clone());
    let integrate_upstream_context = IntegrateUpstreamContext {
        repository: repo,
        target_branch_head: default_target.sha,
        branch_head: stack.head(),
        branch_tree: stack.tree,
        branch_name: &subject_branch.name,
        remote_head: remote_head.id(),
        remote_branch_name: &subject_branch.remote_reference(&remote)?,
        prefers_merge: !do_rebease,
    };

    let (BranchHeadAndTree { head, tree }, new_series_head) =
        integrate_upstream_context.inner_integrate_upstream_commits_for_series(series_head.id())?;

    let mut branch = stack.clone();
    branch.set_stack_head(ctx, head, Some(tree))?;
    checkout_branch_trees(ctx, perm)?;
    branch.replace_head(ctx, &series_head, &repo.find_commit(new_series_head)?)?;
    crate::integration::update_workspace_commit(&vb_state, ctx)?;
    Ok(())
}

/// Integrates upstream work from a remote branch.
///
/// Any to-be integrated commits that are upstream will be placed at the bottom
/// of the branch. Any other upstream commits are placed above the local
/// commits.
///
pub fn integrate_upstream_commits(
    ctx: &CommandContext,
    stack_id: StackId,
    perm: &mut WorktreeWritePermission,
) -> Result<()> {
    conflicts::is_conflicting(ctx, None)?;

    let repository = ctx.repository();
    let project = ctx.project();
    let vb_state = project.virtual_branches();

    let stack = vb_state.get_stack_in_workspace(stack_id)?;

    let Some(upstream_refname) = stack.clone().upstream else {
        bail!("No upstream reference found for branch");
    };

    let upstream_branch = repository.find_branch_by_refname(&upstream_refname.into())?;
    let upstream_branch_head = upstream_branch.get().peel_to_commit()?.id();

    // If the upstream branch head is the same as the local, then the branch is
    // up to date.
    if upstream_branch_head == stack.head() {
        return Ok(());
    }

    let default_target = vb_state.get_default_target()?;
    let default_target_branch = repository.find_branch_by_refname(&default_target.branch.into())?;
    let target_branch_head = default_target_branch.get().peel_to_commit()?.id();

    let integrate_upstream_context = IntegrateUpstreamContext {
        repository,
        target_branch_head,
        branch_head: stack.head(),
        branch_tree: stack.tree,
        branch_name: &stack.name,
        remote_head: upstream_branch_head,
        remote_branch_name: upstream_branch.name()?.unwrap_or("Unknown"),
        prefers_merge: !stack.allow_rebasing,
    };

    let BranchHeadAndTree { head, tree } =
        integrate_upstream_context.inner_integrate_upstream_commits()?;

    let mut stack = stack.clone();

    stack.set_stack_head(ctx, head, Some(tree))?;

    checkout_branch_trees(ctx, perm)?;

    crate::integration::update_workspace_commit(&vb_state, ctx)?;

    Ok(())
}

struct IntegrateUpstreamContext<'a, 'b> {
    repository: &'a git2::Repository,
    /// GitButler's target branch
    target_branch_head: git2::Oid,

    /// The local branch head
    branch_head: git2::Oid,
    /// The uncommited changes associated to the branch
    branch_tree: git2::Oid,
    /// The name of the local branch
    branch_name: &'b str,

    /// The remote branch head
    remote_head: git2::Oid,
    /// The name of the remote branch
    remote_branch_name: &'b str,

    /// Whether to merge or rebase
    prefers_merge: bool,
}

impl IntegrateUpstreamContext<'_, '_> {
    /// Unlike the `inner_integrate_upstream_commits` method, this will do the rebase in two steps.
    /// First it will rebase the series head and it's remote commits, then it will rebase any remaining on the stack.
    fn inner_integrate_upstream_commits_for_series(
        &self,
        series_head: git2::Oid,
    ) -> Result<(BranchHeadAndTree, git2::Oid)> {
        let (new_stack_head, new_series_head) = if self.prefers_merge {
            // If rebase is not allowed AND this is the latest series - create a merge commit on top
            let series_head_commit = self.repository.find_commit(series_head)?;
            let remote_head_commit = self.repository.find_commit(self.remote_head)?;
            let merge_commit = gitbutler_merge_commits(
                self.repository,
                series_head_commit,
                remote_head_commit,
                self.branch_name,        // for error messages only
                self.remote_branch_name, // for error messages only
            )?;
            // the are the same
            let new_stack_head = merge_commit.id();
            let new_series_head = merge_commit.id();
            (new_stack_head, new_series_head)
        } else {
            // Get the commits to rebase for the series
            let OrderCommitsResult {
                merge_base,
                ordered_commits,
            } = order_commits_for_rebasing(
                self.repository,
                self.target_branch_head,
                series_head,
                self.remote_head,
            )?;
            // First rebase the series with it's remote commits
            let new_series_head =
                cherry_rebase_group(self.repository, merge_base, &ordered_commits)?;
            // Get the commits that come after the series head, until the stack head
            let remaining_ids_to_rebase =
                self.repository
                    .l(self.branch_head, LogUntil::Commit(series_head), false)?;
            // Rebase the remaining commits on top of the new series head in order to get the new stack head
            (
                cherry_rebase_group(self.repository, new_series_head, &remaining_ids_to_rebase)?,
                new_series_head,
            )
        };
        // Find what the new head and branch tree should be
        Ok((
            compute_updated_branch_head_for_commits(
                self.repository,
                self.branch_head,
                self.branch_tree,
                new_stack_head,
            )?,
            new_series_head,
        ))
    }

    fn inner_integrate_upstream_commits(&self) -> Result<BranchHeadAndTree> {
        // Find the new branch head after integrating the upstream commits
        let new_head = if self.prefers_merge {
            let branch_head_commit = self.repository.find_commit(self.branch_head)?;
            let remote_head_commit = self.repository.find_commit(self.remote_head)?;
            gitbutler_merge_commits(
                self.repository,
                branch_head_commit,
                remote_head_commit,
                self.branch_name,
                self.remote_branch_name,
            )?
            .id()
        } else {
            let OrderCommitsResult {
                merge_base,
                ordered_commits,
            } = order_commits_for_rebasing(
                self.repository,
                self.target_branch_head,
                self.branch_head,
                self.remote_head,
            )?;

            cherry_rebase_group(self.repository, merge_base, &ordered_commits)?
        };

        // Find what the new head and branch tree should be
        compute_updated_branch_head_for_commits(
            self.repository,
            self.branch_head,
            self.branch_tree,
            new_head,
        )
    }
}

struct OrderCommitsResult {
    merge_base: git2::Oid,
    ordered_commits: Vec<git2::Oid>,
}

fn order_commits_for_rebasing(
    repository: &git2::Repository,
    target_branch_head: git2::Oid,
    branch_head: git2::Oid,
    remote_head: git2::Oid,
) -> Result<OrderCommitsResult> {
    let merge_base =
        repository.merge_base_octopussy(&[target_branch_head, branch_head, remote_head])?;

    let target_branch_commits =
        repository.l(target_branch_head, LogUntil::Commit(merge_base), false)?;
    let local_branch_commits = repository.l(branch_head, LogUntil::Commit(merge_base), false)?;

    let remote_local_merge_base = repository.merge_base(branch_head, remote_head)?;
    let remote_commits = repository.l(
        remote_head,
        LogUntil::Commit(remote_local_merge_base),
        false,
    )?;

    let (integrated_commits, filtered_remote_commits) =
        remote_commits.into_iter().partition(|remote_commit| {
            target_branch_commits
                .iter()
                .any(|target_commit| target_commit == remote_commit)
        });

    let commits_to_rebase = [
        filtered_remote_commits,
        local_branch_commits,
        integrated_commits,
    ]
    .concat();

    Ok(OrderCommitsResult {
        merge_base,
        ordered_commits: commits_to_rebase,
    })
}

#[cfg(test)]
mod test {
    use crate::branch_upstream_integration::{
        order_commits_for_rebasing, IntegrateUpstreamContext,
    };
    use gitbutler_testsupport::testing_repository::{
        assert_commit_tree_matches, TestingRepository,
    };

    mod inner_integrate_upstream_commits {
        use gitbutler_commit::commit_ext::CommitExt as _;
        use gitbutler_repo::LogUntil;
        use gitbutler_repo::RepositoryExt as _;
        use gitbutler_workspace::BranchHeadAndTree;

        use super::*;

        /// Local:  Base -> A -> B
        /// Remote: Base -> A -> B -> X -> Y
        /// Trunk:  Base
        /// Result: Base -> A -> B -> X -> Y
        #[test]
        fn other_added_remote_changes() {
            let test_repository = TestingRepository::open();

            let base_commit = test_repository.commit_tree(None, &[("foo.txt", "foo")]);
            let local_a = test_repository.commit_tree(Some(&base_commit), &[("foo.txt", "foo1")]);
            let local_b = test_repository.commit_tree(Some(&local_a), &[("foo.txt", "foo2")]);

            let remote_x = test_repository.commit_tree(Some(&local_b), &[("foo.txt", "foo3")]);
            let remote_y = test_repository.commit_tree(Some(&remote_x), &[("foo.txt", "foo4")]);

            let ctx = IntegrateUpstreamContext {
                repository: &test_repository.repository,
                target_branch_head: base_commit.id(),
                branch_head: local_b.id(),
                branch_tree: local_b.tree_id(),
                branch_name: "test",
                remote_head: remote_y.id(),
                remote_branch_name: "test",
                prefers_merge: false,
            };

            let BranchHeadAndTree { head, tree: _tree } =
                ctx.inner_integrate_upstream_commits().unwrap();

            assert_eq!(
                test_repository
                    .repository
                    .l(head, LogUntil::Commit(base_commit.id()), false)
                    .unwrap(),
                vec![remote_y.id(), remote_x.id(), local_b.id(), local_a.id()],
            );

            // Also ensure the series implementation does the same thing
            let (BranchHeadAndTree { head, tree: _tree }, _) = ctx
                .inner_integrate_upstream_commits_for_series(local_b.id())
                .unwrap();
            assert_eq!(
                test_repository
                    .repository
                    .l(head, LogUntil::Commit(base_commit.id()), false)
                    .unwrap(),
                vec![remote_y.id(), remote_x.id(), local_b.id(), local_a.id()],
            );
        }

        /// Full Stack: Base -> A -> B -> C -> D
        /// Series One:         A -> B
        /// Series Two:                   C -> D
        /// Series One Remote:  A -> B -> X -> Y
        ///
        /// Result Stack: Base -> A -> B -> X -> Y -> C -> D
        /// Result Series One Head: Y
        #[test]
        fn other_added_remote_changes_multiple_series() {
            let test_repository = TestingRepository::open();

            let base_commit = test_repository.commit_tree(None, &[("foo.txt", "foo")]);
            let local_a = test_repository.commit_tree(Some(&base_commit), &[("foo.txt", "foo1")]);
            let local_b = test_repository.commit_tree(Some(&local_a), &[("foo.txt", "foo2")]);
            let local_c = test_repository.commit_tree(Some(&local_b), &[("foo.txt", "fooC")]);
            let local_d = test_repository.commit_tree(Some(&local_c), &[("foo.txt", "fooD")]);

            let remote_x = test_repository.commit_tree(Some(&local_b), &[("foo.txt", "foo3")]);
            let remote_y = test_repository.commit_tree(Some(&remote_x), &[("foo.txt", "foo4")]);

            let ctx = IntegrateUpstreamContext {
                repository: &test_repository.repository,
                target_branch_head: base_commit.id(),
                branch_head: local_d.id(),
                branch_tree: local_d.tree_id(),
                branch_name: "test",
                remote_head: remote_y.id(),
                remote_branch_name: "test",
                prefers_merge: false,
            };

            let (BranchHeadAndTree { head, tree: _tree }, new_series_head) = ctx
                .inner_integrate_upstream_commits_for_series(local_b.id()) // series head is earlier than stack head
                .unwrap();
            assert_eq!(new_series_head, remote_y.id());
            assert_eq!(
                test_repository
                    .repository
                    .l(head, LogUntil::Commit(base_commit.id()), false)
                    .unwrap()
                    .iter()
                    .map(|c| {
                        let commit = test_repository.repository.find_commit(*c).unwrap();
                        commit.message().unwrap().to_string()
                    })
                    .collect::<Vec<_>>(),
                vec![
                    local_d.message().unwrap(),
                    local_c.message().unwrap(),
                    remote_y.message().unwrap(),
                    remote_x.message().unwrap(),
                    local_b.message().unwrap(),
                    local_a.message().unwrap()
                ],
            );
        }

        /// Local:  Base -> A -> B
        /// Remote: Base -> A -> B' -> Y
        /// Trunk:  Base
        /// Result: Base -> A -> B -> B'' -> Y'
        #[test]
        fn modified_local_commit_unconflicting_content() {
            let test_repository = TestingRepository::open();

            let base_commit = test_repository.commit_tree(None, &[]);
            let local_a = test_repository.commit_tree_with_message(
                Some(&base_commit),
                "A",
                &[("foo.txt", "foo")],
            );
            let local_b = test_repository.commit_tree_with_message(
                Some(&local_a),
                "B",
                &[("foo.txt", "foo1")],
            );

            // imagine someone on the remote rebased local_b
            let remote_b = test_repository.commit_tree_with_message(
                Some(&local_a),
                "B'",
                &[("foo.txt", "foo1"), ("bar.txt", "foo2")],
            );
            let remote_y = test_repository.commit_tree_with_message(
                Some(&remote_b),
                "Y",
                &[("foo.txt", "foo3")],
            );

            let ctx = IntegrateUpstreamContext {
                repository: &test_repository.repository,
                target_branch_head: base_commit.id(),
                branch_head: local_b.id(),
                branch_tree: local_b.tree_id(),
                branch_name: "test",
                remote_head: remote_y.id(),
                remote_branch_name: "test",
                prefers_merge: false,
            };

            let BranchHeadAndTree { head, tree: _tree } =
                ctx.inner_integrate_upstream_commits().unwrap();

            let commits = test_repository
                .repository
                .log(head, LogUntil::Commit(base_commit.id()), false)
                .unwrap();

            assert_eq!(commits.len(), 4);

            let new_y = commits[0].clone();
            let new_b_prime = commits[1].clone();
            let new_b = commits[2].clone();
            let new_a = commits[3].clone();

            assert_commit_tree_matches(
                &test_repository.repository,
                &new_y,
                &[("foo.txt", b"foo3")],
            );

            assert_commit_tree_matches(
                &test_repository.repository,
                &new_b_prime,
                &[("foo.txt", b"foo1"), ("bar.txt", b"foo2")],
            );

            assert_commit_tree_matches(
                &test_repository.repository,
                &new_b,
                &[("foo.txt", b"foo1")],
            );

            assert_commit_tree_matches(&test_repository.repository, &new_a, &[("foo.txt", b"foo")]);

            // Ensure the series implementation does the same
            let (BranchHeadAndTree { head, tree: _tree }, _) = ctx
                .inner_integrate_upstream_commits_for_series(local_b.id())
                .unwrap();

            let commits = test_repository
                .repository
                .log(head, LogUntil::Commit(base_commit.id()), false)
                .unwrap();

            assert_eq!(commits.len(), 4);

            let new_y = commits[0].clone();
            let new_b_prime = commits[1].clone();
            let new_b = commits[2].clone();
            let new_a = commits[3].clone();

            assert_commit_tree_matches(
                &test_repository.repository,
                &new_y,
                &[("foo.txt", b"foo3")],
            );

            assert_commit_tree_matches(
                &test_repository.repository,
                &new_b_prime,
                &[("foo.txt", b"foo1"), ("bar.txt", b"foo2")],
            );

            assert_commit_tree_matches(
                &test_repository.repository,
                &new_b,
                &[("foo.txt", b"foo1")],
            );

            assert_commit_tree_matches(&test_repository.repository, &new_a, &[("foo.txt", b"foo")]);
        }

        /// Local:  Base -> A -> B
        /// Remote: Base -> A -> B' (will conflict when rebased on top of B) -> Y
        /// Trunk:  Base
        /// Result: Base -> A -> B -> B'' (Cft) -> Y'
        #[test]
        fn modified_local_commit_conflicting_content() {
            let test_repository = TestingRepository::open();

            let base_commit = test_repository.commit_tree(None, &[]);
            let local_a = test_repository.commit_tree_with_message(
                Some(&base_commit),
                "A",
                &[("foo.txt", "foo")],
            );
            let local_b = test_repository.commit_tree_with_message(
                Some(&local_a),
                "B",
                &[("foo.txt", "foo1")],
            );

            // imagine someone on the remote rebased local_b
            let remote_b = test_repository.commit_tree_with_message(
                Some(&local_a),
                "B'",
                &[("foo.txt", "foo2")],
            );
            let remote_y = test_repository.commit_tree_with_message(
                Some(&remote_b),
                "Y",
                &[("foo.txt", "foo3")],
            );

            let ctx = IntegrateUpstreamContext {
                repository: &test_repository.repository,
                target_branch_head: base_commit.id(),
                branch_head: local_b.id(),
                branch_tree: local_b.tree_id(),
                branch_name: "test",
                remote_head: remote_y.id(),
                remote_branch_name: "test",
                prefers_merge: false,
            };

            let BranchHeadAndTree { head, tree: _tree } =
                ctx.inner_integrate_upstream_commits().unwrap();

            let commits = test_repository
                .repository
                .log(head, LogUntil::Commit(base_commit.id()), false)
                .unwrap();

            assert_eq!(commits.len(), 4);

            let new_y = commits[0].clone();
            let new_b_prime = commits[1].clone();
            let new_b = commits[2].clone();
            let new_a = commits[3].clone();

            assert!(new_y.is_conflicted());
            assert_commit_tree_matches(
                &test_repository.repository,
                &new_y,
                &[
                    (".auto-resolution/foo.txt", b"foo1"),
                    (".conflict-base-0/foo.txt", b"foo2"),
                    (".conflict-side-0/foo.txt", b"foo1"),
                    (".conflict-side-1/foo.txt", b"foo3"),
                ],
            );

            assert!(new_b_prime.is_conflicted());
            assert_commit_tree_matches(
                &test_repository.repository,
                &new_b_prime,
                &[
                    (".auto-resolution/foo.txt", b"foo1"),
                    (".conflict-base-0/foo.txt", b"foo"),
                    (".conflict-side-0/foo.txt", b"foo1"),
                    (".conflict-side-1/foo.txt", b"foo2"),
                ],
            );

            assert_commit_tree_matches(
                &test_repository.repository,
                &new_b,
                &[("foo.txt", b"foo1")],
            );

            assert_commit_tree_matches(&test_repository.repository, &new_a, &[("foo.txt", b"foo")]);

            // Also ensure the series implementation does the same thing
            let (BranchHeadAndTree { head, tree: _tree }, _) = ctx
                .inner_integrate_upstream_commits_for_series(local_b.id())
                .unwrap();

            let commits = test_repository
                .repository
                .log(head, LogUntil::Commit(base_commit.id()), false)
                .unwrap();

            assert_eq!(commits.len(), 4);

            let new_y = commits[0].clone();
            let new_b_prime = commits[1].clone();
            let new_b = commits[2].clone();
            let new_a = commits[3].clone();

            assert!(new_y.is_conflicted());
            assert_commit_tree_matches(
                &test_repository.repository,
                &new_y,
                &[
                    (".auto-resolution/foo.txt", b"foo1"),
                    (".conflict-base-0/foo.txt", b"foo2"),
                    (".conflict-side-0/foo.txt", b"foo1"),
                    (".conflict-side-1/foo.txt", b"foo3"),
                ],
            );

            assert!(new_b_prime.is_conflicted());
            assert_commit_tree_matches(
                &test_repository.repository,
                &new_b_prime,
                &[
                    (".auto-resolution/foo.txt", b"foo1"),
                    (".conflict-base-0/foo.txt", b"foo"),
                    (".conflict-side-0/foo.txt", b"foo1"),
                    (".conflict-side-1/foo.txt", b"foo2"),
                ],
            );

            assert_commit_tree_matches(
                &test_repository.repository,
                &new_b,
                &[("foo.txt", b"foo1")],
            );

            assert_commit_tree_matches(&test_repository.repository, &new_a, &[("foo.txt", b"foo")]);
        }

        /// Local:  Base -> A -> B
        /// Remote: Base -> A -> B' (no diff changes) -> Y
        /// Trunk:  Base
        /// Result: Base -> A -> B -> Y'
        /// The empty B' commit should be dropped
        #[test]
        fn modified_local_commit_unconflicting_no_op() {
            let test_repository = TestingRepository::open();

            let base_commit = test_repository.commit_tree(None, &[]);
            let local_a = test_repository.commit_tree_with_message(
                Some(&base_commit),
                "A",
                &[("foo.txt", "foo")],
            );
            let local_b = test_repository.commit_tree_with_message(
                Some(&local_a),
                "B",
                &[("foo.txt", "foo1")],
            );

            // imagine someone on the remote rebased local_b
            let remote_b = test_repository.commit_tree_with_message(
                Some(&local_a),
                "B'",
                &[("foo.txt", "foo1")],
            );
            let remote_y = test_repository.commit_tree_with_message(
                Some(&remote_b),
                "Y",
                &[("foo.txt", "foo3")],
            );

            let ctx = IntegrateUpstreamContext {
                repository: &test_repository.repository,
                target_branch_head: base_commit.id(),
                branch_head: local_b.id(),
                branch_tree: local_b.tree_id(),
                branch_name: "test",
                remote_head: remote_y.id(),
                remote_branch_name: "test",
                prefers_merge: false,
            };

            let BranchHeadAndTree { head, tree: _tree } =
                ctx.inner_integrate_upstream_commits().unwrap();

            let commits = test_repository
                .repository
                .log(head, LogUntil::Commit(base_commit.id()), false)
                .unwrap();

            assert_eq!(commits.len(), 3);

            let new_y = commits[0].clone();
            let new_b = commits[1].clone();
            let new_a = commits[2].clone();

            assert_commit_tree_matches(
                &test_repository.repository,
                &new_y,
                &[("foo.txt", b"foo3")],
            );

            assert_commit_tree_matches(
                &test_repository.repository,
                &new_b,
                &[("foo.txt", b"foo1")],
            );

            assert_commit_tree_matches(&test_repository.repository, &new_a, &[("foo.txt", b"foo")]);

            // Also ensure the series implementation does the same thing
            let (BranchHeadAndTree { head, tree: _tree }, _) = ctx
                .inner_integrate_upstream_commits_for_series(local_b.id())
                .unwrap();

            let commits = test_repository
                .repository
                .log(head, LogUntil::Commit(base_commit.id()), false)
                .unwrap();

            assert_eq!(commits.len(), 3);

            let new_y = commits[0].clone();
            let new_b = commits[1].clone();
            let new_a = commits[2].clone();

            assert_commit_tree_matches(
                &test_repository.repository,
                &new_y,
                &[("foo.txt", b"foo3")],
            );

            assert_commit_tree_matches(
                &test_repository.repository,
                &new_b,
                &[("foo.txt", b"foo1")],
            );

            assert_commit_tree_matches(&test_repository.repository, &new_a, &[("foo.txt", b"foo")]);
        }
    }

    mod order_commits_for_rebasing {
        use super::*;

        /// Local:  Base -> A -> B
        /// Remote: Base -> A -> B -> X -> Y
        /// Trunk:  Base
        /// Result: Base -> A -> B -> X -> Y
        #[test]
        fn other_added_remote_changes() {
            let test_repository = TestingRepository::open();

            let base_commit = test_repository.commit_tree(None, &[]);
            let local_a = test_repository.commit_tree(Some(&base_commit), &[]);
            let local_b = test_repository.commit_tree(Some(&local_a), &[]);

            let remote_x = test_repository.commit_tree(Some(&local_b), &[]);
            let remote_y = test_repository.commit_tree(Some(&remote_x), &[]);

            let commits = order_commits_for_rebasing(
                &test_repository.repository,
                base_commit.id(),
                local_b.id(),
                remote_y.id(),
            )
            .unwrap();

            assert_eq!(
                commits.ordered_commits,
                vec![remote_y.id(), remote_x.id(), local_b.id(), local_a.id()],
            );
        }

        /// Local:  Base -> A -> B
        /// Remote: Base -> A -> B' -> Y
        /// Trunk:  Base
        /// Result: Base -> A -> B -> B' -> Y
        #[test]
        fn modified_local_commit() {
            let test_repository = TestingRepository::open();

            let base_commit = test_repository.commit_tree(None, &[]);
            let local_a = test_repository.commit_tree(Some(&base_commit), &[]);
            let local_b = test_repository.commit_tree(Some(&local_a), &[]);

            // imagine someone on the remote rebased local_b
            let remote_b = test_repository.commit_tree(Some(&local_a), &[]);
            let remote_y = test_repository.commit_tree(Some(&remote_b), &[]);

            let commits = order_commits_for_rebasing(
                &test_repository.repository,
                base_commit.id(),
                local_b.id(),
                remote_y.id(),
            )
            .unwrap();

            assert_eq!(
                commits.ordered_commits,
                vec![remote_y.id(), remote_b.id(), local_b.id(), local_a.id()],
            );
        }

        /// Local:  Base -> A -> B
        /// Remote: Base -> M -> N -> A -> B' -> Y
        /// Trunk:  Base -> M -> N
        /// Result: Base -> M -> N -> A -> B -> A' -> B' -> Y
        #[test]
        fn remote_includes_integrated_commits() {
            let test_repository = TestingRepository::open();

            // Setup:
            // (z)
            //  |
            //  y
            //  |
            //  x
            //  |
            // (n) (b)
            //  |   |
            //  m   a
            //  \   /
            //   base_commit
            let base_commit = test_repository.commit_tree(None, &[]);
            let trunk_m = test_repository.commit_tree(Some(&base_commit), &[]);
            let trunk_n = test_repository.commit_tree(Some(&trunk_m), &[]);

            let local_a = test_repository.commit_tree(Some(&base_commit), &[]);
            let local_b = test_repository.commit_tree(Some(&local_a), &[]);

            // imagine someone on the remote rebased local_a
            let remote_x = test_repository.commit_tree(Some(&trunk_n), &[]);
            let remote_y = test_repository.commit_tree(Some(&remote_x), &[]);
            let remote_z = test_repository.commit_tree(Some(&remote_y), &[]);

            let commits = order_commits_for_rebasing(
                &test_repository.repository,
                trunk_n.id(),
                local_b.id(),
                remote_z.id(),
            )
            .unwrap();

            assert_eq!(
                commits.ordered_commits,
                vec![
                    remote_z.id(),
                    remote_y.id(),
                    remote_x.id(),
                    local_b.id(),
                    local_a.id(),
                    trunk_n.id(),
                    trunk_m.id()
                ],
            );
        }
    }
}
