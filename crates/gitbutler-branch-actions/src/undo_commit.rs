use anyhow::{bail, Context as _, Result};
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt as _;
use gitbutler_repo::{rebase::cherry_rebase_group, LogUntil, RepositoryExt as _};
use gitbutler_stack::{Stack, StackId};
use gitbutler_stack_api::StackExt;

use crate::VirtualBranchesExt as _;

/// Removes a commit from a branch by rebasing all commits _except_ for it
/// onto it's parent.
///
/// if successful, it will update the branch head to the new head commit.
///
/// It intentionally does **not** update the branch tree. It is a feature
/// of the operation that the branch tree will not be updated as it allows
/// the user to then re-commit the changes if they wish.
///
/// This may create conflicted commits above the commit that is getting
/// undone.
pub(crate) fn undo_commit(
    ctx: &CommandContext,
    branch_id: StackId,
    commit_oid: git2::Oid,
) -> Result<Stack> {
    let vb_state = ctx.project().virtual_branches();

    let mut branch = vb_state.get_branch_in_workspace(branch_id)?;

    let new_head_commit = inner_undo_commit(ctx.repository(), branch.head(), commit_oid)?;

    branch.set_stack_head(ctx, new_head_commit, None)?;

    crate::integration::update_workspace_commit(&vb_state, ctx)
        .context("failed to update gitbutler workspace")?;

    Ok(branch)
}

fn inner_undo_commit(
    repository: &git2::Repository,
    branch_head_commit: git2::Oid,
    commit_to_remove: git2::Oid,
) -> Result<git2::Oid> {
    let commit_to_remove = repository.find_commit(commit_to_remove)?;

    if commit_to_remove.is_conflicted() {
        bail!("Can not undo a conflicted commit");
    }

    // if commit is the head, just set head to the parent
    if branch_head_commit == commit_to_remove.id() {
        return Ok(commit_to_remove.parent(0)?.id());
    };

    let commits_to_rebase =
        repository.l(branch_head_commit, LogUntil::Commit(commit_to_remove.id()))?;

    let new_head = cherry_rebase_group(
        repository,
        commit_to_remove.parent_id(0)?,
        &commits_to_rebase,
    )?;

    Ok(new_head)
}

#[cfg(test)]
mod test {
    #[cfg(test)]
    mod inner_undo_commit {
        use gitbutler_commit::commit_ext::CommitExt as _;
        use gitbutler_repo::rebase::gitbutler_merge_commits;
        use gitbutler_testsupport::testing_repository::{
            assert_commit_tree_matches, TestingRepository,
        };

        use crate::undo_commit::inner_undo_commit;

        #[test]
        fn undoing_conflicted_commit_errors() {
            let test_repository = TestingRepository::open();

            let a = test_repository.commit_tree(None, &[("foo.txt", "foo")]);
            let b = test_repository.commit_tree(Some(&a), &[("bar.txt", "bar")]);
            let c = test_repository.commit_tree(Some(&a), &[("bar.txt", "baz")]);

            let conflicted_commit =
                gitbutler_merge_commits(&test_repository.repository, b, c, "", "").unwrap();

            // Branch looks like "A -> ConflictedCommit"

            let result = inner_undo_commit(
                &test_repository.repository,
                conflicted_commit.id(),
                conflicted_commit.id(),
            );

            assert!(
                result.is_err(),
                "Should error when trying to undo a conflicted commit"
            );
        }

        #[test]
        fn undoing_head_commit() {
            let test_repository = TestingRepository::open();

            let a = test_repository.commit_tree(None, &[("foo.txt", "foo")]);
            let b = test_repository.commit_tree(Some(&a), &[("bar.txt", "bar")]);
            let c = test_repository.commit_tree(Some(&b), &[("baz.txt", "baz")]);

            let new_head = inner_undo_commit(&test_repository.repository, c.id(), c.id()).unwrap();

            assert_eq!(new_head, b.id(), "The new head should be C's parent");
        }

        #[test]
        fn undoing_commits_may_create_conflicts() {
            let test_repository = TestingRepository::open();

            let a = test_repository.commit_tree(None, &[("foo.txt", "foo")]);
            let b = test_repository.commit_tree(Some(&a), &[("foo.txt", "bar")]);
            let c = test_repository.commit_tree(Some(&b), &[("foo.txt", "baz")]);

            // By dropping the "B" commit, we're effectively cherry-picking
            // C onto A, which in tern is merge trees of:
            // Base: B (content bar)
            // Ours: A (content foo)
            // Theirs: C (content baz)
            //
            // As the theirs and ours both are different to the base, it ends up
            // conflicted.
            let new_head = inner_undo_commit(&test_repository.repository, c.id(), b.id()).unwrap();

            let new_head_commit: git2::Commit =
                test_repository.repository.find_commit(new_head).unwrap();

            assert!(new_head_commit.is_conflicted(), "Should be conflicted");

            assert_commit_tree_matches(
                &test_repository.repository,
                &new_head_commit,
                &[
                    (".auto-resolution/foo.txt", b"foo"),
                    (".conflict-base-0/foo.txt", b"bar"), // B is the base
                    (".conflict-side-0/foo.txt", b"foo"), // "Ours" is A
                    (".conflict-side-1/foo.txt", b"baz"), // "Theirs" is C
                ],
            );

            assert_eq!(
                new_head_commit.parent_id(0).unwrap(),
                a.id(),
                "A should be C prime's parent"
            );
        }
    }
}
