use std::collections::HashMap;

use anyhow::{bail, Context as _, Result};
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt as _;
use gitbutler_diff::Hunk;
use gitbutler_repo::{
    logging::{LogUntil, RepositoryExt as _},
    rebase::cherry_rebase_group,
};
use gitbutler_stack::{OwnershipClaim, Stack, StackId};

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
    stack_id: StackId,
    commit_oid: git2::Oid,
) -> Result<Stack> {
    let vb_state = ctx.project().virtual_branches();

    let mut stack = vb_state.get_stack_in_workspace(stack_id)?;

    let UndoResult {
        new_head: new_head_commit,
        ownership_update,
    } = inner_undo_commit(ctx.repo(), stack.head(), commit_oid)?;

    for ownership in ownership_update {
        stack.ownership.put(ownership);
    }

    stack.set_stack_head(ctx, new_head_commit, None)?;

    let removed_commit = ctx.repo().find_commit(commit_oid)?;
    stack.replace_head(ctx, &removed_commit, &removed_commit.parent(0)?)?;

    crate::integration::update_workspace_commit(&vb_state, ctx)
        .context("failed to update gitbutler workspace")?;

    Ok(stack)
}

struct UndoResult {
    new_head: git2::Oid,
    ownership_update: Vec<OwnershipClaim>,
}

fn inner_undo_commit(
    repository: &git2::Repository,
    branch_head_commit: git2::Oid,
    commit_to_remove: git2::Oid,
) -> Result<UndoResult> {
    let commit_to_remove = repository.find_commit(commit_to_remove)?;

    if commit_to_remove.is_conflicted() {
        bail!("Can not undo a conflicted commit");
    }
    let commit_tree = commit_to_remove
        .tree()
        .context("failed to get commit tree")?;
    let commit_to_remove_parent = commit_to_remove.parent(0)?;
    let commit_parent_tree = commit_to_remove_parent
        .tree()
        .context("failed to get parent tree")?;

    let diff = gitbutler_diff::trees(repository, &commit_parent_tree, &commit_tree, true)?;
    let diff: HashMap<_, _> = gitbutler_diff::diff_files_into_hunks(diff).collect();
    let ownership_update = diff
        .iter()
        .filter_map(|(file_path, hunks)| {
            let file_path = file_path.clone();
            let hunks = hunks
                .iter()
                .map(Into::into)
                .filter(|hunk: &Hunk| hunk.start != 0 && hunk.end != 0)
                .collect::<Vec<_>>();
            if hunks.is_empty() {
                return None;
            }
            Some((file_path, hunks))
        })
        .map(|(file_path, hunks)| OwnershipClaim { file_path, hunks })
        .collect::<Vec<_>>();

    // if commit is the head, just set head to the parent
    if branch_head_commit == commit_to_remove.id() {
        return Ok(UndoResult {
            new_head: commit_to_remove_parent.id(),
            ownership_update,
        });
    };

    let commits_to_rebase = repository.l(
        branch_head_commit,
        LogUntil::Commit(commit_to_remove.id()),
        false,
    )?;

    let new_head = cherry_rebase_group(
        repository,
        commit_to_remove.parent_id(0)?,
        &commits_to_rebase,
        false,
    )?;

    Ok(UndoResult {
        new_head,
        ownership_update,
    })
}

#[cfg(test)]
mod test {
    #[cfg(test)]
    mod inner_undo_commit {
        use std::path::PathBuf;

        use gitbutler_commit::commit_ext::CommitExt as _;
        use gitbutler_repo::rebase::gitbutler_merge_commits;
        use gitbutler_testsupport::testing_repository::{
            assert_commit_tree_matches, TestingRepository,
        };

        use crate::undo_commit::{inner_undo_commit, UndoResult};

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

            let UndoResult {
                new_head,
                ownership_update,
            } = inner_undo_commit(&test_repository.repository, c.id(), c.id()).unwrap();

            assert_eq!(new_head, b.id(), "The new head should be C's parent");
            assert_eq!(
                ownership_update.len(),
                1,
                "Should have one ownership update"
            );
            assert_eq!(
                ownership_update[0].file_path,
                PathBuf::from("baz.txt"),
                "Ownership update should be for baz.txt"
            );
            assert_eq!(
                ownership_update[0].hunks.len(),
                1,
                "Ownership update should have one hunk"
            );
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
            let UndoResult {
                new_head,
                ownership_update,
            } = inner_undo_commit(&test_repository.repository, c.id(), b.id()).unwrap();

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
            assert_eq!(
                ownership_update.len(),
                1,
                "Should have one ownership update"
            );
            assert_eq!(
                ownership_update[0].file_path,
                PathBuf::from("foo.txt"),
                "Ownership update should be for foo.txt"
            );
            assert_eq!(
                ownership_update[0].hunks.len(),
                1,
                "Ownership update should have one hunk"
            );
        }
    }
}
