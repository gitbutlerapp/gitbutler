use anyhow::anyhow;
use git2::FileMode;
use itertools::Itertools;
use std::collections::HashMap;
use std::str::FromStr;
use std::{fs, path::PathBuf};

use anyhow::Result;

use crate::git::diff::FileDiff;
use crate::{git::diff::hunks_by_filepath, projects::Project};

use super::{
    entry::{OperationType, Snapshot, SnapshotDetails, Trailer},
    reflog::set_reference_to_oplog,
    state::OplogHandle,
};

const SNAPSHOT_FILE_LIMIT_BYTES: u64 = 32 * 1024 * 1024;

/// The Oplog trait allows for crating snapshots of the current state of the project as well as restoring to a previous snapshot.
/// Snapshots include the state of the working directory as well as all additional GitButler state (e.g virtual branches, conflict state).
pub trait Oplog {
    /// Creates a snapshot of the current state of the repository and virtual branches using the given label.
    ///
    /// If this is the first shapshot created, supporting structures are initialized:
    ///  - The current oplog head is persisted in `.git/gitbutler/oplog.toml`.
    ///  - A fake branch `gitbutler/target` is created and maintained in order to keep the oplog head reachable.
    ///
    /// The snapshot tree contains:
    ///  - The current state of the working directory under a subtree `workdir`.
    ///  - The state of virtual branches from `.git/gitbutler/virtual_branches.toml` as a blob `virtual_branches.toml`.
    ///  - The state of conflicts from `.git/base_merge_parent` and `.git/conflicts` if present as blobs under a subtree `conflicts`
    ///
    /// If there are files that are untracked and larger than SNAPSHOT_FILE_LIMIT_BYTES, they are excluded from snapshot creation and restoring.
    /// Returns the sha of the created snapshot commit or None if snapshots are disabled.
    fn create_snapshot(&self, details: SnapshotDetails) -> Result<Option<String>>;
    /// Lists the snapshots that have been created for the given repository, up to the given limit.
    /// An alternative way of retrieving the snapshots would be to manually the oplog head `git log <oplog_head>` available in `.git/gitbutler/oplog.toml`.
    ///
    /// If there are no snapshots, an empty list is returned.
    fn list_snapshots(&self, limit: usize, sha: Option<String>) -> Result<Vec<Snapshot>>;
    /// Reverts to a previous state of the working directory, virtual branches and commits.
    /// The provided sha must refer to a valid snapshot commit.
    /// Upon success, a new snapshot is created.
    ///
    /// This will restore the following:
    ///  - The state of the working directory is checked out from the subtree `workdir` in the snapshot.
    ///  - The state of virtual branches is restored from the blob `virtual_branches.toml` in the snapshot.
    ///  - The state of conflicts (.git/base_merge_parent and .git/conflicts) is restored from the subtree `conflicts` in the snapshot (if not present, existing files are deleted).
    ///
    /// If there are files that are untracked and larger than SNAPSHOT_FILE_LIMIT_BYTES, they are excluded from snapshot creation and restoring.
    /// Returns the sha of the created revert snapshot commit or None if snapshots are disabled.
    fn restore_snapshot(&self, sha: String) -> Result<Option<String>>;
    /// Returns the number of lines of code (added plus removed) since the last snapshot. Includes untracked files.
    ///
    /// If there are no snapshots, 0 is returned.
    fn lines_since_snapshot(&self) -> Result<usize>;
    /// Returns the diff of the snapshot and it's parent. It only includes the workdir changes.
    ///
    /// This is useful to show what has changed in this particular snapshot
    fn snapshot_diff(&self, sha: String) -> Result<HashMap<PathBuf, FileDiff>>;
}

impl Oplog for Project {
    fn create_snapshot(&self, details: SnapshotDetails) -> Result<Option<String>> {
        if self.enable_snapshots.is_none() || self.enable_snapshots == Some(false) {
            return Ok(None);
        }

        let repo_path = self.path.as_path();
        let repo = git2::Repository::init(repo_path)?;

        let vb_state = self.virtual_branches();
        let default_target_sha = vb_state.get_default_target()?.sha;

        let oplog_state = OplogHandle::new(&self.gb_dir());
        let oplog_head_commit = match oplog_state.get_oplog_head()? {
            Some(head_sha) => match repo.find_commit(git2::Oid::from_str(&head_sha)?) {
                Ok(commit) => commit,
                Err(_) => repo.find_commit(default_target_sha.into())?,
            },
            // This is the first snapshot - use the default target as starting point
            None => repo.find_commit(default_target_sha.into())?,
        };

        // Create a blob out of `.git/gitbutler/virtual_branches.toml`
        let vb_path = repo_path
            .join(".git")
            .join("gitbutler")
            .join("virtual_branches.toml");
        let vb_content = fs::read(vb_path)?;
        let vb_blob = repo.blob(&vb_content)?;

        // Create a tree out of the conflicts state if present
        let conflicts_tree = write_conflicts_tree(repo_path, &repo)?;

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
        tree_builder.insert("conflicts", conflicts_tree, FileMode::Tree.into())?;

        let tree_id = tree_builder.write()?;
        let tree = repo.find_tree(tree_id)?;

        // Check if there is a difference between the tree and the parent tree, and if not, return so that we dont create noop snapshots
        let parent_tree = oplog_head_commit.tree()?;
        let diff = repo.diff_tree_to_tree(Some(&parent_tree), Some(&tree), None)?;
        if diff.deltas().count() == 0 {
            return Ok(None);
        }

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
            self,
            &default_target_sha.to_string(),
            &new_commit_oid.to_string(),
        )?;

        Ok(Some(new_commit_oid.to_string()))
    }

    fn list_snapshots(&self, limit: usize, sha: Option<String>) -> Result<Vec<Snapshot>> {
        let repo_path = self.path.as_path();
        let repo = git2::Repository::init(repo_path)?;

        let head_sha = match sha {
            Some(sha) => sha,
            None => {
                let oplog_state = OplogHandle::new(&self.gb_dir());
                if let Some(sha) = oplog_state.get_oplog_head()? {
                    sha
                } else {
                    // there are no snapshots so return an empty list
                    return Ok(vec![]);
                }
            }
        };

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

            let tree = commit.tree()?;
            let wd_tree_entry = tree.get_name("workdir");
            let tree = if let Some(wd_tree_entry) = wd_tree_entry {
                repo.find_tree(wd_tree_entry.id())?
            } else {
                // We reached a tree that is not a snapshot
                continue;
            };

            let parent_tree = commit.parent(0)?.tree()?;
            let parent_tree_entry = parent_tree.get_name("workdir");
            let parent_tree = parent_tree_entry
                .map(|entry| repo.find_tree(entry.id()))
                .transpose()?;

            let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;
            let stats = diff.stats()?;

            let mut files_changed = Vec::new();
            diff.print(git2::DiffFormat::NameOnly, |delta, _, _| {
                if let Some(path) = delta.new_file().path() {
                    files_changed.push(path.to_path_buf());
                }
                true
            })?;

            let lines_added = stats.insertions();
            let lines_removed = stats.deletions();

            let details = commit
                .message()
                .and_then(|msg| SnapshotDetails::from_str(msg).ok());

            snapshots.push(Snapshot {
                id: commit_id.to_string(),
                details,
                lines_added,
                lines_removed,
                files_changed,
                created_at: commit.time().seconds() * 1000,
            });

            if snapshots.len() >= limit {
                break;
            }
        }

        Ok(snapshots)
    }

    fn restore_snapshot(&self, sha: String) -> Result<Option<String>> {
        let repo_path = self.path.as_path();
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
        // Restore the state of .git/base_merge_parent and .git/conflicts from the snapshot
        // Will remove those files if they are not present in the snapshot
        _ = restore_conflicts_tree(&tree, &repo, repo_path);
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
            repo_path
                .join(".git")
                .join("gitbutler")
                .join("virtual_branches.toml"),
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
        self.create_snapshot(details)
    }

    fn lines_since_snapshot(&self) -> Result<usize> {
        let repo_path = self.path.as_path();
        let repo = git2::Repository::init(repo_path)?;

        // Exclude files that are larger than the limit (eg. database.sql which may never be intended to be committed)
        let files_to_exclude = get_exclude_list(&repo)?;
        // In-memory, libgit2 internal ignore rule
        repo.add_ignore_rule(&files_to_exclude)?;

        let oplog_state = OplogHandle::new(&self.gb_dir());
        let head_sha = oplog_state.get_oplog_head()?;
        if head_sha.is_none() {
            return Ok(0);
        }
        let head_sha = head_sha.unwrap();

        let commit = repo.find_commit(git2::Oid::from_str(&head_sha)?)?;
        let head_tree = commit.tree()?;

        let wd_tree_entry = head_tree
            .get_name("workdir")
            .ok_or(anyhow!("failed to get workdir tree entry"))?;
        let wd_tree = repo.find_tree(wd_tree_entry.id())?;

        let mut opts = git2::DiffOptions::new();
        opts.include_untracked(true);
        let diff = repo.diff_tree_to_workdir_with_index(Some(&wd_tree), Some(&mut opts));
        let stats = diff?.stats()?;
        Ok(stats.deletions() + stats.insertions())
    }

    fn snapshot_diff(&self, sha: String) -> Result<HashMap<PathBuf, FileDiff>> {
        let repo_path = self.path.as_path();
        let repo = git2::Repository::init(repo_path)?;

        let commit = repo.find_commit(git2::Oid::from_str(&sha)?)?;
        // Top tree
        let tree = commit.tree()?;
        let old_tree = commit.parent(0)?.tree()?;

        let wd_tree_entry = tree
            .get_name("workdir")
            .ok_or(anyhow!("failed to get workdir tree entry"))?;
        let old_wd_tree_entry = old_tree
            .get_name("workdir")
            .ok_or(anyhow!("failed to get old workdir tree entry"))?;

        // workdir tree
        let wd_tree = repo.find_tree(wd_tree_entry.id())?;
        let old_wd_tree = repo.find_tree(old_wd_tree_entry.id())?;

        // Exclude files that are larger than the limit (eg. database.sql which may never be intended to be committed)
        let files_to_exclude = get_exclude_list(&repo)?;
        // In-memory, libgit2 internal ignore rule
        repo.add_ignore_rule(&files_to_exclude)?;

        let mut diff_opts = git2::DiffOptions::new();
        diff_opts
            .recurse_untracked_dirs(true)
            .include_untracked(true)
            .show_binary(true)
            .ignore_submodules(true)
            .show_untracked_content(true);

        let diff =
            repo.diff_tree_to_tree(Some(&old_wd_tree), Some(&wd_tree), Some(&mut diff_opts))?;

        let hunks = hunks_by_filepath(None, &diff)?;
        Ok(hunks)
    }
}

fn restore_conflicts_tree(
    snapshot_tree: &git2::Tree,
    repo: &git2::Repository,
    repo_path: &std::path::Path,
) -> Result<()> {
    let conflicts_tree_entry = snapshot_tree
        .get_name("conflicts")
        .ok_or(anyhow!("failed to get conflicts tree entry"))?;
    let tree = repo.find_tree(conflicts_tree_entry.id())?;

    let base_merge_parent_blob = tree.get_name("base_merge_parent");
    let path = repo_path.join(".git").join("base_merge_parent");
    if let Some(base_merge_parent_blob) = base_merge_parent_blob {
        let base_merge_parent_blob = base_merge_parent_blob
            .to_object(repo)?
            .into_blob()
            .map_err(|_| anyhow!("failed to convert base_merge_parent tree entry to blob"))?;
        fs::write(path, base_merge_parent_blob.content())?;
    } else if path.exists() {
        fs::remove_file(path)?;
    }

    let conflicts_blob = tree.get_name("conflicts");
    let path = repo_path.join(".git").join("conflicts");
    if let Some(conflicts_blob) = conflicts_blob {
        let conflicts_blob = conflicts_blob
            .to_object(repo)?
            .into_blob()
            .map_err(|_| anyhow!("failed to convert conflicts tree entry to blob"))?;
        fs::write(path, conflicts_blob.content())?;
    } else if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn write_conflicts_tree(repo_path: &std::path::Path, repo: &git2::Repository) -> Result<git2::Oid> {
    let merge_parent_path = repo_path.join(".git").join("base_merge_parent");
    let merge_parent_blob = if merge_parent_path.exists() {
        let merge_parent_content = fs::read(merge_parent_path)?;
        Some(repo.blob(&merge_parent_content)?)
    } else {
        None
    };
    let conflicts_path = repo_path.join(".git").join("conflicts");
    let conflicts_blob = if conflicts_path.exists() {
        let conflicts_content = fs::read(conflicts_path)?;
        Some(repo.blob(&conflicts_content)?)
    } else {
        None
    };
    let mut tree_builder = repo.treebuilder(None)?;
    if merge_parent_blob.is_some() {
        tree_builder.insert(
            "base_merge_parent",
            merge_parent_blob.unwrap(),
            FileMode::Blob.into(),
        )?;
    }
    if conflicts_blob.is_some() {
        tree_builder.insert("conflicts", conflicts_blob.unwrap(), FileMode::Blob.into())?;
    }
    let conflicts_tree = tree_builder.write()?;
    Ok(conflicts_tree)
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
                if metadata.is_file()
                    && metadata.len() > SNAPSHOT_FILE_LIMIT_BYTES
                    && entry.status().is_wt_new()
                {
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

        let vb_state = project.virtual_branches();

        let target_sha = initial_commit.to_string();
        let default_target = crate::virtual_branches::target::Target {
            branch: crate::git::RemoteRefname::new("origin", "main"),
            remote_url: Default::default(),
            sha: crate::git::Oid::from_str(&target_sha).unwrap(),
            push_remote_name: None,
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

        // Create conflict state
        let conflicts_path = dir.path().join(".git").join("conflicts");
        std::fs::write(&conflicts_path, "conflict A").unwrap();
        let base_merge_parent_path = dir.path().join(".git").join("base_merge_parent");
        std::fs::write(&base_merge_parent_path, "parent A").unwrap();

        // create a snapshot
        project
            .create_snapshot(SnapshotDetails::new(OperationType::CreateCommit))
            .unwrap();

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

        // remove remove conflict files
        std::fs::remove_file(&conflicts_path).unwrap();
        std::fs::remove_file(&base_merge_parent_path).unwrap();
        // New snapshot with the conflicts removed
        let conflicts_removed_snapshot = project
            .create_snapshot(SnapshotDetails::new(OperationType::UpdateWorkspaceBase))
            .unwrap();

        let initial_snapshot = &project.list_snapshots(10, None).unwrap()[1];
        assert_eq!(
            initial_snapshot.files_changed,
            vec![
                PathBuf::from_str("1.txt").unwrap(),
                PathBuf::from_str("2.txt").unwrap(),
                PathBuf::from_str("uncommitted.txt").unwrap()
            ]
        );
        assert_eq!(initial_snapshot.lines_added, 3);
        assert_eq!(initial_snapshot.lines_removed, 0);
        let second_snapshot = &project.list_snapshots(10, None).unwrap()[0];
        assert_eq!(
            second_snapshot.files_changed,
            vec![
                PathBuf::from_str("1.txt").unwrap(),
                PathBuf::from_str("2.txt").unwrap(),
                PathBuf::from_str("3.txt").unwrap(),
                PathBuf::from_str("uncommitted.txt").unwrap()
            ]
        );
        assert_eq!(second_snapshot.lines_added, 3);
        assert_eq!(second_snapshot.lines_removed, 3);

        // restore from the initial snapshot
        project
            .restore_snapshot(initial_snapshot.id.clone())
            .unwrap();

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

        // The conflict files are restored
        let file_lines = std::fs::read_to_string(&conflicts_path).unwrap();
        assert_eq!(file_lines, "conflict A");
        let file_lines = std::fs::read_to_string(&base_merge_parent_path).unwrap();
        assert_eq!(file_lines, "parent A");

        // Restore from the second snapshot
        project
            .restore_snapshot(conflicts_removed_snapshot.unwrap())
            .unwrap();

        // The conflicts are not present
        assert!(!&conflicts_path.exists());
        assert!(!&base_merge_parent_path.exists());
    }
}
