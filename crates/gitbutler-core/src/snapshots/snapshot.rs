use anyhow::anyhow;
use git2::FileMode;
use itertools::Itertools;
use std::fs;
use std::str::FromStr;

use anyhow::Result;

use crate::{projects::Project, virtual_branches::VirtualBranchesHandle};

use super::{
    entry::{OperationType, Snapshot, SnapshotDetails, Trailer},
    reflog::set_reference_to_oplog,
    state::OplogHandle,
};

const SNAPSHOT_FILE_LIMIT_BYTES: u64 = 32 * 1024 * 1024;

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

    // Create a blob out of `.git/gitbutler/virtual_branches.toml`
    let vb_path = repo_path.join(".git/gitbutler/virtual_branches.toml");
    let vb_content = fs::read(vb_path)?;
    let vb_blob = repo.blob(&vb_content)?;

    // Exclude files that are larger than the limit (eg. database.sql which may never be intended to be committed)
    let files_to_exclude = get_exclude_list(&repo)?;
    // In-memory, libgit2 internal ignore rule
    repo.add_ignore_rule(&files_to_exclude)?;

    // Add everything in the workdir to the index
    let mut index = repo.index()?;
    index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
    index.write()?;

    // Create a tree out of the index
    let tree_id = index.write_tree()?;

    let mut tree_builder = repo.treebuilder(None)?;
    tree_builder.insert("workdir", tree_id, FileMode::Tree.into())?;
    tree_builder.insert("virtual_branches.toml", vb_blob, FileMode::Blob.into())?;

    let tree_id = tree_builder.write()?;
    let tree = repo.find_tree(tree_id)?;

    // Construct a new commit
    let name = "GitButler";
    let email = "gitbutler@gitbutler.com";
    let signature = git2::Signature::now(name, email).unwrap();
    let new_commit_oid = repo.commit(
        None,
        &signature,
        &signature,
        &details.to_string(),
        &tree,
        &[&oplog_head_commit],
    )?;

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
    // Top tree
    let tree = commit.tree()?;
    let vb_tree_entry = tree
        .get_name("virtual_branches.toml")
        .ok_or(anyhow!("failed to get virtual_branches tree entry"))?;
    // virtual_branches.toml blob
    let vb_blob = vb_tree_entry
        .to_object(&repo)?
        .into_blob()
        .map_err(|_| anyhow!("failed to convert virtual_branches tree entry to blob"))?;
    let wd_tree_entry = tree
        .get_name("workdir")
        .ok_or(anyhow!("failed to get workdir tree entry"))?;
    // workdir tree
    let tree = repo.find_tree(wd_tree_entry.id())?;

    // Exclude files that are larger than the limit (eg. database.sql which may never be intended to be committed)
    let files_to_exclude = get_exclude_list(&repo)?;
    // In-memory, libgit2 internal ignore rule
    repo.add_ignore_rule(&files_to_exclude)?;

    // Define the checkout builder
    let mut checkout_builder = git2::build::CheckoutBuilder::new();
    checkout_builder.remove_untracked(true);
    checkout_builder.force();
    // Checkout the tree
    repo.checkout_tree(tree.as_object(), Some(&mut checkout_builder))?;

    // Update virtual_branches.toml with the state from the snapshot
    fs::write(
        repo_path.join(".git/gitbutler/virtual_branches.toml"),
        vb_blob.content(),
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

fn get_exclude_list(repo: &git2::Repository) -> Result<String> {
    let repo_path = repo
        .path()
        .parent()
        .ok_or(anyhow!("failed to get repo path"))?;
    let statuses = repo.statuses(None)?;
    let mut files_to_exclude = vec![];
    for entry in statuses.iter() {
        if let Some(path) = entry.path() {
            let path = repo_path.join(path);
            if let Ok(metadata) = fs::metadata(&path) {
                if metadata.is_file() && metadata.len() > SNAPSHOT_FILE_LIMIT_BYTES {
                    files_to_exclude.push(path);
                }
            }
        }
    }

    // Exclude files that are larger than the limit (eg. database.sql which may never be intended to be committed)
    let files_to_exclude = files_to_exclude
        .iter()
        .filter_map(|f| f.strip_prefix(repo_path).ok())
        .filter_map(|f| f.to_str())
        .join(" ");
    Ok(files_to_exclude)
}

#[cfg(test)]
mod tests {
    use std::{io::Write, path::PathBuf};

    use crate::virtual_branches::Branch;

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
        vb_state.set_default_target(default_target.clone()).unwrap();
        let file_path = dir.path().join("uncommitted.txt");
        std::fs::write(file_path, "test").unwrap();

        let file_path = dir.path().join("large.txt");
        // write 33MB of random data in the file
        let mut file = std::fs::File::create(file_path).unwrap();
        for _ in 0..33 * 1024 {
            let data = [0u8; 1024];
            file.write_all(&data).unwrap();
        }

        // create a snapshot
        create(&project, SnapshotDetails::new(OperationType::CreateCommit)).unwrap();
        let snapshots = list(&project, 100).unwrap();

        // The large file is still here but it will not be part of the snapshot
        let file_path = dir.path().join("large.txt");
        assert!(file_path.exists());

        // Modify file 1, remove file 2, create file 3
        let file_path = dir.path().join("1.txt");
        std::fs::write(file_path, "TEST").unwrap();
        let file_path = dir.path().join("2.txt");
        std::fs::remove_file(file_path).unwrap();
        let file_path = dir.path().join("3.txt");
        std::fs::write(file_path, "something_new").unwrap();
        let file_path = dir.path().join("uncommitted.txt");
        std::fs::write(file_path, "TEST").unwrap();

        // Create a fake branch in virtual_branches.toml
        let id = crate::id::Id::from_str("9acb2a3b-cddf-47d7-b531-a7798978c237").unwrap();
        vb_state
            .set_branch(Branch {
                id,
                ..Default::default()
            })
            .unwrap();
        assert!(vb_state.get_branch(&id).is_ok());

        // restore from the snapshot
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
        let file_path = dir.path().join("uncommitted.txt");
        let file_lines = std::fs::read_to_string(file_path).unwrap();
        assert_eq!(file_lines, "test");

        // The large file is still here but it was not be part of the snapshot
        let file_path = dir.path().join("large.txt");
        assert!(file_path.exists());

        // The fake branch is gone
        assert!(vb_state.get_branch(&id).is_err());
    }
}
