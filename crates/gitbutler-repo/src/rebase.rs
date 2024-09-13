use std::path::Path;

use anyhow::{anyhow, Context, Result};
use bstr::ByteSlice;
use gitbutler_cherry_pick::{ConflictedTreeKey, RepositoryExt};
use gitbutler_command_context::CommandContext;
use gitbutler_commit::{
    commit_ext::CommitExt,
    commit_headers::{CommitHeadersV2, HasCommitHeaders},
};
use gitbutler_error::error::Marker;

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
        .repository()
        .l(from_commit_oid, LogUntil::Commit(to_commit_oid))?;

    if ids_to_rebase.is_empty() {
        return Ok(None);
    }

    let new_head_id = cherry_rebase_group(
        ctx.repository(),
        target_commit_oid,
        &ids_to_rebase,
        ctx.project().succeeding_rebases,
    )?;

    Ok(Some(new_head_id))
}

/// takes a vector of commit oids and rebases them onto a target commit and returns the
/// new head commit oid if it's successful
/// the difference between this and a libgit2 based rebase is that this will successfully
/// rebase empty commits (two commits with identical trees)
pub fn cherry_rebase_group(
    repository: &git2::Repository,
    target_commit_oid: git2::Oid,
    ids_to_rebase: &[git2::Oid],
    succeeding_rebases: bool,
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

                if to_rebase.parent_ids().len() == 1 && head.id() == to_rebase.parent_id(0)? {
                    return Ok(to_rebase);
                };

                let cherrypick_index = repository
                    .cherry_pick_gitbutler(&head, &to_rebase, None)
                    .context("failed to cherry pick")?;

                if cherrypick_index.has_conflicts() {
                    if !succeeding_rebases {
                        return Err(anyhow!("failed to rebase")).context(Marker::BranchConflict);
                    }
                    commit_conflicted_cherry_result(repository, head, to_rebase, cherrypick_index)
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
    let commit_headers = to_rebase.gitbutler_headers();

    let is_merge_commit = to_rebase.parent_count() > 0;

    let merge_tree_oid = cherrypick_index
        .write_tree_to(repository)
        .context("failed to write merge tree")?;

    // Remove empty merge commits
    if is_merge_commit && merge_tree_oid == head.tree_id() {
        return Ok(head);
    }

    let merge_tree = repository
        .find_tree(merge_tree_oid)
        .context("failed to find merge tree")?;

    let commit_headers = commit_headers.map(|commit_headers| CommitHeadersV2 {
        conflicted: None,
        ..commit_headers
    });

    let commit_oid = crate::RepositoryExt::commit_with_signature(
        repository,
        None,
        &to_rebase.author(),
        &to_rebase.committer(),
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
    cherrypick_index: git2::Index,
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

    let mut conflicted_files = Vec::new();

    // get a list of conflicted files from the index
    let index_conflicts = cherrypick_index.conflicts()?.flatten().collect::<Vec<_>>();
    for conflict in index_conflicts {
        // For some reason we have to resolve the index with the "their" side
        // rather than the "our" side, so we then go and later overwrite the
        // output tree with the "our" side.
        let index_entry = conflict.their.or(conflict.our);
        if let Some(entry) = index_entry {
            let path = std::str::from_utf8(&entry.path).unwrap().to_string();
            conflicted_files.push(path)
        }
    }

    let mut resolved_index = repository.cherry_pick_gitbutler(
        &head,
        &to_rebase,
        Some(git2::MergeOptions::default().file_favor(git2::FileFavor::Ours)),
    )?;

    let resolved_index_conflicts = resolved_index.conflicts()?.flatten().collect::<Vec<_>>();
    for conflict in resolved_index_conflicts {
        if let (Some(their), None) = (&conflict.their, &conflict.our) {
            let path = std::str::from_utf8(&their.path).unwrap();
            let path = Path::new(path);
            resolved_index.remove_path(path)?;
        }
        if let (None, Some(our)) = (conflict.their, conflict.our) {
            let path = std::str::from_utf8(&our.path).unwrap();
            let path = Path::new(path);
            resolved_index.remove_path(path)?;
        }
    }

    let resolved_tree_id = resolved_index.write_tree_to(repository)?;

    // convert files into a string and save as a blob
    let conflicted_files_string = conflicted_files.join("\n");
    let conflicted_files_blob = repository.blob(conflicted_files_string.as_bytes())?;

    // create a treewriter
    let mut tree_writer = repository.treebuilder(None)?;

    let side0 = repository.find_real_tree(&head, Default::default())?;
    let side1 = repository.find_real_tree(&to_rebase, ConflictedTreeKey::Theirs)?;

    // save the state of the conflict, so we can recreate it later
    tree_writer.insert(&*ConflictedTreeKey::Ours, side0.id(), 0o040000)?;
    tree_writer.insert(&*ConflictedTreeKey::Theirs, side1.id(), 0o040000)?;
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
            .map(|commit_headers| {
                let conflicted_file_count = conflicted_files.len().try_into().expect(
                    "If you have more than 2^64 conflicting files, we've got bigger problems",
                );
                CommitHeadersV2 {
                    conflicted: Some(conflicted_file_count),
                    ..commit_headers
                }
            });

    let commit_oid = crate::RepositoryExt::commit_with_signature(
        repository,
        None,
        &to_rebase.author(),
        &to_rebase.committer(),
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
    let target_tree = repository.find_real_tree(&target_commit, Default::default())?;
    let incoming_tree = repository.find_real_tree(&incoming_commit, ConflictedTreeKey::Theirs)?;

    let merged_index = repository.merge_trees(&base_tree, &target_tree, &incoming_tree, None)?;

    let mut conflicted_files = vec![];

    // get a list of conflicted files from the index
    let index_conflicts = merged_index.conflicts()?.flatten().collect::<Vec<_>>();
    for conflict in index_conflicts {
        // For some reason we have to resolve the index with the "their" side
        // rather than the "our" side, so we then go and later overwrite the
        // output tree with the "our" side.
        let index_entry = conflict.their.or(conflict.our);
        if let Some(entry) = index_entry {
            let path = std::str::from_utf8(&entry.path).unwrap().to_string();
            conflicted_files.push(path)
        }
    }

    let mut resolved_index = repository.merge_trees(
        &base_tree,
        &target_tree,
        &incoming_tree,
        Some(git2::MergeOptions::default().file_favor(git2::FileFavor::Ours)),
    )?;

    let resolved_index_conflicts = resolved_index.conflicts()?.flatten().collect::<Vec<_>>();
    for conflict in resolved_index_conflicts {
        if let (Some(their), None) = (&conflict.their, &conflict.our) {
            let path = std::str::from_utf8(&their.path).unwrap();
            let path = Path::new(path);
            resolved_index.remove_path(path)?;
        }
        if let (None, Some(our)) = (conflict.their, conflict.our) {
            let path = std::str::from_utf8(&our.path).unwrap();
            let path = Path::new(path);
            resolved_index.remove_path(path)?;
        }
    }

    let resolved_tree_id = resolved_index.write_tree_to(repository)?;

    let (author, committer) = repository.signatures()?;

    // convert files into a string and save as a blob
    let conflicted_files_string = conflicted_files.join("\n");
    let conflicted_files_blob = repository.blob(conflicted_files_string.as_bytes())?;

    // create a treewriter
    let mut tree_writer = repository.treebuilder(None)?;

    // save the state of the conflict, so we can recreate it later
    tree_writer.insert(&*ConflictedTreeKey::Ours, target_tree.id(), 0o040000)?;
    tree_writer.insert(&*ConflictedTreeKey::Theirs, incoming_tree.id(), 0o040000)?;
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

    let tree_oid = tree_writer.write().context("failed to write tree")?;

    let commit_headers = incoming_commit
        .gitbutler_headers()
        .or_else(|| Some(Default::default()))
        .map(|commit_headers| {
            let conflicted_file_count = conflicted_files
                .len()
                .try_into()
                .expect("If you have more than 2^64 conflicting files, we've got bigger problems");

            if conflicted_file_count > 0 {
                CommitHeadersV2 {
                    conflicted: Some(conflicted_file_count),
                    ..commit_headers
                }
            } else {
                CommitHeadersV2 {
                    conflicted: None,
                    ..commit_headers
                }
            }
        });

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
        commit_headers,
    )
    .context("failed to create commit")?;

    Ok(repository.find_commit(commit_oid)?)
}
