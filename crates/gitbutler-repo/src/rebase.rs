use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use bstr::ByteSlice;
use gitbutler_cherry_pick::{ConflictedTreeKey, RepositoryExt};
use gitbutler_command_context::CommandContext;
use gitbutler_commit::{
    commit_ext::CommitExt,
    commit_headers::{CommitHeadersV2, HasCommitHeaders},
};
use serde::{Deserialize, Serialize};

use crate::{LogUntil, RepositoryExt as _};

/// cherry-pick based rebase, which handles empty commits
/// this function takes a commit range and generates a Vector of commit oids
/// and then passes them to `cherry_rebase_group` to rebase them onto the target commit
///
/// Returns the new head commit id
pub fn cherry_rebase(
    ctx: &CommandContext,
    target_commit_oid: git2::Oid,
    to_commit_oid: git2::Oid,
    from_commit_oid: git2::Oid,
) -> Result<Option<git2::Oid>> {
    // get a list of the commits to rebase
    let ids_to_rebase = ctx
        .repo()
        .l(from_commit_oid, LogUntil::Commit(to_commit_oid), false)?;

    if ids_to_rebase.is_empty() {
        return Ok(None);
    }

    let new_head_id = cherry_rebase_group(ctx.repo(), target_commit_oid, &ids_to_rebase, false)?;

    Ok(Some(new_head_id))
}

/// takes a vector of commit oids and rebases them onto a target commit and returns the
/// new head commit oid if it's successful
/// the difference between this and a libgit2 based rebase is that this will successfully
/// rebase empty commits (two commits with identical trees)
///
/// the commit id's to rebase should be ordered such that the child most commit is first
pub fn cherry_rebase_group(
    repository: &git2::Repository,
    target_commit_oid: git2::Oid,
    ids_to_rebase: &[git2::Oid],
    always_rebase: bool,
) -> Result<git2::Oid> {
    // now, rebase unchanged commits onto the new commit
    let commits_to_rebase = ids_to_rebase
        .iter()
        .map(|oid| repository.find_commit(oid.to_owned()))
        .rev()
        .collect::<Result<Vec<_>, _>>()
        .context("failed to read commits to rebase")?;

    let new_head_id = commits_to_rebase
        .into_iter()
        .fold(
            repository
                .find_commit(target_commit_oid)
                .context("failed to find new commit"),
            |head, to_rebase| {
                let head = head?;

                if !always_rebase
                    && to_rebase.parent_ids().len() == 1
                    && head.id() == to_rebase.parent_id(0)?
                {
                    return Ok(to_rebase);
                };

                let mut cherrypick_index = repository
                    .cherry_pick_gitbutler(&head, &to_rebase, None)
                    .context("failed to cherry pick")?;

                if cherrypick_index.has_conflicts() {
                    commit_conflicted_cherry_result(
                        repository,
                        head,
                        to_rebase,
                        &mut cherrypick_index,
                    )
                } else {
                    commit_unconflicted_cherry_result(repository, head, to_rebase, cherrypick_index)
                }
            },
        )?
        .id();

    Ok(new_head_id)
}

fn commit_unconflicted_cherry_result<'repository>(
    repository: &'repository git2::Repository,
    head: git2::Commit<'repository>,
    to_rebase: git2::Commit,
    mut cherrypick_index: git2::Index,
) -> Result<git2::Commit<'repository>> {
    let merge_tree_oid = cherrypick_index
        .write_tree_to(repository)
        .context("failed to write merge tree")?;

    // Remove empty commits
    if merge_tree_oid == head.tree_id() {
        return Ok(head);
    }

    let merge_tree = repository
        .find_tree(merge_tree_oid)
        .context("failed to find merge tree")?;

    // Set conflicted header to None
    let commit_headers = to_rebase
        .gitbutler_headers()
        .map(|commit_headers| CommitHeadersV2 {
            conflicted: None,
            ..commit_headers
        });

    let (_, committer) = repository.signatures()?;

    let commit_oid = crate::RepositoryExt::commit_with_signature(
        repository,
        None,
        &to_rebase.author(),
        &committer,
        &to_rebase.message_bstr().to_str_lossy(),
        &merge_tree,
        &[&head],
        commit_headers,
    )
    .context("failed to create commit")?;

    repository
        .find_commit(commit_oid)
        .context("failed to find commit")
}

fn commit_conflicted_cherry_result<'repository>(
    repository: &'repository git2::Repository,
    head: git2::Commit,
    to_rebase: git2::Commit,
    cherrypick_index: &mut git2::Index,
) -> Result<git2::Commit<'repository>> {
    let commit_headers = to_rebase.gitbutler_headers();

    // If the commit we're rebasing is conflicted, use the commits original base.
    let base_tree = if to_rebase.is_conflicted() {
        repository.find_real_tree(&to_rebase, ConflictedTreeKey::Base)?
    } else {
        let base_commit = to_rebase.parent(0)?;
        repository.find_real_tree(&base_commit, Default::default())?
    };

    // in case someone checks this out with vanilla Git, we should warn why it looks like this
    let readme_content =
        b"You have checked out a GitButler Conflicted commit. You probably didn't mean to do this.";
    let readme_blob = repository.blob(readme_content)?;

    let conflicted_files = resolve_index(repository, cherrypick_index)?;

    let resolved_tree_id = cherrypick_index.write_tree_to(repository)?;

    // convert files into a string and save as a blob
    let conflicted_files_string = toml::to_string(&conflicted_files)?;
    let conflicted_files_blob = repository.blob(conflicted_files_string.as_bytes())?;

    // create a treewriter
    let mut tree_writer = repository.treebuilder(None)?;

    let head_tree = repository.find_real_tree(&head, Default::default())?;
    let to_rebase_tree = repository.find_real_tree(&to_rebase, ConflictedTreeKey::Theirs)?;

    // save the state of the conflict, so we can recreate it later
    tree_writer.insert(&*ConflictedTreeKey::Ours, head_tree.id(), 0o040000)?;
    tree_writer.insert(&*ConflictedTreeKey::Theirs, to_rebase_tree.id(), 0o040000)?;
    tree_writer.insert(&*ConflictedTreeKey::Base, base_tree.id(), 0o040000)?;
    tree_writer.insert(
        &*ConflictedTreeKey::AutoResolution,
        resolved_tree_id,
        0o040000,
    )?;
    tree_writer.insert(
        &*ConflictedTreeKey::ConflictFiles,
        conflicted_files_blob,
        0o100644,
    )?;
    tree_writer.insert("README.txt", readme_blob, 0o100644)?;

    let tree_oid = tree_writer.write().context("failed to write tree")?;

    let commit_headers =
        commit_headers
            .or_else(|| Some(Default::default()))
            .map(|commit_headers| CommitHeadersV2 {
                conflicted: Some(conflicted_files.total_entries() as u64),
                ..commit_headers
            });

    let (_, committer) = repository.signatures()?;

    let commit_oid = crate::RepositoryExt::commit_with_signature(
        repository,
        None,
        &to_rebase.author(),
        &committer,
        &to_rebase.message_bstr().to_str_lossy(),
        &repository
            .find_tree(tree_oid)
            .context("failed to find tree")?,
        &[&head],
        commit_headers,
    )
    .context("failed to create commit")?;

    repository
        .find_commit(commit_oid)
        .context("failed to find commit")
}

/// Merge two commits together
///
/// The `target_commit` and `incoming_commit` must have a common ancestor.
///
/// If there is a merge conflict, the
pub fn gitbutler_merge_commits<'repository>(
    repository: &'repository git2::Repository,
    target_commit: git2::Commit<'repository>,
    incoming_commit: git2::Commit<'repository>,
    target_branch_name: &str,
    incoming_branch_name: &str,
) -> Result<git2::Commit<'repository>> {
    let merge_base = repository.merge_base(target_commit.id(), incoming_commit.id())?;
    let merge_base = repository.find_commit(merge_base)?;

    let base_tree = repository.find_real_tree(&merge_base, Default::default())?;
    // We want to use the auto-resolution when computing the merge, but for
    // reconstructing it later, we want the "theirsiest" and "oursiest" trees
    let target_tree = repository.find_real_tree(&target_commit, ConflictedTreeKey::Theirs)?;
    let incoming_tree = repository.find_real_tree(&incoming_commit, ConflictedTreeKey::Ours)?;

    let target_merge_tree = repository.find_real_tree(&target_commit, Default::default())?;
    let incoming_merge_tree = repository.find_real_tree(&incoming_commit, Default::default())?;
    let mut merged_index =
        repository.merge_trees(&base_tree, &incoming_merge_tree, &target_merge_tree, None)?;

    let tree_oid;
    let conflicted_files;

    if merged_index.has_conflicts() {
        conflicted_files = resolve_index(repository, &mut merged_index)?;

        // Index gets resolved from the `resolve_index` call above, so we can safly write it out
        let resolved_tree_id = merged_index.write_tree_to(repository)?;

        // convert files into a string and save as a blob
        let conflicted_files_string = toml::to_string(&conflicted_files)?;
        let conflicted_files_blob = repository.blob(conflicted_files_string.as_bytes())?;

        // create a treewriter
        let mut tree_writer = repository.treebuilder(None)?;

        // save the state of the conflict, so we can recreate it later
        tree_writer.insert(&*ConflictedTreeKey::Ours, incoming_tree.id(), 0o040000)?;
        tree_writer.insert(&*ConflictedTreeKey::Theirs, target_tree.id(), 0o040000)?;
        tree_writer.insert(&*ConflictedTreeKey::Base, base_tree.id(), 0o040000)?;
        tree_writer.insert(
            &*ConflictedTreeKey::AutoResolution,
            resolved_tree_id,
            0o040000,
        )?;
        tree_writer.insert(
            &*ConflictedTreeKey::ConflictFiles,
            conflicted_files_blob,
            0o100644,
        )?;

        // in case someone checks this out with vanilla Git, we should warn why it looks like this
        let readme_content =
        b"You have checked out a GitButler Conflicted commit. You probably didn't mean to do this.";
        let readme_blob = repository.blob(readme_content)?;
        tree_writer.insert("README.txt", readme_blob, 0o100644)?;

        tree_oid = tree_writer.write().context("failed to write tree")?;
    } else {
        conflicted_files = Default::default();
        tree_oid = merged_index.write_tree_to(repository)?;
    }

    let conflicted_file_count = conflicted_files.total_entries() as u64;

    let commit_headers = if conflicted_file_count > 0 {
        CommitHeadersV2 {
            conflicted: Some(conflicted_file_count),
            ..Default::default()
        }
    } else {
        CommitHeadersV2 {
            conflicted: None,
            ..Default::default()
        }
    };

    let (author, committer) = repository.signatures()?;

    let commit_oid = crate::RepositoryExt::commit_with_signature(
        repository,
        None,
        &author,
        &committer,
        &format!(
            "Merge `{}` into `{}`",
            incoming_branch_name, target_branch_name
        ),
        &repository
            .find_tree(tree_oid)
            .context("failed to find tree")?,
        &[&target_commit, &incoming_commit],
        Some(commit_headers),
    )
    .context("failed to create commit")?;

    Ok(repository.find_commit(commit_oid)?)
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConflictEntries {
    ancestor_entries: Vec<PathBuf>,
    our_entries: Vec<PathBuf>,
    their_entries: Vec<PathBuf>,
}

impl ConflictEntries {
    pub fn has_entries(&self) -> bool {
        !self.ancestor_entries.is_empty()
            || !self.our_entries.is_empty()
            || !self.their_entries.is_empty()
    }

    pub fn total_entries(&self) -> usize {
        let set = self
            .ancestor_entries
            .iter()
            .chain(self.our_entries.iter())
            .chain(self.their_entries.iter())
            .collect::<HashSet<_>>();

        set.len()
    }
}

/// Automatically resolves an index with a preferences for the "our" side
///
/// Within our rebasing and merging logic, "their" is the commit that is getting
/// cherry picked, and "our" is the commit that it is getting cherry picked on
/// to.
///
/// This means that if we experience a conflict, we drop the changes that are
/// in the commit that is getting cherry picked in favor of what came before it
fn resolve_index(
    repository: &git2::Repository,
    index: &mut git2::Index,
) -> Result<ConflictEntries, anyhow::Error> {
    fn bytes_to_path(path: &[u8]) -> Result<PathBuf> {
        let path = std::str::from_utf8(path)?;
        Ok(Path::new(path).to_owned())
    }

    let mut ancestor_entries = vec![];
    let mut our_entries = vec![];
    let mut their_entries = vec![];

    // Set the index on an in-memory repository
    let in_memory_repository = repository.in_memory_repo()?;
    in_memory_repository.set_index(index)?;

    let index_conflicts = index.conflicts()?.flatten().collect::<Vec<_>>();

    for mut conflict in index_conflicts {
        // There may be a case when there is an ancestor in the index without
        // a "their" OR "our" side. This is probably caused by the same file
        // getting renamed and modified in the two commits.
        if let Some(ancestor) = &conflict.ancestor {
            let path = bytes_to_path(&ancestor.path)?;
            index.remove_path(&path)?;

            ancestor_entries.push(path);
        }

        if let (Some(their), None) = (&conflict.their, &conflict.our) {
            // Their (the commit we're rebasing)'s change gets dropped
            let their_path = bytes_to_path(&their.path)?;
            index.remove_path(&their_path)?;

            their_entries.push(their_path);
        } else if let (None, Some(our)) = (&conflict.their, &mut conflict.our) {
            // Our (the commit we're rebasing onto)'s gets kept
            let blob = repository.find_blob(our.id)?;
            our.flags = 0; // For some unknown reason we need to set flags to 0
            index.add_frombuffer(our, blob.content())?;

            let our_path = bytes_to_path(&our.path)?;

            our_entries.push(our_path);
        } else if let (Some(their), Some(our)) = (&conflict.their, &mut conflict.our) {
            // We keep our (the commit we're rebasing onto)'s side of the
            // conflict
            let their_path = bytes_to_path(&their.path)?;
            let blob = repository.find_blob(our.id)?;

            index.remove_path(&their_path)?;
            our.flags = 0; // For some unknown reason we need to set flags to 0
            index.add_frombuffer(our, blob.content())?;

            let our_path = bytes_to_path(&our.path)?;

            their_entries.push(their_path);
            our_entries.push(our_path);
        }
    }

    Ok(ConflictEntries {
        ancestor_entries,
        our_entries,
        their_entries,
    })
}

#[cfg(test)]
mod test {

    #[cfg(test)]
    mod resolve_index {
        use crate::rebase::resolve_index;
        use gitbutler_testsupport::testing_repository::TestingRepository;

        #[test]
        fn test_same_file_twice() {
            let test_repository = TestingRepository::open();

            // Make some commits
            let a = test_repository.commit_tree(None, &[("foo.txt", "a")]);
            let b = test_repository.commit_tree(None, &[("foo.txt", "b")]);
            let c = test_repository.commit_tree(None, &[("foo.txt", "c")]);
            test_repository.commit_tree(None, &[("foo.txt", "asdfasdf")]);

            // Merge the index
            let mut index: git2::Index = test_repository
                .repository
                .merge_trees(
                    &a.tree().unwrap(), // Base
                    &b.tree().unwrap(), // Ours
                    &c.tree().unwrap(), // Theirs
                    None,
                )
                .unwrap();

            assert!(index.has_conflicts());

            // Call our index resolution function
            resolve_index(&test_repository.repository, &mut index).unwrap();

            // Ensure there are no conflicts
            assert!(!index.has_conflicts());

            let tree = index.write_tree_to(&test_repository.repository).unwrap();
            let tree: git2::Tree = test_repository.repository.find_tree(tree).unwrap();

            let blob = tree.get_name("foo.txt").unwrap().id(); // We fail here to get the entry because the tree is empty
            let blob: git2::Blob = test_repository.repository.find_blob(blob).unwrap();

            assert_eq!(blob.content(), b"b")
        }

        #[test]
        fn test_diverging_renames() {
            let test_repository = TestingRepository::open();

            // Make some commits
            let a = test_repository.commit_tree(None, &[("foo.txt", "a")]);
            let b = test_repository.commit_tree(None, &[("bar.txt", "a")]);
            let c = test_repository.commit_tree(None, &[("baz.txt", "a")]);
            test_repository.commit_tree(None, &[("foo.txt", "asdfasdf")]);

            // Merge the index
            let mut index: git2::Index = test_repository
                .repository
                .merge_trees(
                    &a.tree().unwrap(), // Base
                    &b.tree().unwrap(), // Ours
                    &c.tree().unwrap(), // Theirs
                    None,
                )
                .unwrap();

            assert!(index.has_conflicts());

            // Call our index resolution function
            resolve_index(&test_repository.repository, &mut index).unwrap();

            // Ensure there are no conflicts
            assert!(!index.has_conflicts());

            let tree = index.write_tree_to(&test_repository.repository).unwrap();
            let tree: git2::Tree = test_repository.repository.find_tree(tree).unwrap();

            assert!(tree.get_name("foo.txt").is_none());
            assert!(tree.get_name("baz.txt").is_none());

            let blob = tree.get_name("bar.txt").unwrap().id(); // We fail here to get the entry because the tree is empty
            let blob: git2::Blob = test_repository.repository.find_blob(blob).unwrap();

            assert_eq!(blob.content(), b"a")
        }

        #[test]
        fn test_converging_renames() {
            let test_repository = TestingRepository::open();

            // Make some commits
            let a = test_repository.commit_tree(None, &[("foo.txt", "a"), ("bar.txt", "b")]);
            let b = test_repository.commit_tree(None, &[("baz.txt", "a")]);
            let c = test_repository.commit_tree(None, &[("baz.txt", "b")]);
            test_repository.commit_tree(None, &[("foo.txt", "asdfasdf")]);

            // Merge the index
            let mut index: git2::Index = test_repository
                .repository
                .merge_trees(
                    &a.tree().unwrap(), // Base
                    &b.tree().unwrap(), // Ours
                    &c.tree().unwrap(), // Theirs
                    None,
                )
                .unwrap();

            assert!(index.has_conflicts());

            // Call our index resolution function
            resolve_index(&test_repository.repository, &mut index).unwrap();

            // Ensure there are no conflicts
            assert!(!index.has_conflicts());

            let tree = index.write_tree_to(&test_repository.repository).unwrap();
            let tree: git2::Tree = test_repository.repository.find_tree(tree).unwrap();

            assert!(tree.get_name("foo.txt").is_none());
            assert!(tree.get_name("bar.txt").is_none());

            let blob = tree.get_name("baz.txt").unwrap().id(); // We fail here to get the entry because the tree is empty
            let blob: git2::Blob = test_repository.repository.find_blob(blob).unwrap();

            assert_eq!(blob.content(), b"a")
        }
    }
}
