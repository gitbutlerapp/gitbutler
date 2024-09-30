use anyhow::{bail, Context as _, Result};
use gitbutler_branch::{signature, BranchId, SignaturePurpose};
use gitbutler_cherry_pick::RepositoryExt as _;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt as _;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_repo::{rebase::cherry_rebase_group, LogUntil, RepositoryExt as _};

use crate::{branch_trees::checkout_branch_trees, VirtualBranchesExt as _};

/// Moves a commit up or down a stack by a certain offset.
///
/// After the commit is moved, the combined branch trees are checked out.
/// A stack must have at least two commits in it. A 0 offset is a no-op.
///
/// Presume we had the stack:
///
/// A
/// B
/// C
/// D
///
/// If C was the subject, and the offset was -1, we would expect:
///
/// A
/// C
/// B
/// D
///
/// Or, if B was the subject, and the offset was 1, we would expect:
///
/// A
/// C
/// B
/// D
pub(crate) fn reorder_commit(
    ctx: &CommandContext,
    branch_id: BranchId,
    subject_commit_oid: git2::Oid,
    offset: i32,
    perm: &mut WorktreeWritePermission,
) -> Result<()> {
    let repository = ctx.repository();
    let vb_state = ctx.project().virtual_branches();
    let default_target = vb_state.get_default_target()?;
    let default_target_commit = repository
        .find_reference(&default_target.branch.to_string())?
        .peel_to_commit()?;
    let merge_base = repository.merge_base(default_target_commit.id(), subject_commit_oid)?;

    let mut branch = vb_state.get_branch_in_workspace(branch_id)?;

    let ReorderResult { tree, head } = inner_reorder_commit(
        repository,
        merge_base,
        subject_commit_oid,
        offset,
        &repository.l(branch.head, LogUntil::Commit(merge_base))?,
        &repository.find_tree(branch.tree)?,
        ctx.project().succeeding_rebases,
    )?;

    branch.tree = tree;
    branch.head = head;

    branch.updated_timestamp_ms = gitbutler_time::time::now_ms();
    vb_state.set_branch(branch.clone())?;

    checkout_branch_trees(ctx, perm)?;

    crate::integration::update_workspace_commit(&vb_state, ctx)
        .context("failed to update gitbutler workspace")?;

    Ok(())
}

struct ReorderResult {
    tree: git2::Oid,
    head: git2::Oid,
}

/// Commit reordering implemented with libgit2 primatives
///
/// This returns a new head commit and tree for the branch.
///
/// If the tree ends up in a conflicted state when rebasing the head commit
/// will actually be the commited tree in a conflicted state, and the tree
/// will be the auto-resolved tree.
///
/// After usage (which should only be in `reorder_commit`), its important to
/// re-merge all the branch trees together and check that out, as otherwise
/// the tree writes will be lost.
///
/// * `base_commit`: The merge base of the branch head and trunk
/// * `subject_commit`: The commit to be moved
/// * `offset`: The offset to move the subject commit by, where
///     negative is up (towards the branch head) and positive is down
/// * `branch_commits`: The commits all the commits in the branch.
/// * `branch_tree`: The tree of the branch
fn inner_reorder_commit(
    repository: &git2::Repository,
    base_commit: git2::Oid,
    subject_commit: git2::Oid,
    offset: i32,
    branch_commits: &[git2::Oid],
    branch_tree: &git2::Tree,
    succeeding_rebases: bool,
) -> Result<ReorderResult> {
    if branch_commits.len() < 2 {
        bail!("Cannot re-order less than two commits");
    };

    if offset == 0 {
        return Ok(ReorderResult {
            tree: branch_tree.id(),
            head: branch_commits[0],
        });
    };

    ensure_offset_within_bounds(subject_commit, offset, branch_commits)?;

    let author = signature(SignaturePurpose::Author)?;
    let committer = signature(SignaturePurpose::Committer)?;
    let tree_commit = repository.commit(
        None,
        &author,
        &committer,
        "Uncommited changes",
        branch_tree,
        &[&repository.find_commit(branch_commits[0])?],
    )?;

    let branch_commits = reorder_commit_list(subject_commit, offset, branch_commits)?;

    // Rebase branch commits
    // We are passing all the commits to the cherry_rebase_group funcion, but
    // this is not a concern as it will verbaitm copy any commits that have
    // not had their parents changed.
    let new_head_oid =
        cherry_rebase_group(repository, base_commit, &branch_commits, succeeding_rebases)?;

    // Rebase branch tree on top of new stack
    let new_tree_commit =
        cherry_rebase_group(repository, new_head_oid, &[tree_commit], succeeding_rebases)?;
    let new_tree_commit = repository.find_commit(new_tree_commit)?;

    if new_tree_commit.is_conflicted() {
        Ok(ReorderResult {
            tree: repository
                .find_real_tree(&new_tree_commit, Default::default())?
                .id(),
            head: new_tree_commit.id(),
        })
    } else {
        Ok(ReorderResult {
            tree: new_tree_commit.tree_id(),
            head: new_head_oid,
        })
    }
}

fn reorder_commit_list(
    subject_commit: git2::Oid,
    offset: i32,
    branch_commits: &[git2::Oid],
) -> Result<Vec<git2::Oid>> {
    let subject_index = branch_commits
        .iter()
        .position(|c| *c == subject_commit)
        .ok_or(anyhow::anyhow!(
            "Subject commit not found in branch commits"
        ))?;

    let mut output = branch_commits.to_owned();

    output.remove(subject_index);
    output.insert(((subject_index as i32) + offset) as usize, subject_commit);

    Ok(output)
}

/// Presume we had the stack:
///
/// A
/// B
/// C // idx 2 // 2 + -1 = 1; 1 < 0 => All good
/// D
///
/// If C was the subject, and the offset was -1, we would expect:
///
/// A
/// C
/// B
/// D
///
/// Presume we had the stack:
///
/// A
/// B // idx 1 // 1 + 1 = 2; 2 >= 4 => All good
/// C
/// D
///
/// If B was the subject, and the offset was 1, we would expect:
///
/// A
/// C
/// B
/// D
fn ensure_offset_within_bounds(
    subject_commit: git2::Oid,
    offset: i32,
    branch_commits: &[git2::Oid],
) -> Result<()> {
    let subject_index = branch_commits
        .iter()
        .position(|c| *c == subject_commit)
        .ok_or(anyhow::anyhow!(
            "Subject commit not found in branch commits"
        ))?;

    if subject_index as i32 + offset < 0 {
        bail!("Offset is out of bounds");
    }

    if subject_index as i32 + offset >= branch_commits.len() as i32 {
        bail!("Offset is out of bounds");
    }

    Ok(())
}

#[cfg(test)]
mod test {
    #[cfg(test)]
    mod inner_reorder_commit {
        use gitbutler_cherry_pick::RepositoryExt as _;
        use gitbutler_commit::commit_ext::CommitExt as _;
        use gitbutler_repo::LogUntil;
        use gitbutler_repo::RepositoryExt as _;
        use gitbutler_testsupport::testing_repository::{
            assert_commit_tree_matches, TestingRepository,
        };

        #[test]
        fn less_than_two_commits_is_an_error() {
            let test_repository = TestingRepository::open();

            let merge_base = test_repository.commit_tree(None, &[("foo.txt", "a")]);
            let b = test_repository.commit_tree(Some(&merge_base), &[("foo.txt", "b")]);

            let result = crate::reorder_commits::inner_reorder_commit(
                &test_repository.repository,
                merge_base.id(),
                b.id(),
                0,
                &[b.id()],
                &b.tree().unwrap(),
                true,
            );

            assert!(result.is_err());
        }

        #[test]
        fn unconflicting_reorder() {
            let test_repository = TestingRepository::open();

            let merge_base = test_repository.commit_tree(None, &[("foo.txt", "a")]);
            // Commit A adds file bar
            let a = test_repository
                .commit_tree(Some(&merge_base), &[("foo.txt", "a"), ("bar.txt", "a")]);
            // Commit B adds file baz
            let b = test_repository.commit_tree(
                Some(&a),
                &[("foo.txt", "a"), ("bar.txt", "a"), ("baz.txt", "a")],
            );

            let result = crate::reorder_commits::inner_reorder_commit(
                &test_repository.repository,
                merge_base.id(),
                a.id(),
                -1,
                &[b.id(), a.id()],
                &b.tree().unwrap(),
                true,
            )
            .unwrap();

            let a_prime: git2::Commit =
                test_repository.repository.find_commit(result.head).unwrap();
            assert!(!a_prime.is_conflicted());

            assert_commit_tree_matches(
                &test_repository.repository,
                &a_prime,
                &[("foo.txt", b"a"), ("bar.txt", b"a"), ("baz.txt", b"a")],
            );

            let b_prime: git2::Commit = a_prime.parent(0).unwrap();
            assert!(!b_prime.is_conflicted());

            assert_commit_tree_matches(
                &test_repository.repository,
                &b_prime,
                &[("foo.txt", b"a"), ("baz.txt", b"a")],
            );
            assert!(b_prime.tree().unwrap().get_name("bar.txt").is_none());

            // In this case, the tree should be the same as a_prime, as there
            // were no uncommited files
            assert_eq!(result.tree, a_prime.tree_id());
        }

        #[test]
        fn conflicting_reorder() {
            let test_repository = TestingRepository::open();

            let merge_base = test_repository.commit_tree(None, &[("foo.txt", "a")]);
            let a = test_repository.commit_tree(Some(&merge_base), &[("foo.txt", "x")]);
            let b = test_repository.commit_tree(Some(&a), &[("foo.txt", "y")]);

            // When we re-order a and b, they will conflict.
            //
            // Setup:  : Step 1:  : Step 2:  :
            // B: y    :          : A': x    :
            // |       :          :          :
            // A: x    : B': a    : B': a    : <- B' is the auto-resolved tree
            // |       :          :          :
            // MB: a   : MB: a    : MB: a    :
            //
            // Reorder step 1:
            // Cherry pick B on top of Merge Base:
            // Merge trees: Bt and MBt, with base At. The theirs and ours sides
            // both have changes compared to the base so it conflicts.
            // We auto resolve to have the content of MBt: "a"
            //
            // Reorder step 2:
            // Cherry pick A on top of B':
            // Merge trees: At and B't, with base MBt. MBt is unchange relative
            // to the base, so it merges cleanly to "x".

            let result = crate::reorder_commits::inner_reorder_commit(
                &test_repository.repository,
                merge_base.id(),
                a.id(),
                -1,
                &[b.id(), a.id()],
                &b.tree().unwrap(),
                true,
            )
            .unwrap();

            let a_prime: git2::Commit =
                test_repository.repository.find_commit(result.head).unwrap();
            assert!(!a_prime.is_conflicted());

            assert_commit_tree_matches(&test_repository.repository, &a_prime, &[("foo.txt", b"x")]);

            let b_prime: git2::Commit = a_prime.parent(0).unwrap();
            assert!(b_prime.is_conflicted());

            assert_commit_tree_matches(
                &test_repository.repository,
                &b_prime,
                &[
                    (".auto-resolution/foo.txt", b"a"),
                    (".conflict-base-0/foo.txt", b"x"),
                    (".conflict-side-0/foo.txt", b"a"),
                    (".conflict-side-1/foo.txt", b"y"),
                ],
            );

            // In this case, the tree should be the same as a_prime, as there
            // were no uncommited files
            assert_eq!(result.tree, a_prime.tree_id());

            // We should now be able to re-order the commits back back to their
            // original order and the tree should be the same as the original.

            // When we re-order A' and B', they become unconflicted
            //
            // Setup:  : Step 1:  : Step 2:  :
            // A': y   :          : B'': x   :
            // |       :          :          :
            // B': a   : A'': y   : A'': y   : <- B' is the auto-resolved tree
            // |       :          :          :
            // MB: a   : MB: a    : MB: a    :
            //
            // Reorder step 1:
            // Cherry pick A'' on top of Merge Base:
            // Merge trees: A't and MBt, with base B't (auto-resolution. MBt is
            // unchanged relative to the base, so it merges cleanly to "x".
            //
            // Reorder step 2:
            // Cherry pick B' on top of A'':
            // Merge trees: A''t and B't (ours == Bt), with base At.
            // A''t is y, B't is a, and At is y. This is a clean merge.

            let result = crate::reorder_commits::inner_reorder_commit(
                &test_repository.repository,
                merge_base.id(),
                b_prime.id(),
                -1,
                &[a_prime.id(), b_prime.id()],
                &a_prime.tree().unwrap(),
                true,
            )
            .unwrap();

            let b_double_prime: git2::Commit =
                test_repository.repository.find_commit(result.head).unwrap();

            assert!(!b_double_prime.is_conflicted());
            assert_eq!(b_double_prime.tree_id(), b.tree_id());

            let a_double_prime: git2::Commit = b_double_prime.parent(0).unwrap();

            assert!(!a_double_prime.is_conflicted());
            assert_eq!(a_double_prime.tree_id(), a.tree_id());
        }

        #[test]
        fn conflicting_tree() {
            let test_repository = TestingRepository::open();

            let merge_base = test_repository.commit_tree(None, &[("foo.txt", "a")]);
            let a = test_repository.commit_tree(Some(&merge_base), &[("foo.txt", "x")]);
            let b = test_repository.commit_tree(Some(&a), &[("foo.txt", "y")]);
            let tree = test_repository.commit_tree(Some(&b), &[("foo.txt", "z")]);

            let result = crate::reorder_commits::inner_reorder_commit(
                &test_repository.repository,
                merge_base.id(),
                a.id(),
                -1,
                &[b.id(), a.id()],
                &tree.tree().unwrap(),
                true,
            )
            .unwrap();

            let commits: Vec<git2::Commit> = test_repository
                .repository
                .log(result.head, LogUntil::Commit(merge_base.id()))
                .unwrap();

            assert_eq!(
                commits.len(),
                3,
                "The conflicted tree should be come a commit"
            );

            let tree_commit = &commits[0];
            let a_prime = &commits[1];
            let b_prime = &commits[2];

            assert!(tree_commit.is_conflicted());
            assert!(!a_prime.is_conflicted());
            assert!(b_prime.is_conflicted());

            assert_eq!(
                test_repository
                    .repository
                    .find_real_tree(tree_commit, Default::default())
                    .unwrap()
                    .id(),
                result.tree,
                "The tree should be the auto-resolved tree of the tree commit"
            );

            // Order the commits back to their initial order and all should be
            // resolved
            let result = crate::reorder_commits::inner_reorder_commit(
                &test_repository.repository,
                merge_base.id(),
                b_prime.id(),
                -1,
                &[tree_commit.id(), a_prime.id(), b_prime.id()],
                &tree.tree().unwrap(),
                true,
            )
            .unwrap();

            let commits: Vec<git2::Commit> = test_repository
                .repository
                .log(result.head, LogUntil::Commit(merge_base.id()))
                .unwrap();

            assert_eq!(commits.len(), 3);

            let tree_commit = &commits[0];
            let b_double_prime = &commits[1];
            let a_double_prime = &commits[2];

            assert!(!tree_commit.is_conflicted());
            assert!(!b_double_prime.is_conflicted());
            assert!(!a_double_prime.is_conflicted());

            assert_eq!(tree_commit.tree_id(), result.tree);
        }
    }

    #[cfg(test)]
    mod reorder_commit_list {
        use gitbutler_testsupport::testing_repository::TestingRepository;

        use crate::reorder_commits::reorder_commit_list;

        #[test]
        fn move_commit_up() {
            let test_repository = TestingRepository::open();

            let a = test_repository.commit_tree(None, &[("foo.txt", "a")]).id();
            let b = test_repository.commit_tree(None, &[("foo.txt", "b")]).id();
            let c = test_repository.commit_tree(None, &[("foo.txt", "c")]).id();
            let d = test_repository.commit_tree(None, &[("foo.txt", "d")]).id();

            let branch_commits = reorder_commit_list(c, -1, &[a, b, c, d]).unwrap();

            assert_eq!(branch_commits, &[a, c, b, d]);
        }

        #[test]
        fn move_commit_up_to_top() {
            let test_repository = TestingRepository::open();

            let a = test_repository.commit_tree(None, &[("foo.txt", "a")]).id();
            let b = test_repository.commit_tree(None, &[("foo.txt", "b")]).id();
            let c = test_repository.commit_tree(None, &[("foo.txt", "c")]).id();
            let d = test_repository.commit_tree(None, &[("foo.txt", "d")]).id();

            let branch_commits = reorder_commit_list(c, -2, &[a, b, c, d]).unwrap();

            assert_eq!(branch_commits, &[c, a, b, d]);
        }

        #[test]
        fn move_nowhere() {
            let test_repository = TestingRepository::open();

            let a = test_repository.commit_tree(None, &[("foo.txt", "a")]).id();
            let b = test_repository.commit_tree(None, &[("foo.txt", "b")]).id();
            let c = test_repository.commit_tree(None, &[("foo.txt", "c")]).id();
            let d = test_repository.commit_tree(None, &[("foo.txt", "d")]).id();

            let branch_commits = reorder_commit_list(b, 0, &[a, b, c, d]).unwrap();

            assert_eq!(branch_commits, &[a, b, c, d]);
        }

        #[test]
        fn move_commit_down() {
            let test_repository = TestingRepository::open();

            let a = test_repository.commit_tree(None, &[("foo.txt", "a")]).id();
            let b = test_repository.commit_tree(None, &[("foo.txt", "b")]).id();
            let c = test_repository.commit_tree(None, &[("foo.txt", "c")]).id();
            let d = test_repository.commit_tree(None, &[("foo.txt", "d")]).id();

            let branch_commits = reorder_commit_list(b, 1, &[a, b, c, d]).unwrap();

            assert_eq!(branch_commits, &[a, c, b, d]);
        }

        #[test]
        fn move_commit_down_two() {
            let test_repository = TestingRepository::open();

            let a = test_repository.commit_tree(None, &[("foo.txt", "a")]).id();
            let b = test_repository.commit_tree(None, &[("foo.txt", "b")]).id();
            let c = test_repository.commit_tree(None, &[("foo.txt", "c")]).id();
            let d = test_repository.commit_tree(None, &[("foo.txt", "d")]).id();

            let branch_commits = reorder_commit_list(b, 2, &[a, b, c, d]).unwrap();

            assert_eq!(branch_commits, &[a, c, d, b]);
        }
    }

    #[cfg(test)]
    mod ensure_offset_within_bounds {
        use crate::reorder_commits::ensure_offset_within_bounds;
        use gitbutler_testsupport::testing_repository::TestingRepository;

        #[test]
        fn test() {
            let test_repository = TestingRepository::open();

            let a = test_repository.commit_tree(None, &[("foo.txt", "a")]).id();
            let b = test_repository.commit_tree(None, &[("foo.txt", "b")]).id();
            let c = test_repository.commit_tree(None, &[("foo.txt", "c")]).id();
            let d = test_repository.commit_tree(None, &[("foo.txt", "d")]).id();

            assert!(ensure_offset_within_bounds(b, -2, &[a, b, c, d]).is_err());
            assert!(ensure_offset_within_bounds(b, -1, &[a, b, c, d]).is_ok());
            assert!(ensure_offset_within_bounds(b, 0, &[a, b, c, d]).is_ok());
            assert!(ensure_offset_within_bounds(b, 1, &[a, b, c, d]).is_ok());
            assert!(ensure_offset_within_bounds(b, 2, &[a, b, c, d]).is_ok());
            assert!(ensure_offset_within_bounds(b, 3, &[a, b, c, d]).is_err());
        }
    }
}
