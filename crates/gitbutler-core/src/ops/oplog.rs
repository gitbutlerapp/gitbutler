use anyhow::anyhow;
use git2::{FileMode, Oid};
use itertools::Itertools;
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;
use std::{fs, path::PathBuf};

use anyhow::Result;

use crate::git::diff::FileDiff;
use crate::virtual_branches::Branch;
use crate::{git::diff::hunks_by_filepath, projects::Project};

use super::{
    entry::{OperationType, Snapshot, SnapshotDetails, Trailer},
    reflog::set_reference_to_oplog,
    state::OplogHandle,
};

const SNAPSHOT_FILE_LIMIT_BYTES: u64 = 32 * 1024 * 1024;

/// The Oplog trait allows for crating snapshots of the current state of the project as well as restoring to a previous snapshot.
/// Snapshots include the state of the working directory as well as all additional GitButler state (e.g virtual branches, conflict state).
/// The data is stored as git trees in the following shape:
/// .
/// ├── workdir/
/// ├── virtual_branches
/// │   └── [branch-id]
/// │       ├── commit-message.txt
/// │       └── tree (subtree)
/// │   └── [branch-id]
/// │       ├── commit-message.txt
/// │       └── tree (subtree)
/// └── virtual_branches.toml
pub trait Oplog {
    /// Prepares a snapshot of the current state of the working directory as well as GitButler data.
    /// Returns a tree sha of the snapshot. The snapshot is not discoverable until it is comitted with `commit_snapshot`
    /// If there are files that are untracked and larger than SNAPSHOT_FILE_LIMIT_BYTES, they are excluded from snapshot creation and restoring.
    fn prepare_snapshot(&self) -> Result<String>;
    /// Commits the snapshot tree that is created with the `prepare_snapshot` method.
    /// Committing it makes the snapshot discoverable in `list_snapshots` as well as restorable with `restore_snapshot`.
    /// Returns the sha of the created snapshot commit or None if snapshots are disabled.
    fn commit_snapshot(
        &self,
        snapshot_tree_sha: String,
        details: SnapshotDetails,
    ) -> Result<Option<String>>;
    /// Creates a snapshot of the current state of the working directory as well as GitButler data.
    /// This is a convinience method that combines `prepare_snapshot` and `commit_snapshot`.
    fn create_snapshot(&self, details: SnapshotDetails) -> Result<Option<String>>;
    /// Lists the snapshots that have been created for the given repository, up to the given limit.
    /// An alternative way of retrieving the snapshots would be to manually the oplog head `git log <oplog_head>` available in `.git/gitbutler/operations-log.toml`.
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
    /// Determines if a new snapshot should be created due to file changes being created since the last snapshot.
    /// The needs for the automatic snapshotting are:
    ///  - It needs to facilitate backup of work in progress code
    ///  - The snapshots should not be too frequent or small - both for UX and performance reasons
    ///  - Checking if an automatic snapshot is needed should be fast and efficient since it is called on filesystem events
    ///
    /// This implementation works as follows:
    ///  - If it's been more than 5 minutes since the last snapshot,
    ///    check the sum of added and removed lines since the last snapshot, otherwise return false.
    ///  - If the sum of added and removed lines is greater than a configured threshold, return true, otherwise return false.
    fn should_auto_snapshot(&self) -> Result<bool>;
    /// Returns the diff of the snapshot and it's parent. It only includes the workdir changes.
    ///
    /// This is useful to show what has changed in this particular snapshot
    fn snapshot_diff(&self, sha: String) -> Result<HashMap<PathBuf, FileDiff>>;
}

impl Oplog for Project {
    fn prepare_snapshot(&self) -> Result<String> {
        let repo_path = self.path.as_path();
        let repo = git2::Repository::init(repo_path)?;

        let vb_state = self.virtual_branches();

        // grab the target tree sha
        let default_target_sha = vb_state.get_default_target()?.sha;
        let default_target = repo.find_commit(default_target_sha.into())?;
        let target_tree_oid = default_target.tree_id();

        // Create a blob out of `.git/gitbutler/virtual_branches.toml`
        let vb_path = repo_path
            .join(".git")
            .join("gitbutler")
            .join("virtual_branches.toml");
        let vb_content = fs::read(vb_path)?;
        let vb_blob = repo.blob(&vb_content)?;

        // Create a tree out of the conflicts state if present
        let conflicts_tree = write_conflicts_tree(repo_path, &repo)?;

        // write out the index as a tree to store
        let mut index = repo.index()?;
        let index_tree_oid = index.write_tree()?;

        // start building our snapshot tree
        let mut tree_builder = repo.treebuilder(None)?;
        tree_builder.insert("index", index_tree_oid, FileMode::Tree.into())?;
        tree_builder.insert("target_tree", target_tree_oid, FileMode::Tree.into())?;
        tree_builder.insert("conflicts", conflicts_tree, FileMode::Tree.into())?;
        tree_builder.insert("virtual_branches.toml", vb_blob, FileMode::Blob.into())?;

        // go through all virtual branches and create a subtree for each with the tree and any commits encoded
        let mut branches_tree_builder = repo.treebuilder(None)?;
        let mut head_trees = Vec::new();

        for branch in vb_state.list_branches()? {
            if branch.applied {
                head_trees.push(branch.tree);
            }

            // commits in virtual branches (tree and commit data)
            // calculate all the commits between branch.head and the target and codify them
            let mut branch_tree_builder = repo.treebuilder(None)?;
            branch_tree_builder.insert("tree", branch.tree.into(), FileMode::Tree.into())?;

            // lets get all the commits between the branch head and the target
            let mut revwalk = repo.revwalk()?;
            revwalk.push(branch.head.into())?;
            revwalk.hide(default_target.id())?;

            let mut commits_tree_builder = repo.treebuilder(None)?;
            for commit_id in revwalk {
                let commit_id = commit_id?;
                let commit = repo.find_commit(commit_id)?;
                let commit_tree = commit.tree()?;

                let mut commit_tree_builder = repo.treebuilder(None)?;

                // get the raw commit data
                let commit_header = commit.raw_header_bytes();
                let commit_message = commit.message_raw_bytes();
                let commit_data = [commit_header, b"\n", commit_message].concat();

                // convert that data into a blob
                let commit_data_blob = repo.blob(&commit_data)?;
                commit_tree_builder.insert("commit", commit_data_blob, FileMode::Blob.into())?;

                commit_tree_builder.insert("tree", commit_tree.id(), FileMode::Tree.into())?;

                let commit_tree_id = commit_tree_builder.write()?;
                commits_tree_builder.insert(
                    &commit_id.to_string(),
                    commit_tree_id,
                    FileMode::Tree.into(),
                )?;
            }

            let commits_tree_id = commits_tree_builder.write()?;
            branch_tree_builder.insert("commits", commits_tree_id, FileMode::Tree.into())?;

            let branch_tree_id = branch_tree_builder.write()?;
            branches_tree_builder.insert(
                &branch.id.to_string(),
                branch_tree_id,
                FileMode::Tree.into(),
            )?;
        }

        // also add the gitbutler/integration commit to the branches tree
        let head = repo.head()?;
        if head.is_branch() && head.name().unwrap() == "refs/heads/gitbutler/integration" {
            let commit = head.peel_to_commit()?;
            let commit_tree = commit.tree()?;

            let mut commit_tree_builder = repo.treebuilder(None)?;

            // get the raw commit data
            let commit_header = commit.raw_header_bytes();
            let commit_message = commit.message_raw_bytes();
            let commit_data = [commit_header, b"\n", commit_message].concat();

            // convert that data into a blob
            let commit_data_blob = repo.blob(&commit_data)?;
            commit_tree_builder.insert("commit", commit_data_blob, FileMode::Blob.into())?;
            commit_tree_builder.insert("tree", commit_tree.id(), FileMode::Tree.into())?;

            let commit_tree_id = commit_tree_builder.write()?;

            // gotta make a subtree to match
            let mut commits_tree_builder = repo.treebuilder(None)?;
            commits_tree_builder.insert(
                commit.id().to_string(),
                commit_tree_id,
                FileMode::Tree.into(),
            )?;
            let commits_tree_id = commits_tree_builder.write()?;

            let mut branch_tree_builder = repo.treebuilder(None)?;
            branch_tree_builder.insert("tree", commit_tree.id(), FileMode::Tree.into())?;
            branch_tree_builder.insert("commits", commits_tree_id, FileMode::Tree.into())?;
            let branch_tree_id = branch_tree_builder.write()?;

            branches_tree_builder.insert("integration", branch_tree_id, FileMode::Tree.into())?;
        }

        let branch_tree_id = branches_tree_builder.write()?;
        tree_builder.insert("virtual_branches", branch_tree_id, FileMode::Tree.into())?;

        // merge all the branch trees together, this should be our worktree
        // TODO: when we implement sub-hunk splitting, this merge logic will need to incorporate that
        if head_trees.is_empty() {
            // if there are no applied branches, then it's just the target tree
            tree_builder.insert("workdir", target_tree_oid, FileMode::Tree.into())?;
        } else if head_trees.len() == 1 {
            // if there is just one applied branch, then it's just that branch tree
            tree_builder.insert("workdir", head_trees[0].into(), FileMode::Tree.into())?;
        } else {
            // otherwise merge one branch tree at a time with target_tree_oid as the base
            let mut workdir_tree_oid = target_tree_oid;
            let base_tree = repo.find_tree(target_tree_oid)?;
            let mut current_ours = base_tree.clone();

            let head_trees_iter = head_trees.iter();
            // iterate through all head trees
            for head_tree in head_trees_iter {
                let current_theirs = repo.find_tree(git2::Oid::from(*head_tree))?;
                let mut workdir_temp_index =
                    repo.merge_trees(&base_tree, &current_ours, &current_theirs, None)?;
                workdir_tree_oid = workdir_temp_index.write_tree_to(&repo)?;
                current_ours = current_theirs;
            }
            tree_builder.insert("workdir", workdir_tree_oid, FileMode::Tree.into())?;
        }

        // ok, write out the final oplog tree
        let tree_id = tree_builder.write()?;
        Ok(tree_id.to_string())
    }

    fn commit_snapshot(&self, tree_id: String, details: SnapshotDetails) -> Result<Option<String>> {
        let repo_path = self.path.as_path();
        let repo = git2::Repository::init(repo_path)?;

        let tree = repo.find_tree(Oid::from_str(&tree_id)?)?;

        let oplog_state = OplogHandle::new(&self.gb_dir());
        let oplog_head_commit = match oplog_state.get_oplog_head()? {
            Some(head_sha) => match repo.find_commit(git2::Oid::from_str(&head_sha)?) {
                Ok(commit) => Some(commit),
                Err(_) => None, // cant find the old one, start over
            },
            // This is the first snapshot - no parents
            None => None,
        };

        // Check if there is a difference between the tree and the parent tree, and if not, return so that we dont create noop snapshots
        if let Some(ref head_commit) = oplog_head_commit {
            let parent_tree = head_commit.tree()?;
            let diff = repo.diff_tree_to_tree(Some(&parent_tree), Some(&tree), None)?;
            if diff.deltas().count() == 0 {
                return Ok(None);
            }
        }

        // Construct a new commit
        let name = "GitButler";
        let email = "gitbutler@gitbutler.com";
        let signature = git2::Signature::now(name, email).unwrap();
        let parents = if let Some(ref oplog_head_commit) = oplog_head_commit {
            vec![oplog_head_commit]
        } else {
            vec![]
        };
        let new_commit_oid = repo.commit(
            None,
            &signature,
            &signature,
            &details.to_string(),
            &tree,
            parents.as_slice(),
        )?;

        oplog_state.set_oplog_head(new_commit_oid.to_string())?;

        let vb_state = self.virtual_branches();
        // grab the target tree sha
        let default_target_sha = vb_state.get_default_target()?.sha;

        set_reference_to_oplog(
            self,
            &default_target_sha.to_string(),
            &new_commit_oid.to_string(),
        )?;

        Ok(Some(new_commit_oid.to_string()))
    }

    fn create_snapshot(&self, details: SnapshotDetails) -> Result<Option<String>> {
        let tree_id = self.prepare_snapshot()?;
        self.commit_snapshot(tree_id, details)
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

            let details = commit
                .message()
                .and_then(|msg| SnapshotDetails::from_str(msg).ok());

            if let Ok(parent) = commit.parent(0) {
                let parent_tree = parent.tree()?;
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

                snapshots.push(Snapshot {
                    id: commit_id.to_string(),
                    details,
                    lines_added,
                    lines_removed,
                    files_changed,
                    created_at: commit.time().seconds(),
                });

                if snapshots.len() >= limit {
                    break;
                }
            } else {
                // this is the very first snapshot
                snapshots.push(Snapshot {
                    id: commit_id.to_string(),
                    details,
                    lines_added: 0,
                    lines_removed: 0,
                    files_changed: Vec::new(), // Fix: Change 0 to an empty vector
                    created_at: commit.time().seconds(),
                });
                break;
            }
        }

        Ok(snapshots)
    }

    fn restore_snapshot(&self, sha: String) -> Result<Option<String>> {
        let repo_path = self.path.as_path();
        let repo = git2::Repository::init(repo_path)?;

        // prepare snapshot
        let snapshot_tree = self.prepare_snapshot();

        let commit = repo.find_commit(git2::Oid::from_str(&sha)?)?;
        // Top tree
        let top_tree = commit.tree()?;
        let vb_tree_entry = top_tree
            .get_name("virtual_branches.toml")
            .ok_or(anyhow!("failed to get virtual_branches.toml blob"))?;
        // virtual_branches.toml blob
        let vb_blob = vb_tree_entry
            .to_object(&repo)?
            .into_blob()
            .map_err(|_| anyhow!("failed to convert virtual_branches tree entry to blob"))?;
        // Restore the state of .git/base_merge_parent and .git/conflicts from the snapshot
        // Will remove those files if they are not present in the snapshot
        _ = restore_conflicts_tree(&top_tree, &repo, repo_path);
        let wd_tree_entry = top_tree
            .get_name("workdir")
            .ok_or(anyhow!("failed to get workdir tree entry"))?;

        // make sure we reconstitute any commits that were in the snapshot that are not here for some reason
        // for every entry in the virtual_branches subtree, reconsitute the commits
        let vb_tree_entry = top_tree
            .get_name("virtual_branches")
            .ok_or(anyhow!("failed to get virtual_branches tree entry"))?;
        let vb_tree = vb_tree_entry
            .to_object(&repo)?
            .into_tree()
            .map_err(|_| anyhow!("failed to convert virtual_branches tree entry to tree"))?;

        // walk through all the entries (branches)
        let walker = vb_tree.iter();
        for branch_entry in walker {
            let branch_tree = branch_entry
                .to_object(&repo)?
                .into_tree()
                .map_err(|_| anyhow!("failed to convert virtual_branches tree entry to tree"))?;
            let branch_name = branch_entry.name();

            let commits_tree_entry = branch_tree
                .get_name("commits")
                .ok_or(anyhow!("failed to get commits tree entry"))?;
            let commits_tree = commits_tree_entry
                .to_object(&repo)?
                .into_tree()
                .map_err(|_| anyhow!("failed to convert commits tree entry to tree"))?;

            // walk through all the commits in the branch
            let commit_walker = commits_tree.iter();
            for commit_entry in commit_walker {
                // for each commit, recreate the commit from the commit data if it doesn't exist
                if let Some(commit_id) = commit_entry.name() {
                    // check for the oid in the repo
                    let commit_oid = git2::Oid::from_str(commit_id)?;
                    if repo.find_commit(commit_oid).is_err() {
                        // commit is not in the repo, let's build it from our data
                        // we get the data from the blob entry and create a commit object from it, which should match the oid of the entry
                        let commit_tree = commit_entry
                            .to_object(&repo)?
                            .into_tree()
                            .map_err(|_| anyhow!("failed to convert commit tree entry to tree"))?;
                        let commit_blob_entry = commit_tree
                            .get_name("commit")
                            .ok_or(anyhow!("failed to get workdir tree entry"))?;
                        let commit_blob = commit_blob_entry
                            .to_object(&repo)?
                            .into_blob()
                            .map_err(|_| anyhow!("failed to convert commit tree entry to blob"))?;
                        let new_commit_oid = repo
                            .odb()?
                            .write(git2::ObjectType::Commit, commit_blob.content())?;
                        if new_commit_oid != commit_oid {
                            return Err(anyhow!("commit oid mismatch"));
                        }
                    }

                    // if branch_name is 'integration', we need to create or update the gitbutler/integration branch
                    if let Some(branch_name) = branch_name {
                        if branch_name == "integration" {
                            let integration_commit = repo.find_commit(commit_oid)?;
                            // reset the branch if it's there
                            let branch =
                                repo.find_branch("gitbutler/integration", git2::BranchType::Local);
                            if let Ok(mut branch) = branch {
                                // need to detatch the head for just a minuto
                                repo.set_head_detached(commit_oid)?;
                                branch.delete()?;
                            }
                            // ok, now we set the branch to what it was and update HEAD
                            repo.branch("gitbutler/integration", &integration_commit, true)?;
                            // make sure head is gitbutler/integration
                            repo.set_head("refs/heads/gitbutler/integration")?;
                        }
                    }
                }
            }
        }

        // workdir tree
        let work_tree = repo.find_tree(wd_tree_entry.id())?;

        // Exclude files that are larger than the limit (eg. database.sql which may never be intended to be committed)
        let files_to_exclude = get_exclude_list(&repo)?;
        // In-memory, libgit2 internal ignore rule
        repo.add_ignore_rule(&files_to_exclude)?;

        // Define the checkout builder
        let mut checkout_builder = git2::build::CheckoutBuilder::new();
        checkout_builder.remove_untracked(true);
        checkout_builder.force();
        // Checkout the tree
        repo.checkout_tree(work_tree.as_object(), Some(&mut checkout_builder))?;

        // Update virtual_branches.toml with the state from the snapshot
        fs::write(
            repo_path
                .join(".git")
                .join("gitbutler")
                .join("virtual_branches.toml"),
            vb_blob.content(),
        )?;

        // reset the repo index to our index tree
        let index_tree_entry = top_tree
            .get_name("index")
            .ok_or(anyhow!("failed to get virtual_branches.toml blob"))?;
        let index_tree = index_tree_entry
            .to_object(&repo)?
            .into_tree()
            .map_err(|_| anyhow!("failed to convert index tree entry to tree"))?;
        let mut index = repo.index()?;
        index.read_tree(&index_tree)?;

        let restored_operation = commit
            .message()
            .and_then(|msg| SnapshotDetails::from_str(msg).ok())
            .map(|d| d.operation.to_string())
            .unwrap_or_default();
        let restored_date = commit.time().seconds() * 1000;

        // create new snapshot
        let details = SnapshotDetails {
            version: Default::default(),
            operation: OperationType::RestoreFromSnapshot,
            title: "Restored from snapshot".to_string(),
            body: None,
            trailers: vec![
                Trailer {
                    key: "restored_from".to_string(),
                    value: sha,
                },
                Trailer {
                    key: "restored_operation".to_string(),
                    value: restored_operation,
                },
                Trailer {
                    key: "restored_date".to_string(),
                    value: restored_date.to_string(),
                },
            ],
        };
        snapshot_tree.and_then(|snapshot_tree| self.commit_snapshot(snapshot_tree, details))
    }

    fn should_auto_snapshot(&self) -> Result<bool> {
        let oplog_state = OplogHandle::new(&self.gb_dir());
        let last_snapshot_time = oplog_state.get_modified_at()?;
        if last_snapshot_time.elapsed()? > Duration::from_secs(300) {
            let changed_lines = lines_since_snapshot(self)?;
            if changed_lines > self.snapshot_lines_threshold() {
                return Ok(true);
            }
        } else {
            return Ok(false);
        }
        Ok(false)
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

/// Returns the number of lines of code (added plus removed) since the last snapshot. Includes untracked files.
///
/// If there are no snapshots, 0 is returned.
fn lines_since_snapshot(project: &Project) -> Result<usize> {
    // This looks at the diff between the tree of the currenly selected as 'default' branch (where new changes go)
    // and that same tree in the last snapshot. For some reason, comparing workdir to the workdir subree from
    // the snapshot simply does not give us what we need here, so instead using tree to tree comparison.

    let repo_path = project.path.as_path();
    let repo = git2::Repository::init(repo_path)?;

    // Exclude files that are larger than the limit (eg. database.sql which may never be intended to be committed)
    let files_to_exclude = get_exclude_list(&repo)?;
    // In-memory, libgit2 internal ignore rule
    repo.add_ignore_rule(&files_to_exclude)?;

    let oplog_state = OplogHandle::new(&project.gb_dir());
    let head_sha = oplog_state.get_oplog_head()?;
    if head_sha.is_none() {
        return Ok(0);
    }
    let head_sha = head_sha.unwrap();

    let vb_state = project.virtual_branches();
    let binding = vb_state.list_branches()?;

    let dirty_branches: Vec<&Branch> = binding
        .iter()
        .filter(|b| b.applied)
        .filter(|b| !b.ownership.claims.is_empty())
        .collect();

    let mut lines_changed = 0;
    for branch in dirty_branches {
        lines_changed += branch_lines_since_snapshot(branch, &repo, head_sha.clone())?;
    }
    Ok(lines_changed)
}

fn branch_lines_since_snapshot(
    branch: &Branch,
    repo: &git2::Repository,
    head_sha: String,
) -> Result<usize> {
    let active_branch_tree = repo.find_tree(branch.tree.into())?;

    let commit = repo.find_commit(git2::Oid::from_str(&head_sha)?)?;
    let head_tree = commit.tree()?;
    let virtual_branches = head_tree
        .get_name("virtual_branches")
        .ok_or(anyhow!("failed to get virtual_branches tree entry"))?;
    let virtual_branches = repo.find_tree(virtual_branches.id())?;
    let old_active_branch = virtual_branches
        .get_name(branch.id.to_string().as_str())
        .ok_or(anyhow!("failed to get active branch from tree entry"))?;
    let old_active_branch = repo.find_tree(old_active_branch.id())?;
    let old_active_branch_tree = old_active_branch
        .get_name("tree")
        .ok_or(anyhow!("failed to get integration tree entry"))?;
    let old_active_branch_tree = repo.find_tree(old_active_branch_tree.id())?;

    let mut opts = git2::DiffOptions::new();
    opts.include_untracked(true);
    opts.ignore_submodules(true);

    let diff = repo.diff_tree_to_tree(
        Some(&active_branch_tree),
        Some(&old_active_branch_tree),
        Some(&mut opts),
    );
    let stats = diff?.stats()?;
    Ok(stats.deletions() + stats.insertions())
}
