use std::{
    collections::{HashMap, hash_map::Entry},
    fs,
    path::Path,
    str::{FromStr, from_utf8},
};

use anyhow::{Context as _, Result, anyhow, bail};
use but_core::{RepositoryExt, TreeChange, diff::tree_changes};
use but_ctx::{
    Context,
    access::{WorktreeReadPermission, WorktreeWritePermission},
};
use but_meta::virtual_branches_legacy_types;
use but_oxidize::{
    ObjectIdExt as _, OidExt, git2_to_gix_object_id, gix_time_to_git2, gix_to_git2_oid,
};
use git2::FileMode;
use gitbutler_cherry_pick::RepositoryExtLite;
use gitbutler_repo::{RepositoryExt as _, SignaturePurpose};
use gitbutler_stack::{VirtualBranchesHandle, VirtualBranchesState};
use gix::{ObjectId, bstr::ByteSlice, prelude::ObjectIdExt};
use tracing::instrument;

use super::{
    entry::{OperationKind, Snapshot, SnapshotDetails, Trailer},
    reflog::set_reference_to_oplog,
    state::OplogHandle,
};
use crate::{entry::Version, reflog::ReflogCommits};

/// The maximum size of files to automatically start tracking, i.e. untracked files we pick up for tree-creation.
/// **Inactive for now** while it's hard to tell if it's safe *not* to pick up everything.
const AUTO_TRACK_LIMIT_BYTES: u64 = 0;

/// The Oplog allows for crating snapshots of the current state of the project as well as restoring to a previous snapshot.
/// Snapshots include the state of the working directory as well as all additional GitButler state (e.g. virtual branches, conflict state).
/// The data is stored as git trees in the following shape:
///
/// ```text
/// .
/// ├── conflicts/…
/// ├── index/
/// ├── target_tree/…
/// ├── virtual_branches
/// │   └── [branch-id]
/// │       ├── commit-message.txt
/// │       └── tree (subtree)
/// │   └── [branch-id]
/// │       ├── commit-message.txt
/// │       └── tree (subtree)
/// ├── virtual_branches.toml
/// └── worktree/…
/// ```
pub trait OplogExt {
    fn snapshot_workspace_tree(&self, sha: gix::ObjectId) -> Result<gix::ObjectId>;
    /// Prepares a snapshot of the current state of the working directory as well as GitButler data.
    /// Returns a tree hash of the snapshot. The snapshot is not discoverable until it is committed with [`commit_snapshot`](Self::commit_snapshot())
    /// If there are files that are untracked and larger than `SNAPSHOT_FILE_LIMIT_BYTES`, they are excluded from snapshot creation and restoring.
    fn prepare_snapshot(&self, perm: &WorktreeReadPermission) -> Result<git2::Oid>;

    /// Commits the snapshot tree that is created with the [`prepare_snapshot`](Self::prepare_snapshot) method,
    /// which yielded the `snapshot_tree_id` for the entire snapshot state.
    /// Use `details` to provide metadata about the snapshot.
    ///
    /// Committing it makes the snapshot discoverable in [`list_snapshots`](Self::list_snapshots) as well as
    /// restorable with [`restore_snapshot`](Self::restore_snapshot).
    ///
    /// Returns `Some(snapshot_commit_id)` if it was created or `None` if nothing changed between the previous oplog
    /// commit and the current one (after comparing trees).
    fn commit_snapshot(
        &self,
        snapshot_tree_id: git2::Oid,
        details: SnapshotDetails,
        perm: &mut WorktreeWritePermission,
    ) -> Result<git2::Oid>;

    /// Creates a snapshot of the current state of the working directory as well as GitButler data.
    /// This is a convenience method that combines [`prepare_snapshot`](Self::prepare_snapshot) and
    /// [`commit_snapshot`](Self::commit_snapshot).
    ///
    /// Returns `Some(snapshot_commit_id)` if it was created or `None` if nothing changed between the previous oplog
    /// commit and the current one (after comparing trees).
    ///
    /// Note that errors in snapshot creation is typically ignored, so we want to learn about them.
    fn create_snapshot(
        &self,
        details: SnapshotDetails,
        perm: &mut WorktreeWritePermission,
    ) -> Result<git2::Oid>;

    /// Lists the snapshots that have been created for the given repository, up to the given limit,
    /// and with the most recent snapshot first, and at the end of the vec.
    ///
    /// Use `oplog_commit_id` if the traversal root for snapshot discovery should be the specified commit, which
    /// is usually obtained from a previous iteration. Useful along with `limit` to allow starting where the iteration
    /// left off. Note that the `oplog_commit_id` is always returned as first item in the result vec.
    ///
    /// An alternative way of retrieving the snapshots would be to manually the oplog head `git log <oplog_head>` available in `.git/gitbutler/operations-log.toml`.
    ///
    /// If there are no snapshots, an empty list is returned.
    fn list_snapshots(
        &self,
        limit: usize,
        oplog_commit_id: Option<git2::Oid>,
        exclude_kind: Vec<OperationKind>,
    ) -> Result<Vec<Snapshot>>;

    /// Reverts to a previous state of the working directory, virtual branches and commits.
    /// The provided `snapshot_commit_id` must refer to a valid snapshot commit, as returned by [`create_snapshot`](Self::create_snapshot).
    /// Upon success, a new snapshot is created representing the state right before this call.
    ///
    /// This will restore the following:
    ///  - The state of the working directory is checked out from the subtree `workdir` in the snapshot.
    ///  - The state of virtual branches is restored from the blob `virtual_branches.toml` in the snapshot.
    ///  - The state of conflicts (.git/base_merge_parent and .git/conflicts) is restored from the subtree `conflicts` in the snapshot (if not present, existing files are deleted).
    ///
    /// If there are files that are untracked and larger than `SNAPSHOT_FILE_LIMIT_BYTES`, they are excluded from snapshot creation and restoring.
    /// Returns the sha of the created revert snapshot commit or None if snapshots are disabled.
    fn restore_snapshot(
        &self,
        snapshot_commit_id: git2::Oid,
        guard: &mut WorktreeWritePermission,
    ) -> Result<git2::Oid>;

    /// Returns the diff of the snapshot and it's parent. It only includes the workdir changes.
    ///
    /// This is useful to show what has changed in this particular snapshot
    fn snapshot_diff(&self, sha: git2::Oid) -> Result<Vec<TreeChange>>;

    /// Gets a specific snapshot by its commit sha.
    fn get_snapshot(&self, sha: git2::Oid) -> Result<Snapshot>;

    /// Gets the sha of the last snapshot commit if present.
    fn oplog_head(&self) -> Result<Option<git2::Oid>>;
}

impl OplogExt for Context {
    fn prepare_snapshot(&self, perm: &WorktreeReadPermission) -> Result<git2::Oid> {
        prepare_snapshot(self, perm)
    }

    fn commit_snapshot(
        &self,
        snapshot_tree_id: git2::Oid,
        details: SnapshotDetails,
        perm: &mut WorktreeWritePermission,
    ) -> Result<git2::Oid> {
        commit_snapshot(
            &self.project_data_dir(),
            &*self.git2_repo.get()?,
            snapshot_tree_id,
            details,
            perm,
        )
    }

    #[instrument(skip(self, details, perm), err(Debug))]
    fn create_snapshot(
        &self,
        details: SnapshotDetails,
        perm: &mut WorktreeWritePermission,
    ) -> Result<git2::Oid> {
        let tree_id = prepare_snapshot(self, perm.read_permission())?;
        commit_snapshot(
            &self.project_data_dir(),
            &*self.git2_repo.get()?,
            tree_id,
            details,
            perm,
        )
    }

    #[instrument(skip(self), err(Debug))]
    fn get_snapshot(&self, sha: git2::Oid) -> Result<Snapshot> {
        let repo = self.clone_repo_for_merging()?;
        let commit = repo.find_commit(sha.to_gix())?;
        let commit_time = gix_time_to_git2(commit.time()?);
        let details = commit
            .message_raw()?
            .to_str()
            .ok()
            .and_then(|msg| SnapshotDetails::from_str(msg).ok())
            .ok_or(anyhow!("Commit is not a snapshot"))?;

        let snapshot = Snapshot {
            commit_id: sha,
            created_at: commit_time,
            details: Some(details),
        };
        Ok(snapshot)
    }

    #[instrument(skip(self), err(Debug))]
    fn list_snapshots(
        &self,
        limit: usize,
        oplog_commit_id: Option<git2::Oid>,
        exclude_kind: Vec<OperationKind>,
    ) -> Result<Vec<Snapshot>> {
        let repo = self.clone_repo_for_merging()?;
        let traversal_root_id = git2_to_gix_object_id(match oplog_commit_id {
            Some(id) => id,
            None => {
                let oplog_state = OplogHandle::new(&self.project_data_dir());
                if let Some(id) = oplog_state.oplog_head()? {
                    id
                } else {
                    return Ok(vec![]);
                }
            }
        })
        .attach(&repo);

        let mut snapshots = Vec::new();

        for commit_info in traversal_root_id.ancestors().all()? {
            if snapshots.len() == limit {
                break;
            }
            let commit_id = commit_info?.id();
            if oplog_commit_id.is_some() && commit_id == traversal_root_id {
                continue;
            }
            let commit = commit_id.object()?.into_commit();
            let mut parents = commit.parent_ids();
            let (first_parent, second_parent) = (parents.next(), parents.next());
            if second_parent.is_some() {
                break;
            }

            let tree = commit.tree()?;
            if tree
                .lookup_entry_by_path("virtual_branches.toml")?
                .is_none()
            {
                // We reached a tree that is not a snapshot
                tracing::warn!("Commit {commit_id} didn't seem to be an oplog commit - skipping");
                continue;
            }

            let commit_id = gix_to_git2_oid(commit_id);
            let details = commit
                .message_raw()?
                .to_str()
                .ok()
                .and_then(|msg| SnapshotDetails::from_str(msg).ok());
            let commit_time = gix_time_to_git2(commit.time()?);
            if let Some(details) = &details
                && exclude_kind.contains(&details.operation)
            {
                continue;
            }

            snapshots.push(Snapshot {
                commit_id,
                details,
                created_at: commit_time,
            });
            if first_parent.is_none() {
                break;
            }
        }

        Ok(snapshots)
    }

    fn restore_snapshot(
        &self,
        snapshot_commit_id: git2::Oid,
        guard: &mut WorktreeWritePermission,
    ) -> Result<git2::Oid> {
        // let mut guard = self.exclusive_worktree_access();
        restore_snapshot(self, snapshot_commit_id, guard)
    }

    fn snapshot_diff(&self, sha: git2::Oid) -> Result<Vec<TreeChange>> {
        let gix_repo = self.clone_repo_for_merging()?;
        let repo = self.git2_repo.get()?;

        let commit = repo.find_commit(sha)?;

        let wd_tree_id = tree_from_applied_vbranches(&gix_repo, commit.id(), self)?;
        let wd_tree = repo.find_tree(wd_tree_id)?;

        // Handle the case where this is the first snapshot (no parent)
        let old_wd_tree_id = if commit.parent_count() > 0 {
            Some(tree_from_applied_vbranches(&gix_repo, commit.parent(0)?.id(), self)?.to_gix())
        } else {
            // For the first snapshot, compare against empty tree
            None
        };

        tree_changes(&gix_repo, old_wd_tree_id, wd_tree.id().to_gix())
    }

    fn snapshot_workspace_tree(&self, sha: gix::ObjectId) -> Result<gix::ObjectId> {
        let repo = self.repo.get()?;
        let tree = repo.find_commit(sha)?.tree()?;
        let workspace = tree
            .find_entry("worktree")
            .context("Failed to find workspace tree in snapshot")?;
        Ok(workspace.object_id())
    }

    /// Gets the sha of the last snapshot commit if present.
    fn oplog_head(&self) -> Result<Option<git2::Oid>> {
        let oplog_state = OplogHandle::new(&self.project_data_dir());
        oplog_state.oplog_head()
    }
}

/// Get a tree of the working dir (applied branches merged)
fn get_workdir_tree(
    wd_trees_cache: Option<&mut HashMap<gix::ObjectId, gix::ObjectId>>,
    commit_id: impl Into<gix::ObjectId>,
    repo: &gix::Repository,
    ctx: &Context,
) -> Result<ObjectId, anyhow::Error> {
    let snapshot_commit = repo.find_commit(commit_id.into())?;
    let details = snapshot_commit
        .message_raw()?
        .to_str()
        .ok()
        .and_then(|msg| SnapshotDetails::from_str(msg).ok());
    // In version 3 snapshots, the worktree is stored directly in the snapshot tree
    if let Some(details) = details
        && details.version == Version(3)
    {
        let worktree_entry = snapshot_commit
            .tree()?
            .lookup_entry_by_path("worktree")?
            .context(format!(
                "no entry at 'worktree' on sha {:?}, version: {:?}",
                &snapshot_commit.id(),
                &details.version,
            ))?;
        let worktree_id = worktree_entry.id().detach();
        return Ok(worktree_id);
    }
    match wd_trees_cache {
        Some(cache) => {
            if let Entry::Vacant(entry) = cache.entry(snapshot_commit.id)
                && let Ok(tree_id) =
                    tree_from_applied_vbranches(repo, gix_to_git2_oid(snapshot_commit.id), ctx)
            {
                entry.insert(git2_to_gix_object_id(tree_id));
            }
            cache.get(&snapshot_commit.id).copied().ok_or_else(|| {
                anyhow!("Could not get a tree of all applied virtual branches merged")
            })
        }
        None => tree_from_applied_vbranches(repo, gix_to_git2_oid(snapshot_commit.id), ctx)
            .map(|x| x.to_gix()),
    }
}

pub fn prepare_snapshot(
    ctx: &Context,
    _shared_access: &WorktreeReadPermission,
) -> Result<git2::Oid> {
    let repo = ctx.git2_repo.get()?;

    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());

    // grab the target commit
    let default_target_commit = repo.find_commit(vb_state.get_default_target()?.sha)?;
    let target_tree_id = default_target_commit.tree_id();

    // Create a blob out of `.git/gitbutler/virtual_branches.toml`
    let vb_path = repo.path().join("gitbutler").join("virtual_branches.toml");
    let vb_content = fs::read(vb_path)?;
    let vb_blob_id = repo.blob(&vb_content)?;

    // Create a tree out of the conflicts state if present
    let conflicts_tree_id = write_conflicts_tree(&repo)?;

    // write out the index as a tree to store
    let mut index = repo.index()?;
    let index_tree_oid = index.write_tree()?;

    // start building our snapshot tree
    let mut tree_builder = repo.treebuilder(None)?;
    tree_builder.insert("index", index_tree_oid, FileMode::Tree.into())?;
    tree_builder.insert("target_tree", target_tree_id, FileMode::Tree.into())?;
    tree_builder.insert("conflicts", conflicts_tree_id, FileMode::Tree.into())?;
    tree_builder.insert("virtual_branches.toml", vb_blob_id, FileMode::Blob.into())?;

    // go through all virtual branches and create a subtree for each with the tree and any commits encoded
    let mut branches_tree_builder = repo.treebuilder(None)?;
    let mut head_tree_ids = Vec::new();

    let gix_repo = ctx.repo.get()?;

    for mut stack in vb_state.list_stacks_in_workspace()? {
        head_tree_ids.push(stack.tree(ctx)?);

        // commits in virtual branches (tree and commit data)
        // calculate all the commits between branch.head and the target and codify them
        let mut branch_tree_builder = repo.treebuilder(None)?;
        branch_tree_builder.insert("tree", stack.tree(ctx)?, FileMode::Tree.into())?;

        // let's get all the commits between the branch head and the target
        let mut revwalk = repo.revwalk()?;
        revwalk.push(stack.head_oid(&gix_repo)?.to_git2())?;
        revwalk.hide(default_target_commit.id())?;

        // If the references are out of sync, now is a good time to update them
        stack.sync_heads_with_references(&vb_state, &gix_repo).ok();

        let mut commits_tree_builder = repo.treebuilder(None)?;
        for commit_id in revwalk {
            let commit_id = commit_id?;
            let commit = repo.find_commit(commit_id)?;
            let commit_tree = commit.tree()?;

            let mut commit_tree_builder = repo.treebuilder(None)?;
            let commit_data_blob_id = repo.blob(&serialize_commit(&commit))?;
            commit_tree_builder.insert("commit", commit_data_blob_id, FileMode::Blob.into())?;
            commit_tree_builder.insert("tree", commit_tree.id(), FileMode::Tree.into())?;
            let commit_tree_id = commit_tree_builder.write()?;

            commits_tree_builder.insert(
                commit_id.to_string(),
                commit_tree_id,
                FileMode::Tree.into(),
            )?;
        }

        let commits_tree_id = commits_tree_builder.write()?;
        branch_tree_builder.insert("commits", commits_tree_id, FileMode::Tree.into())?;

        let branch_tree_id = branch_tree_builder.write()?;
        branches_tree_builder.insert(
            stack.id.to_string(),
            branch_tree_id,
            FileMode::Tree.into(),
        )?;
    }

    // Add the worktree tree
    let worktree = repo.create_wd_tree(AUTO_TRACK_LIMIT_BYTES)?;
    tree_builder.insert("worktree", worktree.id(), FileMode::Tree.into())?;

    // also add the gitbutler/workspace commit to the branches tree
    let head = repo.head()?;
    if head.name() == Some("refs/heads/gitbutler/workspace") {
        let head_commit = head.peel_to_commit()?;
        let head_tree = head_commit.tree()?;

        let mut head_commit_tree_builder = repo.treebuilder(None)?;

        // convert that data into a blob
        let commit_data_blob = repo.blob(&serialize_commit(&head_commit))?;
        head_commit_tree_builder.insert("commit", commit_data_blob, FileMode::Blob.into())?;
        head_commit_tree_builder.insert("tree", head_tree.id(), FileMode::Tree.into())?;

        let head_commit_tree_id = head_commit_tree_builder.write()?;

        // have to make a subtree to match
        let mut commits_tree_builder = repo.treebuilder(None)?;
        commits_tree_builder.insert(
            head_commit.id().to_string(),
            head_commit_tree_id,
            FileMode::Tree.into(),
        )?;
        let commits_tree_id = commits_tree_builder.write()?;

        let mut branch_tree_builder = repo.treebuilder(None)?;
        branch_tree_builder.insert("tree", head_tree.id(), FileMode::Tree.into())?;
        branch_tree_builder.insert("commits", commits_tree_id, FileMode::Tree.into())?;
        let branch_tree_id = branch_tree_builder.write()?;

        branches_tree_builder.insert("workspace", branch_tree_id, FileMode::Tree.into())?;
    }

    let branch_tree_id = branches_tree_builder.write()?;
    tree_builder.insert("virtual_branches", branch_tree_id, FileMode::Tree.into())?;

    let tree_id = tree_builder.write()?;
    Ok(tree_id)
}

fn commit_snapshot(
    project_data_dir: &Path,
    repo: &git2::Repository,
    snapshot_tree_id: git2::Oid,
    details: SnapshotDetails,
    _exclusive_access: &mut WorktreeWritePermission,
) -> Result<git2::Oid> {
    let snapshot_tree = repo.find_tree(snapshot_tree_id)?;

    let oplog_state = OplogHandle::new(project_data_dir);
    let oplog_head_commit = oplog_state
        .oplog_head()?
        .and_then(|head_id| repo.find_commit(head_id).ok());

    // Construct a new commit
    let committer = gitbutler_repo::signature(SignaturePurpose::Committer)?;
    let author = gitbutler_repo::signature(SignaturePurpose::Author)?;
    let parents = oplog_head_commit
        .as_ref()
        .map(|head| vec![head])
        .unwrap_or_default();
    let snapshot_commit_id = repo.commit(
        None,
        &author,
        &committer,
        &details.to_string(),
        &snapshot_tree,
        parents.as_slice(),
    )?;

    oplog_state.set_oplog_head(snapshot_commit_id)?;

    set_reference_to_oplog(repo.path(), ReflogCommits::new(project_data_dir)?)?;

    Ok(snapshot_commit_id)
}

fn restore_snapshot(
    ctx: &Context,
    snapshot_commit_id: git2::Oid,
    exclusive_access: &mut WorktreeWritePermission,
) -> Result<git2::Oid> {
    let git2_repo = ctx.git2_repo.get()?;
    // Use a separate repo without caching so we are sure the 'has commit' checks pick up all changes.
    let repo = ctx.repo.get()?;

    let before_restore_snapshot_result = prepare_snapshot(ctx, exclusive_access.read_permission());
    let snapshot_commit = git2_repo.find_commit(snapshot_commit_id)?;

    let snapshot_tree = snapshot_commit.tree()?;
    let vb_toml_entry = snapshot_tree
        .get_name("virtual_branches.toml")
        .context("failed to get virtual_branches.toml blob")?;
    // virtual_branches.toml blob
    let vb_toml_blob = git2_repo
        .find_blob(vb_toml_entry.id())
        .context("failed to convert virtual_branches tree entry to blob")?;

    if let Err(err) = restore_conflicts_tree(&snapshot_tree, &git2_repo) {
        tracing::warn!("failed to restore conflicts tree - ignoring: {err}")
    }

    // make sure we reconstitute any commits that were in the snapshot that are not here for some reason
    // for every entry in the virtual_branches subtree, reconsitute the commits
    let vb_tree_entry = snapshot_tree
        .get_name("virtual_branches")
        .context("failed to get virtual_branches tree entry")?;
    let vb_tree = git2_repo
        .find_tree(vb_tree_entry.id())
        .context("failed to convert virtual_branches tree entry to tree")?;

    // walk through all the entries (branches by id)
    let walker = vb_tree.iter();
    for branch_entry in walker {
        let branch_tree = git2_repo
            .find_tree(branch_entry.id())
            .context("failed to convert virtual_branches tree entry to tree")?;
        let branch_name = branch_entry.name();

        let commits_tree_entry = branch_tree
            .get_name("commits")
            .context("failed to get commits tree entry")?;
        let commits_tree = git2_repo
            .find_tree(commits_tree_entry.id())
            .context("failed to convert commits tree entry to tree")?;

        // walk through all the commits in the branch
        for commit_entry in commits_tree.iter() {
            // for each commit, recreate the commit from the commit data if it doesn't exist
            if let Some(commit_id) = commit_entry.name() {
                // check for the oid in the repo
                let commit_oid = git2::Oid::from_str(commit_id)?;
                if !repo.has_object(commit_oid.to_gix()) {
                    // commit is not in the repo, let's build it from our data
                    let new_commit_oid = deserialize_commit(&git2_repo, &commit_entry)?;
                    if new_commit_oid != commit_oid {
                        bail!("commit id mismatch: failed to recreate a commit from its parts");
                    }
                }

                // if branch_name is 'workspace', we need to create or update the gitbutler/workspace branch
                if branch_name == Some("workspace") {
                    // TODO(ST): with `gitoxide`, just update the branch without this dance,
                    //           similar to `git update-ref`.
                    //           Then a missing workspace branch also doesn't have to be
                    //           fatal, but we wouldn't want to `set_head()` if we are
                    //           not already on the workspace branch.
                    let mut workspace_ref = git2_repo.workspace_ref_from_head()?;

                    // reset the branch if it's there, otherwise bail as we don't meddle with other branches
                    // need to detach the head for just a moment.
                    git2_repo.set_head_detached(commit_oid)?;
                    workspace_ref.delete()?;

                    // ok, now we set the branch to what it was and update HEAD
                    let workspace_commit = git2_repo.find_commit(commit_oid)?;
                    git2_repo.branch("gitbutler/workspace", &workspace_commit, true)?;
                    // make sure head is gitbutler/workspace
                    git2_repo.set_head("refs/heads/gitbutler/workspace")?;
                }
            }
        }
    }

    git2_repo.workspace_ref_from_head().context(
        "We will not change a worktree which for some reason isn't on the workspace branch",
    )?;

    let gix_repo = ctx.clone_repo_for_merging()?;

    let workdir_tree = git2_repo.find_tree(
        get_workdir_tree(None, snapshot_commit_id.to_gix(), &gix_repo, ctx)?.to_git2(),
    )?;

    git2_repo.ignore_large_files_in_diffs(AUTO_TRACK_LIMIT_BYTES)?;

    // Define the checkout builder
    if ctx.settings().feature_flags.cv3 {
        but_core::worktree::safe_checkout_from_head(
            workdir_tree.id().to_gix(),
            &gix_repo,
            but_core::worktree::checkout::Options::default(),
        )?;
    } else {
        let mut checkout_builder = git2::build::CheckoutBuilder::new();
        checkout_builder.remove_untracked(true);
        checkout_builder.force();
        // Checkout the tree
        git2_repo.checkout_tree(workdir_tree.as_object(), Some(&mut checkout_builder))?;
    }

    // Update virtual_branches.toml with the state from the snapshot
    fs::write(
        git2_repo
            .path()
            .join("gitbutler")
            .join("virtual_branches.toml"),
        vb_toml_blob.content(),
    )?;

    // Now that the toml file has been restored, update references to reflect the the values from virtual_branches.toml
    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let stacks = vb_state.list_stacks_in_workspace()?;
    for stack in stacks {
        for branch in stack.heads {
            branch.set_reference_to_head_value(&gix_repo).ok();
        }
    }

    // reset the repo index to our index tree
    let index_tree_entry = snapshot_tree
        .get_name("index")
        .context("failed to get virtual_branches.toml blob")?;
    let index_tree = git2_repo
        .find_tree(index_tree_entry.id())
        .context("failed to convert index tree entry to tree")?;
    let mut index = git2_repo.index()?;
    index.read_tree(&index_tree)?;

    let restored_operation = snapshot_commit
        .message()
        .and_then(|msg| SnapshotDetails::from_str(msg).ok())
        .map(|d| d.operation.to_string())
        .unwrap_or_default();

    // create new snapshot
    let before_restore_snapshot_tree_id = before_restore_snapshot_result?;
    let restored_date_ms = snapshot_commit.time().seconds() * 1000;
    let details = SnapshotDetails {
        version: Default::default(),
        operation: OperationKind::RestoreFromSnapshot,
        title: "Restored from snapshot".to_string(),
        body: None,
        trailers: vec![
            Trailer {
                key: "restored_from".to_string(),
                value: snapshot_commit_id.to_string(),
            },
            Trailer {
                key: "restored_operation".to_string(),
                value: restored_operation,
            },
            Trailer {
                key: "restored_date".to_string(),
                value: restored_date_ms.to_string(),
            },
        ],
    };
    commit_snapshot(
        &ctx.project_data_dir(),
        &*ctx.git2_repo.get()?,
        before_restore_snapshot_tree_id,
        details,
        exclusive_access,
    )
}

/// Restore the state of .git/base_merge_parent and .git/conflicts from the snapshot
/// Will remove those files if they are not present in the snapshot
fn restore_conflicts_tree(snapshot_tree: &git2::Tree, repo: &git2::Repository) -> Result<()> {
    let conflicts_tree_entry = snapshot_tree
        .get_name("conflicts")
        .context("failed to get conflicts tree entry")?;

    let conflicts_tree = repo.find_tree(conflicts_tree_entry.id())?;
    let base_merge_parent_entry = conflicts_tree.get_name("base_merge_parent");
    let base_merge_parent_path = repo.path().join("base_merge_parent");
    if let Some(base_merge_parent_blob) = base_merge_parent_entry {
        let base_merge_parent_blob = repo
            .find_blob(base_merge_parent_blob.id())
            .context("failed to convert base_merge_parent tree entry to blob")?;
        fs::write(base_merge_parent_path, base_merge_parent_blob.content())?;
    } else if base_merge_parent_path.exists() {
        fs::remove_file(base_merge_parent_path)?;
    }

    let conflicts_entry = conflicts_tree.get_name("conflicts");
    let conflicts_path = repo.path().join("conflicts");
    if let Some(conflicts_entry) = conflicts_entry {
        let conflicts_blob = repo
            .find_blob(conflicts_entry.id())
            .context("failed to convert conflicts tree entry to blob")?;
        fs::write(conflicts_path, conflicts_blob.content())?;
    } else if conflicts_path.exists() {
        fs::remove_file(conflicts_path)?;
    }
    Ok(())
}

fn write_conflicts_tree(repo: &git2::Repository) -> Result<git2::Oid> {
    let git_dir = repo.path();
    let merge_parent_path = git_dir.join("base_merge_parent");
    let merge_parent_blob = if merge_parent_path.exists() {
        let merge_parent_content = fs::read(merge_parent_path)?;
        Some(repo.blob(&merge_parent_content)?)
    } else {
        None
    };
    let conflicts_path = git_dir.join("conflicts");
    let conflicts_blob = if conflicts_path.exists() {
        let conflicts_content = fs::read(conflicts_path)?;
        Some(repo.blob(&conflicts_content)?)
    } else {
        None
    };
    let mut tree_builder = repo.treebuilder(None)?;
    if let Some(merge_parent_blob) = merge_parent_blob {
        tree_builder.insert(
            "base_merge_parent",
            merge_parent_blob,
            FileMode::Blob.into(),
        )?;
    }
    if let Some(conflicts_blob) = conflicts_blob {
        tree_builder.insert("conflicts", conflicts_blob, FileMode::Blob.into())?;
    }
    let conflicts_tree = tree_builder.write()?;
    Ok(conflicts_tree)
}

fn serialize_commit(commit: &git2::Commit<'_>) -> Vec<u8> {
    let commit_header = commit.raw_header_bytes();
    let commit_message = commit.message_raw_bytes();
    [commit_header, b"\n", commit_message].concat()
}

/// we get the data from the blob entry and re-create a commit object from it,
/// whose returned id should match the one we stored.
fn deserialize_commit(
    repo: &git2::Repository,
    commit_entry: &git2::TreeEntry,
) -> Result<git2::Oid> {
    let commit_tree = repo
        .find_tree(commit_entry.id())
        .context("failed to convert commit tree entry to tree")?;
    let commit_blob_entry = commit_tree
        .get_name("commit")
        .context("failed to get workdir tree entry")?;
    let commit_blob = repo
        .find_blob(commit_blob_entry.id())
        .context("failed to convert commit tree entry to blob")?;
    let new_commit_oid = repo
        .odb()?
        .write(git2::ObjectType::Commit, commit_blob.content())?;
    Ok(new_commit_oid)
}

/// Creates a tree that is the merge of all applied branches from a given snapshot and returns the tree id.
/// Note that `repo` must have caching setup for merges.
fn tree_from_applied_vbranches(
    repo: &gix::Repository,
    snapshot_commit_id: git2::Oid,
    ctx: &Context,
) -> Result<git2::Oid> {
    let snapshot_commit = repo.find_commit(git2_to_gix_object_id(snapshot_commit_id))?;
    let snapshot_tree = snapshot_commit.tree()?;

    // If the `worktree` subtree is available, we should return that instead
    if let Some(tree) = snapshot_tree.lookup_entry_by_path("worktree")? {
        return Ok(tree.id().to_git2());
    }

    let target_tree_entry = snapshot_tree
        .lookup_entry_by_path("target_tree")?
        .context("no entry at 'target_entry'")?;
    let target_tree_id = target_tree_entry.id().detach();

    let vb_toml_entry = snapshot_tree
        .lookup_entry_by_path("virtual_branches.toml")?
        .context("failed to get virtual_branches.toml blob")?;
    let vb_toml_blob = repo
        .find_blob(vb_toml_entry.id())
        .context("failed to convert virtual_branches tree entry to blob")?;

    let vbs_from_toml: VirtualBranchesState = toml::from_str::<
        virtual_branches_legacy_types::VirtualBranches,
    >(from_utf8(&vb_toml_blob.data)?)?
    .into();
    let applied_branch_trees: Vec<_> = vbs_from_toml
        .list_stacks_in_workspace()?
        .iter()
        .flat_map(|b| b.tree(ctx))
        .collect();

    let mut workdir_tree_id = target_tree_id;
    let base_tree_id = target_tree_id;
    let mut current_ours_id = target_tree_id;

    let (merge_option_fail_fast, conflict_kind) = repo.merge_options_fail_fast()?;
    for branch_id in applied_branch_trees {
        let mut merge = repo.merge_trees(
            base_tree_id,
            current_ours_id,
            branch_id.to_gix(),
            repo.default_merge_labels(),
            merge_option_fail_fast.clone(),
        )?;
        if merge.has_unresolved_conflicts(conflict_kind) {
            tracing::warn!(
                "Failed to merge tree {branch_id} - this branch is probably applied at a time when it should not be"
            );
        } else {
            let id = merge.tree.write()?.detach();
            workdir_tree_id = id;
            current_ours_id = id;
        }
    }

    Ok(gix_to_git2_oid(workdir_tree_id))
}
