use std::str::FromStr;

use anyhow::Result;

use crate::{projects::Project, virtual_branches::VirtualBranchesHandle};

use super::{
    entry::{OperationType, Snapshot, SnapshotDetails, Trailer},
    reflog::set_reference_to_oplog,
    state::OplogHandle,
};

/// Creates a snapshot of the current state of the repository and virtual branches using the given label.
///
/// If this is the first shapshot created, supporting structures are initialized:
///  - The current oplog head is persisted in `.git/gitbutler/oplog.toml`.
///  - A fake branch `gitbutler/target` is created and maintained in order to keep the oplog head reachable.
///
/// The state of virtual branches `.git/gitbutler/virtual_branches.toml` is copied to the project root so that it is snapshotted.
pub fn create(project: &Project, details: SnapshotDetails) -> Result<()> {
    if project.enable_snapshots.is_none() || project.enable_snapshots == Some(false) {
        return Ok(());
    }

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
        &details.to_string(),
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

    set_reference_to_oplog(
        project,
        &default_target_sha.to_string(),
        &new_commit_oid.to_string(),
    )?;

    Ok(())
}

/// Lists the snapshots that have been created for the given repository, up to the given limit.
/// An alternative way of retrieving the snapshots would be to manually the oplog head `git log <oplog_head>` available in `.git/gitbutler/oplog.toml`.
///
/// If there are no snapshots, an empty list is returned.
pub fn list(project: &Project, limit: usize) -> Result<Vec<Snapshot>> {
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

    for commit_id in revwalk {
        let commit_id = commit_id?;
        let commit = repo.find_commit(commit_id)?;

        if commit.parent_count() > 1 {
            break;
        }

        let details = commit
            .message()
            .and_then(|msg| SnapshotDetails::from_str(msg).ok());

        snapshots.push(Snapshot {
            id: commit_id.to_string(),
            details,
            created_at: commit.time().seconds() * 1000,
        });

        if snapshots.len() >= limit {
            break;
        }
    }

    Ok(snapshots)
}

/// Reverts to a previous state of the working directory, virtual branches and commits.
/// The provided sha must refer to a valid snapshot commit.
/// Upon success, a new snapshot is created.
///
/// The state of virtual branches `.git/gitbutler/virtual_branches.toml` is restored from the snapshot.
pub fn restore(project: &Project, sha: String) -> Result<()> {
    let repo_path = project.path.as_path();
    let repo = git2::Repository::init(repo_path)?;

    let commit = repo.find_commit(git2::Oid::from_str(&sha)?)?;
    let tree = commit.tree()?;

    // Define the checkout builder
    let mut checkout_builder = git2::build::CheckoutBuilder::new();
    checkout_builder.remove_untracked(true);
    checkout_builder.force();
    // Checkout the tree
    repo.checkout_tree(tree.as_object(), Some(&mut checkout_builder))?;

    // mv virtual_branches.toml from project root to .git/gitbutler
    std::fs::rename(
        repo_path.join("virtual_branches.toml"),
        repo_path.join(".git/gitbutler/virtual_branches.toml"),
    )?;

    // create new snapshot
    let details = SnapshotDetails {
        version: Default::default(),
        operation: OperationType::RestoreFromSnapshot,
        title: "Restored from snapshot".to_string(),
        body: None,
        trailers: vec![Trailer {
            key: "restored_from".to_string(),
            value: sha,
        }],
    };
    create(project, details)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_and_restore() {
        let dir = tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let file_path = dir.path().join("1.txt");
        std::fs::write(file_path, "test").unwrap();
        let file_path = dir.path().join("2.txt");
        std::fs::write(file_path, "test").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(&PathBuf::from("1.txt")).unwrap();
        index.add_path(&PathBuf::from("2.txt")).unwrap();
        let oid = index.write_tree().unwrap();
        let name = "Your Name";
        let email = "your.email@example.com";
        let signature = git2::Signature::now(name, email).unwrap();
        let initial_commit = repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                "initial commit",
                &repo.find_tree(oid).unwrap(),
                &[],
            )
            .unwrap();

        // create a new branch called "gitbutler/integraion" from initial commit
        repo.branch(
            "gitbutler/integration",
            &repo.find_commit(initial_commit).unwrap(),
            false,
        )
        .unwrap();

        let project = Project {
            path: dir.path().to_path_buf(),
            enable_snapshots: Some(true),
            ..Default::default()
        };
        // create gb_dir folder
        std::fs::create_dir_all(project.gb_dir()).unwrap();

        let vb_state = VirtualBranchesHandle::new(&project.gb_dir());

        let target_sha = initial_commit.to_string();
        let default_target = crate::virtual_branches::target::Target {
            branch: crate::git::RemoteRefname::new("origin", "main"),
            remote_url: Default::default(),
            sha: crate::git::Oid::from_str(&target_sha).unwrap(),
        };
        vb_state.set_default_target(default_target).unwrap();

        // create a snapshot
        create(&project, SnapshotDetails::new(OperationType::CreateCommit)).unwrap();
        let snapshots = list(&project, 100).unwrap();

        let file_path = dir.path().join("1.txt");
        std::fs::write(file_path, "TEST").unwrap();
        let file_path = dir.path().join("2.txt");
        std::fs::remove_file(file_path).unwrap();
        let file_path = dir.path().join("3.txt");
        std::fs::write(file_path, "something_new").unwrap();
        // File 1 was modified, file 2 was deleted, file 3 was added

        restore(&project, snapshots.first().unwrap().id.clone()).unwrap();

        let file_path = dir.path().join("1.txt");
        let file_lines = std::fs::read_to_string(file_path).unwrap();
        assert_eq!(file_lines, "test");
        let file_path = dir.path().join("2.txt");
        assert!(file_path.exists());
        let file_lines = std::fs::read_to_string(file_path).unwrap();
        assert_eq!(file_lines, "test");
        let file_path = dir.path().join("3.txt");
        assert!(!file_path.exists());
    }
}
