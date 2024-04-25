use anyhow::Result;
use serde::Serialize;

use crate::{projects::Project, virtual_branches::VirtualBranchesHandle};

use super::{reflog::SnapshotsReference, state::OplogHandle};

#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotEntry {
    pub sha: String,
    pub label: String,
    pub created_at: i64, // milliseconds since epoch
}

pub fn create(project: Project, label: String) -> Result<()> {
    let repo_path = project.path.as_path();
    let repo = git2::Repository::init(repo_path)?;

    let vb_state = VirtualBranchesHandle::new(&project.gb_dir());
    let default_target_sha = vb_state.get_default_target()?.sha;

    let oplog_state = OplogHandle::new(&project.gb_dir());
    let oplog_head_commit = match oplog_state.get_oplog_head()? {
        Some(head_sha) => match repo.find_commit(git2::Oid::from_str(&head_sha)?) {
            Ok(commit) => commit,
            Err(_) => repo.find_commit(default_target_sha.into())?,
        },
        // This is the first snapshot - use the default target as starting point
        None => repo.find_commit(default_target_sha.into())?,
    };

    // Copy virtual_branches.rs to the project root so that we snapshot it
    std::fs::copy(
        repo_path.join(".git/gitbutler/virtual_branches.toml"),
        repo_path.join("virtual_branches.toml"),
    )?;

    // Add everything in the workdir to the index
    let mut index = repo.index()?;
    index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
    index.write()?;

    // Create a tree out of the index
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    // Construct a new commit
    let signature = repo.signature()?;
    let new_commit_oid = repo.commit(
        None,
        &signature,
        &signature,
        &label,
        &tree,
        &[&oplog_head_commit],
    )?;

    // Remove the copied virtual_branches.rs
    std::fs::remove_file(project.path.join("virtual_branches.toml"))?;

    // Reset the workdir to how it was
    let integration_branch = repo
        .find_branch("gitbutler/integration", git2::BranchType::Local)?
        .get()
        .peel_to_commit()?;

    repo.reset(
        &integration_branch.into_object(),
        git2::ResetType::Mixed,
        None,
    )?;

    oplog_state.set_oplog_head(new_commit_oid.to_string())?;

    let reflog_hack = SnapshotsReference::new(project, &default_target_sha.to_string())?;
    reflog_hack.set_target_ref(&default_target_sha.to_string())?;
    reflog_hack.set_oplog_ref(&new_commit_oid.to_string())?;

    Ok(())
}

pub fn list(project: Project, limit: usize) -> Result<Vec<SnapshotEntry>> {
    let repo_path = project.path.as_path();
    let repo = git2::Repository::init(repo_path)?;

    let oplog_state = OplogHandle::new(&project.gb_dir());
    let head_sha = oplog_state.get_oplog_head()?;
    if head_sha.is_none() {
        // there are no snapshots to return
        return Ok(vec![]);
    }
    let head_sha = head_sha.unwrap();

    let oplog_head_commit = repo.find_commit(git2::Oid::from_str(&head_sha)?)?;

    let mut revwalk = repo.revwalk()?;
    revwalk.push(oplog_head_commit.id())?;

    let mut snapshots = Vec::new();

    // TODO: how do we know when to stop?
    for commit_id in revwalk {
        let commit_id = commit_id?;
        let commit = repo.find_commit(commit_id)?;

        if commit.parent_count() > 1 {
            break;
        }
        snapshots.push(SnapshotEntry {
            sha: commit_id.to_string(),
            label: commit.summary().unwrap_or_default().to_string(),
            created_at: commit.time().seconds() * 1000,
        });

        if snapshots.len() >= limit {
            break;
        }
    }

    Ok(snapshots)
}

pub fn restore(project: Project, sha: String) -> Result<()> {
    let repo_path = project.path.as_path();
    let repo = git2::Repository::init(repo_path)?;

    let commit = repo.find_commit(git2::Oid::from_str(&sha)?)?;
    let tree = commit.tree()?;

    // Define the checkout builder
    let mut checkout_builder = git2::build::CheckoutBuilder::new();
    checkout_builder.force();
    // Checkout the tree
    repo.checkout_tree(tree.as_object(), Some(&mut checkout_builder))?;

    // mv virtual_branches.toml from project root to .git/gitbutler
    std::fs::rename(
        repo_path.join("virtual_branches.toml"),
        repo_path.join(".git/gitbutler/virtual_branches.toml"),
    )?;

    // create new snapshot
    let label = format!(
        "Restored from snapshot {}",
        commit.message().unwrap_or(&sha)
    );
    create(project, label)?;

    Ok(())
}
