use std::{
    collections::{HashMap, hash_map::Entry},
    fs,
    str::{FromStr, from_utf8},
};

use anyhow::{Context as _, Result, anyhow, bail};
use but_core::{RepositoryExt, TreeChange, WORKSPACE_REF_NAME, diff::tree_changes};
use but_ctx::{
    Context,
    access::{RepoExclusive, RepoShared},
};
use but_meta::virtual_branches_legacy_types::VirtualBranches;
use but_oxidize::{ObjectIdExt as _, OidExt};
use gitbutler_cherry_pick::GixRepositoryExt as _;
use gitbutler_repo::{
    SignaturePurpose, commit_ids_excluding_reachable_from_with_graph, commit_without_signature_gix,
    signature_gix,
};
use gix::objs::Write as _;
use gix::{
    ObjectId,
    bstr::{BString, ByteSlice, ByteVec},
    object::tree::EntryKind,
};
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
    fn prepare_snapshot(&self, perm: &RepoShared) -> Result<gix::ObjectId>;

    /// Commits the snapshot tree that is created with the [`prepare_snapshot`](Self::prepare_snapshot) method,
    /// which yielded the `snapshot_tree_id` for the entire snapshot state.
    /// Use `details` to provide metadata about the snapshot.
    ///
    /// Committing it makes the snapshot discoverable in [`snapshots_iter`](Self::snapshots_iter) as well as
    /// restorable with [`restore_snapshot`](Self::restore_snapshot).
    ///
    /// Returns `Some(snapshot_commit_id)` if it was created or `None` if nothing changed between the previous oplog
    /// commit and the current one (after comparing trees).
    fn commit_snapshot(
        &self,
        snapshot_tree_id: gix::ObjectId,
        details: SnapshotDetails,
        perm: &mut RepoExclusive,
    ) -> Result<gix::ObjectId>;

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
        perm: &mut RepoExclusive,
    ) -> Result<gix::ObjectId>;

    /// Returns an iterator over snapshots, with the most recent snapshot first.
    ///
    /// Use `oplog_commit_id` if the traversal root for snapshot discovery should be the specified
    /// commit, which is usually obtained from a previous iteration. The iterator starts after the
    /// provided `oplog_commit_id`, making it useful as a pagination cursor.
    ///
    /// An alternative way of retrieving the snapshots would be to manually inspect the oplog head
    /// using `git log <oplog_head>` available in `.git/gitbutler/operations-log.toml`.
    ///
    /// If there are no snapshots, an empty iterator is returned.
    fn snapshots_iter(
        &self,
        oplog_commit_id: Option<gix::ObjectId>,
        exclude_kind: Vec<OperationKind>,
        include_kind: Option<Vec<OperationKind>>,
    ) -> Result<impl Iterator<Item = Result<Snapshot>>>;

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
        snapshot_commit_id: gix::ObjectId,
        restore_kind: RestoreKind,
        guard: &mut RepoExclusive,
    ) -> Result<gix::ObjectId>;

    /// Returns the diff showing what this snapshot's operation changed.
    ///
    /// When `child_id` is provided, it is used as the "after" state directly,
    /// avoiding an O(n) walk from the oplog head to find it.
    fn snapshot_diff(
        &self,
        sha: gix::ObjectId,
        child_id: Option<gix::ObjectId>,
    ) -> Result<Vec<TreeChange>>;

    /// Gets a specific snapshot by its commit sha.
    fn get_snapshot(&self, sha: gix::ObjectId) -> Result<Snapshot>;

    /// Gets the sha of the last snapshot commit if present.
    fn oplog_head(&self) -> Result<Option<gix::ObjectId>>;
}

impl OplogExt for Context {
    fn prepare_snapshot(&self, perm: &RepoShared) -> Result<gix::ObjectId> {
        prepare_snapshot(self, perm)
    }

    fn commit_snapshot(
        &self,
        snapshot_tree_id: gix::ObjectId,
        details: SnapshotDetails,
        perm: &mut RepoExclusive,
    ) -> Result<gix::ObjectId> {
        let target = self.project_meta()?.target_commit_id_or_err()?;
        let repo = self.repo.get()?;
        commit_snapshot(self, &repo, snapshot_tree_id, details, perm, target)
    }

    #[instrument(skip(self, details, perm), err(Debug))]
    fn create_snapshot(
        &self,
        details: SnapshotDetails,
        perm: &mut RepoExclusive,
    ) -> Result<gix::ObjectId> {
        let PreparedSnapshot {
            tree_id,
            target_base_oid,
        } = prepare_snapshot_with_target(self, perm.read_permission())?;
        let repo = self.repo.get()?;
        commit_snapshot(self, &repo, tree_id, details, perm, target_base_oid)
    }

    #[instrument(skip(self), err(Debug))]
    fn get_snapshot(&self, sha: gix::ObjectId) -> Result<Snapshot> {
        let repo = self.repo.get()?;
        let commit = repo.find_commit(sha)?;
        let details = commit
            .message_raw()?
            .to_str()
            .ok()
            .and_then(|msg| SnapshotDetails::from_str(msg).ok())
            .ok_or(anyhow!("Commit is not a snapshot"))?;

        let snapshot = Snapshot {
            commit_id: sha,
            created_at: commit.time()?,
            details: Some(details),
        };
        Ok(snapshot)
    }

    #[instrument(skip(self), err(Debug))]
    fn snapshots_iter(
        &self,
        oplog_commit_id: Option<gix::ObjectId>,
        exclude_kind: Vec<OperationKind>,
        include_kind: Option<Vec<OperationKind>>,
    ) -> Result<impl Iterator<Item = Result<Snapshot>>> {
        let repo = self.repo.get()?.clone();
        let next_commit_id = match oplog_commit_id {
            Some(id) => Some(id),
            None => {
                let oplog_state = OplogHandle::new(&self.project_data_dir());
                oplog_state.oplog_head()?
            }
        };

        Ok(SnapshotIter {
            repo,
            next_commit_id,
            skip_initial_commit: oplog_commit_id.is_some(),
            exclude_kind,
            include_kind,
        })
    }

    fn restore_snapshot(
        &self,
        snapshot_commit_id: gix::ObjectId,
        restore_kind: RestoreKind,
        guard: &mut RepoExclusive,
    ) -> Result<gix::ObjectId> {
        // let mut guard = self.exclusive_worktree_access();
        restore_snapshot(self, snapshot_commit_id, restore_kind, guard)
    }

    fn snapshot_diff(
        &self,
        sha: gix::ObjectId,
        child_id: Option<gix::ObjectId>,
    ) -> Result<Vec<TreeChange>> {
        let repo = self.clone_repo_for_merging()?;

        // Each snapshot captures the state BEFORE its operation, so to show what
        // the operation changed we need to diff this snapshot (before) against the
        // next snapshot (after the operation ran). The next snapshot is the child
        // commit — the one whose parent is `sha`.
        let before_tree_id = tree_from_applied_vbranches(&repo, sha, self)?;

        let resolved_child = match child_id {
            Some(id) => Some(id),
            None => find_oplog_child(&repo, self, sha)?,
        };
        let after_tree_id = match resolved_child {
            Some(child_id) => tree_from_applied_vbranches(&repo, child_id, self)?,
            None => {
                // This is the oplog head (most recent snapshot). The operation has
                // completed but no subsequent snapshot exists yet, so diff against the
                // current workspace commit tree.
                let workspace_ref: &gix::refs::FullNameRef = WORKSPACE_REF_NAME.try_into()?;
                let ws_commit = repo.find_reference(workspace_ref)?.peel_to_commit()?;
                ws_commit.tree_id()?.detach()
            }
        };

        tree_changes(&repo, Some(before_tree_id), after_tree_id)
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
    fn oplog_head(&self) -> Result<Option<gix::ObjectId>> {
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
                && let Ok(tree_id) = tree_from_applied_vbranches(repo, snapshot_commit.id, ctx)
            {
                entry.insert(tree_id);
            }
            cache.get(&snapshot_commit.id).copied().ok_or_else(|| {
                anyhow!("Could not get a tree of all applied virtual branches merged")
            })
        }
        None => tree_from_applied_vbranches(repo, snapshot_commit.id, ctx),
    }
}

fn write_index_tree(ctx: &Context) -> Result<gix::ObjectId> {
    #[expect(deprecated, reason = "index materialization boundary")]
    let git2_repo = ctx.git2_repo.get()?;
    let mut index = git2_repo.index()?;
    Ok(index.write_tree()?.to_gix())
}

fn checkout_workdir_tree(ctx: &Context, workdir_tree_id: gix::ObjectId) -> Result<()> {
    #[expect(deprecated, reason = "checkout/materialization boundary")]
    let git2_repo = ctx.git2_repo.get()?;
    ignore_large_files_in_diffs(&git2_repo, AUTO_TRACK_LIMIT_BYTES)?;
    let workdir_tree = git2_repo.find_tree(workdir_tree_id.to_git2())?;
    let mut checkout_builder = git2::build::CheckoutBuilder::new();
    checkout_builder.remove_untracked(true);
    checkout_builder.force();
    git2_repo.checkout_tree(workdir_tree.as_object(), Some(&mut checkout_builder))?;
    Ok(())
}

fn ignore_large_files_in_diffs(repo: &git2::Repository, limit_in_bytes: u64) -> Result<()> {
    if limit_in_bytes == 0 {
        return Ok(());
    }
    let gix_repo = gix::open(repo.path())?;
    let worktree_dir = gix_repo
        .workdir()
        .context("All repos are expected to have a worktree")?;
    let files_to_exclude: Vec<_> = gix_repo
        .dirwalk_iter(
            gix_repo.index_or_empty()?,
            None::<BString>,
            Default::default(),
            gix_repo
                .dirwalk_options()?
                .emit_ignored(None)
                .emit_pruned(false)
                .emit_untracked(gix::dir::walk::EmissionMode::Matching),
        )?
        .filter_map(Result::ok)
        .filter_map(|item| {
            let path = worktree_dir.join(gix::path::from_bstr(item.entry.rela_path.as_bstr()));
            let file_is_too_large = path
                .metadata()
                .is_ok_and(|md| md.is_file() && md.len() > limit_in_bytes);
            file_is_too_large
                .then(|| Vec::from(item.entry.rela_path).into_string().ok())
                .flatten()
        })
        .collect();
    let ignore_list = files_to_exclude.join("\n");
    repo.add_ignore_rule(&ignore_list)?;
    Ok(())
}

fn reset_index_to_tree(ctx: &Context, tree_id: gix::ObjectId) -> Result<()> {
    #[expect(deprecated, reason = "index materialization boundary")]
    let git2_repo = ctx.git2_repo.get()?;
    let tree = git2_repo
        .find_tree(tree_id.to_git2())
        .context("failed to convert index tree entry to tree")?;
    let mut index = git2_repo.index()?;
    index.read_tree(&tree)?;
    index.write()?;
    Ok(())
}

pub fn prepare_snapshot(ctx: &Context, shared_access: &RepoShared) -> Result<gix::ObjectId> {
    prepare_snapshot_with_target(ctx, shared_access).map(|prepared| prepared.tree_id)
}

struct PreparedSnapshot {
    tree_id: gix::ObjectId,
    target_base_oid: gix::ObjectId,
}

mod legacy_virtual_branches {
    use std::path::PathBuf;

    use anyhow::Result;
    use but_ctx::Context;
    use but_meta::{
        legacy_storage,
        virtual_branches_legacy_types::{Stack, StackBranch, VirtualBranches},
    };

    pub(super) fn restore_legacy_metadata_from_toml(
        ctx: &Context,
        contents: &[u8],
    ) -> Result<but_meta::VirtualBranchesTomlMetadata> {
        let path = toml_path(ctx);
        but_utils::write(&path, contents)?;
        legacy_storage::import_toml_into_db(&path)?;
        ctx.legacy_meta()
    }

    fn toml_path(ctx: &Context) -> PathBuf {
        ctx.project_data_dir().join("virtual_branches.toml")
    }

    pub(super) fn in_workspace_stacks(
        virtual_branches: &VirtualBranches,
    ) -> impl Iterator<Item = &Stack> {
        virtual_branches
            .branches
            .values()
            .filter(|stack| stack.in_workspace)
    }

    pub(super) fn in_workspace_stacks_mut(
        virtual_branches: &mut VirtualBranches,
    ) -> impl Iterator<Item = &mut Stack> {
        virtual_branches
            .branches
            .values_mut()
            .filter(|stack| stack.in_workspace)
    }

    pub(super) fn stack_head_oid(
        stack: &Stack,
        default_target_oid: gix::ObjectId,
        repo: &gix::Repository,
    ) -> Result<gix::ObjectId> {
        if let Some(branch) = stack.heads.last() {
            branch_head_oid(branch, repo)
        } else {
            Ok(default_target_oid)
        }
    }

    pub(super) fn sync_stack_heads_from_refs(stack: &mut Stack, repo: &gix::Repository) -> bool {
        let mut changed = false;
        for head in &mut stack.heads {
            changed |= sync_branch_head_from_ref(head, repo).unwrap_or(false);
        }
        changed
    }

    fn branch_head_oid(branch: &StackBranch, repo: &gix::Repository) -> Result<gix::ObjectId> {
        if let Some(mut reference) = repo.try_find_reference(&branch.name)? {
            let commit = reference.peel_to_commit()?;
            Ok(commit.id)
        } else {
            set_reference_to_stored_head(branch, repo)?;
            Ok(branch.head)
        }
    }

    pub(super) fn set_reference_to_stored_head(
        branch: &StackBranch,
        repo: &gix::Repository,
    ) -> Result<()> {
        repo.reference(
            qualified_reference_name(&branch.name),
            branch.head,
            gix::refs::transaction::PreviousValue::Any,
            "GitButler reference",
        )?;
        Ok(())
    }

    fn sync_branch_head_from_ref(branch: &mut StackBranch, repo: &gix::Repository) -> Result<bool> {
        let oid_from_ref = branch_head_oid(branch, repo)?;
        if oid_from_ref != branch.head {
            branch.head = oid_from_ref;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn qualified_reference_name(name: &str) -> String {
        format!("refs/heads/{}", name.trim_matches('/'))
    }
}

fn prepare_snapshot_with_target(
    ctx: &Context,
    _shared_access: &RepoShared,
) -> Result<PreparedSnapshot> {
    let repo = ctx.repo.get()?;
    let empty_tree_id = repo.empty_tree().id;

    // grab the target commit
    let default_target_commit_id = ctx.project_meta()?.target_commit_id_or_err()?;
    let target_tree_id = repo
        .find_commit(default_target_commit_id)?
        .tree_id()?
        .detach();

    // Create a tree out of the conflicts state if present
    let conflicts_tree_id = write_conflicts_tree(&repo)?;

    let commit_graph_cache = repo.commit_graph_if_enabled()?;
    let mut graph = repo.revision_graph(commit_graph_cache.as_ref());

    // write out the index as a tree to store
    let index_tree_id = write_index_tree(ctx)?;

    // start building our snapshot tree
    let mut snapshot_tree = repo.empty_tree().edit()?;
    snapshot_tree.upsert("index", EntryKind::Tree, index_tree_id)?;
    snapshot_tree.upsert("target_tree", EntryKind::Tree, target_tree_id)?;
    snapshot_tree.upsert("conflicts", EntryKind::Tree, conflicts_tree_id)?;
    snapshot_tree.upsert("virtual_branches", EntryKind::Tree, empty_tree_id)?;

    let legacy_meta_path = {
        let mut legacy_meta = ctx.legacy_meta()?;
        let mut virtual_branches_changed = false;
        for stack in legacy_virtual_branches::in_workspace_stacks_mut(legacy_meta.data_mut()) {
            let stack_head =
                legacy_virtual_branches::stack_head_oid(stack, default_target_commit_id, &repo)?;
            let stack_tree = repo.find_commit(stack_head)?.tree_id()?.detach();
            let stack_id = stack.id.to_string();
            let mut stack_tree_cursor =
                snapshot_tree.cursor_at(format!("virtual_branches/{stack_id}"))?;

            // commits in virtual branches (tree and commit data)
            // calculate all the commits between branch.head and the target and codify them
            stack_tree_cursor.upsert("tree", EntryKind::Tree, stack_tree)?;

            // If the references are out of sync, now is a good time to update them
            virtual_branches_changed |=
                legacy_virtual_branches::sync_stack_heads_from_refs(stack, &repo);

            for commit_id in commit_ids_excluding_reachable_from_with_graph(
                &repo,
                stack_head,
                default_target_commit_id,
                &mut graph,
            )? {
                let commit = repo.find_commit(commit_id)?;
                let commit_tree_id = commit.tree_id()?.detach();
                let commit_data_blob_id = repo.write_blob(&commit.data)?;

                stack_tree_cursor.upsert(
                    format!("commits/{commit_id}/commit"),
                    EntryKind::Blob,
                    commit_data_blob_id,
                )?;
                stack_tree_cursor.upsert(
                    format!("commits/{commit_id}/tree"),
                    EntryKind::Tree,
                    commit_tree_id,
                )?;
            }
        }

        if virtual_branches_changed {
            legacy_meta.set_changed_to_necessitate_write();
            legacy_meta.write_unreconciled()?;
        }
        legacy_meta.path().to_owned()
    };

    // The loop above may update the legacy metadata if stored heads drifted from refs, so
    // snapshot virtual_branches.toml only after that final synchronization attempt.
    // This is relevant only for snapshot restore.
    // Create a blob out of `.git/gitbutler/virtual_branches.toml`
    let vb_content = fs::read(legacy_meta_path)?;
    let vb_blob_id = repo.write_blob(&vb_content)?;
    snapshot_tree.upsert("virtual_branches.toml", EntryKind::Blob, vb_blob_id)?;
    // Add the worktree tree
    #[expect(deprecated)]
    let worktree = repo.create_wd_tree(AUTO_TRACK_LIMIT_BYTES)?;
    snapshot_tree.upsert("worktree", EntryKind::Tree, worktree)?;

    // also add the gitbutler/workspace commit to the branches tree
    let mut head = repo.head()?;
    if head
        .referent_name()
        .is_some_and(|name| name.as_bstr() == WORKSPACE_REF_NAME)
    {
        let head_commit = head.peel_to_commit()?;
        let head_tree_id = head_commit.tree_id()?.detach();
        let head_commit_id = head_commit.id;
        let commit_data_blob = repo.write_blob(&head_commit.data)?;

        snapshot_tree.upsert(
            "virtual_branches/workspace/tree",
            EntryKind::Tree,
            head_tree_id,
        )?;
        snapshot_tree.upsert(
            format!("virtual_branches/workspace/commits/{head_commit_id}/commit"),
            EntryKind::Blob,
            commit_data_blob,
        )?;
        snapshot_tree.upsert(
            format!("virtual_branches/workspace/commits/{head_commit_id}/tree"),
            EntryKind::Tree,
            head_tree_id,
        )?;
    }

    Ok(PreparedSnapshot {
        tree_id: snapshot_tree.write()?.detach(),
        target_base_oid: default_target_commit_id,
    })
}

fn commit_snapshot(
    ctx: &Context,
    repo: &gix::Repository,
    snapshot_tree_id: gix::ObjectId,
    details: SnapshotDetails,
    _exclusive_access: &mut RepoExclusive,
    target: gix::ObjectId,
) -> Result<gix::ObjectId> {
    repo.find_tree(snapshot_tree_id)?;

    let project_data_dir = ctx.project_data_dir();
    let oplog_state = OplogHandle::new(&project_data_dir);
    let oplog_head_commit = oplog_state
        .oplog_head()?
        .and_then(|head_id| repo.find_commit(head_id).ok());

    let committer = signature_gix(SignaturePurpose::Committer);
    let author = signature_gix(SignaturePurpose::Author);
    let parents = oplog_head_commit
        .as_ref()
        .map(|head| vec![head.id])
        .unwrap_or_default();
    let snapshot_commit_id = commit_without_signature_gix(
        repo,
        None,
        author,
        committer,
        details.to_string().as_str().into(),
        snapshot_tree_id,
        &parents,
        None,
    )?;

    oplog_state.set_oplog_head(snapshot_commit_id)?;

    set_reference_to_oplog(repo.git_dir(), ReflogCommits::new(ctx, target)?)?;

    Ok(snapshot_commit_id)
}

/// The kind of restore to perform.
#[derive(Debug, Copy, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
pub enum RestoreKind {
    /// An explicit restore that restores to a specific point in the oplog.
    ///
    /// Used by `but oplog restore` among others.
    ExplicitRestoreFromSnapshot,
    /// An implicit restore that undoes the last snapshot.
    ///
    /// Its implicit in the sense that the user doesn't provide the exact snapshot to restore to.
    /// We figure that out.
    ///
    /// Used by `but undo` among others.
    RestoreFromSnapshotViaUndo,
    /// An implicit restore that redos the last undo.
    ///
    /// Its implicit in the sense that the user doesn't provide the exact snapshot to restore to.
    /// We figure that out.
    ///
    /// Used by `but undo` among others.
    RestoreFromSnapshotViaRedo,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(RestoreKind);

fn restore_snapshot(
    ctx: &Context,
    snapshot_commit_id: gix::ObjectId,
    restore_kind: RestoreKind,
    exclusive_access: &mut RepoExclusive,
) -> Result<gix::ObjectId> {
    // Use a separate repo without caching so we are sure the 'has commit' checks pick up all changes.
    let repo = ctx.repo.get()?;

    let before_restore_snapshot_result = prepare_snapshot(ctx, exclusive_access.read_permission());
    let snapshot_commit = repo.find_commit(snapshot_commit_id)?;

    // The worktree checkout below diffs from this tree, so capture it before any refs move.
    let pre_restore_head_tree_id = repo.head_tree_id_or_empty()?.detach();

    let snapshot_tree = snapshot_commit.tree()?;
    let vb_toml_entry = snapshot_tree
        .lookup_entry_by_path("virtual_branches.toml")?
        .context("failed to get virtual_branches.toml blob")?;
    // virtual_branches.toml blob
    let vb_toml_blob = repo
        .find_blob(vb_toml_entry.id())
        .context("failed to convert virtual_branches tree entry to blob")?;

    if let Err(err) = restore_conflicts_tree(&snapshot_tree, &repo) {
        tracing::warn!("failed to restore conflicts tree - ignoring: {err}")
    }

    // make sure we reconstitute any commits that were in the snapshot that are not here for some reason
    // for every entry in the virtual_branches subtree, reconsitute the commits
    let vb_tree_entry = snapshot_tree
        .lookup_entry_by_path("virtual_branches")?
        .context("failed to get virtual_branches tree entry")?;
    let vb_tree = repo
        .find_tree(vb_tree_entry.id())
        .context("failed to convert virtual_branches tree entry to tree")?;

    // walk through all the entries (branches by id)
    let workspace_ref: &gix::refs::FullNameRef = WORKSPACE_REF_NAME.try_into()?;
    // The workspace commit to repoint `gitbutler/workspace` at, applied *after* the checkout below.
    let mut restored_workspace_commit: Option<gix::ObjectId> = None;
    for branch_entry in vb_tree.iter() {
        let branch_entry = branch_entry?;
        let branch_tree = repo
            .find_tree(branch_entry.id())
            .context("failed to convert virtual_branches tree entry to tree")?;
        let branch_name = branch_entry.filename();

        let commits_tree_entry = branch_tree.lookup_entry_by_path("commits")?;
        // Empty branches (head == target) have no commits, so the snapshot
        // won't contain a `commits` subtree for them. Skip reconstitution.
        let Some(commits_tree_entry) = commits_tree_entry else {
            continue;
        };
        let commits_tree = repo
            .find_tree(commits_tree_entry.id())
            .context("failed to convert commits tree entry to tree")?;

        // walk through all the commits in the branch
        for commit_entry in commits_tree.iter() {
            let commit_entry = commit_entry?;
            // for each commit, recreate the commit from the commit data if it doesn't exist
            let commit_id = commit_entry.filename();
            // check for the oid in the repo
            let commit_oid = gix::ObjectId::from_hex(commit_id)?;
            if !repo.has_object(commit_oid) {
                // commit is not in the repo, let's build it from our data
                let new_commit_oid = deserialize_commit(commit_entry.id())?;
                if new_commit_oid != commit_oid {
                    bail!("commit id mismatch: failed to recreate a commit from its parts");
                }
            }

            // TODO: in the next iteration, this of course can't be hardcoded.
            if branch_name == "workspace" {
                restored_workspace_commit = Some(commit_oid);
            }
        }
    }

    let head = repo.head()?;
    let head_ref = head
        .referent_name()
        .context("We will not change a worktree in detached HEAD state")?;
    if head_ref != workspace_ref {
        bail!("We will not change a worktree which for some reason isn't on the workspace branch");
    }

    let gix_repo = ctx.clone_repo_for_merging()?;
    let workdir_tree_id = get_workdir_tree(None, snapshot_commit_id, &gix_repo, ctx)?;

    // Check out the snapshot's worktree while HEAD still points at the pre-restore commit: both
    // backends diff from the head tree, so the workspace ref is repointed only afterwards (below).
    // The git2 backend reads HEAD for its baseline; cv3 gets `pre_restore_head_tree_id` directly.
    if ctx.settings.feature_flags.cv3 {
        but_core::worktree::safe_checkout(
            pre_restore_head_tree_id,
            workdir_tree_id,
            &gix_repo,
            but_core::worktree::checkout::Options::default(),
        )?;
    } else {
        checkout_workdir_tree(ctx, workdir_tree_id)?;
    }

    // Worktree now matches the snapshot; repoint gitbutler/workspace at the restored commit.
    if let Some(commit_oid) = restored_workspace_commit {
        repo.reference(
            workspace_ref,
            commit_oid,
            gix::refs::transaction::PreviousValue::Any,
            "restore snapshot workspace ref",
        )?;
    }

    // Update virtual_branches.toml with the state from the snapshot
    let vb_state =
        legacy_virtual_branches::restore_legacy_metadata_from_toml(ctx, &vb_toml_blob.data)?;

    // Now that legacy metadata has been restored, update references to reflect the restored heads.
    for stack in legacy_virtual_branches::in_workspace_stacks(vb_state.data()) {
        for branch in &stack.heads {
            legacy_virtual_branches::set_reference_to_stored_head(branch, &gix_repo).ok();
        }
    }
    // The restored TOML is the source of truth for the target as well - bring the project
    // metadata in Git config back in line with it so the restore isn't partial.
    ctx.resync_project_meta_from_legacy()?;
    ctx.invalidate_workspace_cache()?;

    // reset the repo index to our index tree
    let index_tree_entry = snapshot_tree
        .lookup_entry_by_path("index")?
        .context("failed to get virtual_branches.toml blob")?;
    reset_index_to_tree(ctx, index_tree_entry.id().detach())?;

    let restored_operation = snapshot_commit
        .message_raw()?
        .to_str()
        .ok()
        .and_then(|msg| SnapshotDetails::from_str(msg).ok())
        .map(|d| d.operation)
        .unwrap_or(OperationKind::Unknown);

    // create new snapshot
    let before_restore_snapshot_tree_id = before_restore_snapshot_result?;
    let restored_date_ms = snapshot_commit.time()?.seconds * 1000;
    let operation = match restore_kind {
        RestoreKind::RestoreFromSnapshotViaUndo => OperationKind::RestoreFromSnapshotViaUndo,
        RestoreKind::RestoreFromSnapshotViaRedo => OperationKind::RestoreFromSnapshotViaRedo,
        RestoreKind::ExplicitRestoreFromSnapshot => OperationKind::RestoreFromSnapshot,
    };
    let details = SnapshotDetails {
        version: Default::default(),
        operation,
        title: operation.as_persisted_str().to_owned(),
        body: None,
        trailers: Vec::from([
            Trailer::RestoredFrom(snapshot_commit_id),
            Trailer::RestoredOperation(restored_operation),
            Trailer::RestoredDate(restored_date_ms),
        ]),
    };
    let repo = ctx.repo.get()?;
    let target = ctx.project_meta()?.target_commit_id_or_err()?;
    commit_snapshot(
        ctx,
        &repo,
        before_restore_snapshot_tree_id,
        details,
        exclusive_access,
        target,
    )
}

/// Restore the state of .git/base_merge_parent and .git/conflicts from the snapshot
/// Will remove those files if they are not present in the snapshot
fn restore_conflicts_tree(snapshot_tree: &gix::Tree, repo: &gix::Repository) -> Result<()> {
    let conflicts_tree_entry = snapshot_tree
        .lookup_entry_by_path("conflicts")?
        .context("failed to get conflicts tree entry")?;

    let conflicts_tree = repo.find_tree(conflicts_tree_entry.id())?;
    let base_merge_parent_entry = conflicts_tree.lookup_entry_by_path("base_merge_parent")?;
    let base_merge_parent_path = repo.path().join("base_merge_parent");
    if let Some(base_merge_parent_blob) = base_merge_parent_entry {
        let base_merge_parent_blob = repo
            .find_blob(base_merge_parent_blob.id())
            .context("failed to convert base_merge_parent tree entry to blob")?;
        fs::write(base_merge_parent_path, &base_merge_parent_blob.data)?;
    } else if base_merge_parent_path.exists() {
        fs::remove_file(base_merge_parent_path)?;
    }

    let conflicts_entry = conflicts_tree.lookup_entry_by_path("conflicts")?;
    let conflicts_path = repo.path().join("conflicts");
    if let Some(conflicts_entry) = conflicts_entry {
        let conflicts_blob = repo
            .find_blob(conflicts_entry.id())
            .context("failed to convert conflicts tree entry to blob")?;
        fs::write(conflicts_path, &conflicts_blob.data)?;
    } else if conflicts_path.exists() {
        fs::remove_file(conflicts_path)?;
    }
    Ok(())
}

fn write_conflicts_tree(repo: &gix::Repository) -> Result<gix::ObjectId> {
    let git_dir = repo.path();
    let merge_parent_path = git_dir.join("base_merge_parent");
    let merge_parent_blob = if merge_parent_path.exists() {
        let merge_parent_content = fs::read(merge_parent_path)?;
        Some(repo.write_blob(&merge_parent_content)?)
    } else {
        None
    };
    let conflicts_path = git_dir.join("conflicts");
    let conflicts_blob = if conflicts_path.exists() {
        let conflicts_content = fs::read(conflicts_path)?;
        Some(repo.write_blob(&conflicts_content)?)
    } else {
        None
    };
    let mut tree_builder = repo.empty_tree().edit()?;
    if let Some(merge_parent_blob) = merge_parent_blob {
        tree_builder.upsert("base_merge_parent", EntryKind::Blob, merge_parent_blob)?;
    }
    if let Some(conflicts_blob) = conflicts_blob {
        tree_builder.upsert("conflicts", EntryKind::Blob, conflicts_blob)?;
    }
    Ok(tree_builder.write()?.detach())
}

/// we get the data from the blob entry and re-create a commit object from it,
/// whose returned id should match the one we stored.
fn deserialize_commit(commit_tree_id: gix::Id) -> Result<gix::ObjectId> {
    let repo = commit_tree_id.repo;
    let commit_tree = repo
        .find_tree(commit_tree_id)
        .context("failed to convert commit tree entry to tree")?;
    let commit_blob_entry = commit_tree
        .lookup_entry_by_path("commit")?
        .context("failed to get workdir tree entry")?;
    let commit_blob = repo
        .find_blob(commit_blob_entry.id())
        .context("failed to convert commit tree entry to blob")?;
    repo.write_buf(gix::object::Kind::Commit, &commit_blob.data)
        .map_err(anyhow::Error::from_boxed)
}

/// Creates a tree that is the merge of all applied branches from a given snapshot and returns the tree id.
/// Note that `repo` must have caching setup for merges.
fn tree_from_applied_vbranches(
    repo: &gix::Repository,
    snapshot_commit_id: gix::ObjectId,
    ctx: &Context,
) -> Result<gix::ObjectId> {
    let snapshot_commit = repo.find_commit(snapshot_commit_id)?;
    let snapshot_tree = snapshot_commit.tree()?;

    // Prefer the workspace commit tree over the worktree tree.
    // The worktree tree captures the entire working directory state (including uncommitted
    // and untracked files), so diffing consecutive worktree trees shows all file changes
    // that accumulated between operations — not just what the operation itself changed.
    // The workspace commit tree only reflects committed branch state, giving accurate diffs.
    if let Some(tree) = snapshot_tree.lookup_entry_by_path("virtual_branches/workspace/tree")? {
        return Ok(tree.id().detach());
    }
    // Fall back to worktree for older snapshots that don't have a workspace tree.
    if let Some(tree) = snapshot_tree.lookup_entry_by_path("worktree")? {
        return Ok(tree.id().detach());
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

    let vbs_from_toml: VirtualBranches = toml::from_str(from_utf8(&vb_toml_blob.data)?)?;
    let default_target_oid = ctx.project_meta()?.target_commit_id_or_err()?;
    let applied_branch_trees: Vec<_> = legacy_virtual_branches::in_workspace_stacks(&vbs_from_toml)
        .map(|stack| {
            let head_oid =
                legacy_virtual_branches::stack_head_oid(stack, default_target_oid, repo)?;
            let commit = repo.find_commit(head_oid)?;
            repo.find_real_tree(&commit, Default::default())
                .map(|id| id.detach())
        })
        .collect::<Result<Vec<_>>>()?;

    let mut workdir_tree_id = target_tree_id;
    let base_tree_id = target_tree_id;
    let mut current_ours_id = target_tree_id;

    let (merge_option_fail_fast, conflict_kind) = repo.merge_options_fail_fast()?;
    for branch_id in applied_branch_trees {
        let mut merge = repo.merge_trees(
            base_tree_id,
            current_ours_id,
            branch_id,
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

    Ok(workdir_tree_id)
}

/// Walk the oplog from its head to find the child of `target_id` (the commit whose parent is `target_id`).
/// Returns `None` if `target_id` is the oplog head (no child exists yet).
fn find_oplog_child(
    repo: &gix::Repository,
    ctx: &Context,
    target_id: gix::ObjectId,
) -> Result<Option<gix::ObjectId>> {
    let oplog_state = OplogHandle::new(&ctx.project_data_dir());
    let Some(head_id) = oplog_state.oplog_head()? else {
        return Ok(None);
    };
    if head_id == target_id {
        return Ok(None);
    }

    let mut current = head_id;
    loop {
        let commit = repo.find_commit(current)?;
        let parent_id = commit.parent_ids().next().map(|id| id.detach());
        match parent_id {
            Some(pid) if pid == target_id => return Ok(Some(current)),
            Some(pid) => current = pid,
            None => return Ok(None),
        }
    }
}

struct SnapshotIter {
    repo: gix::Repository,
    next_commit_id: Option<gix::ObjectId>,
    skip_initial_commit: bool,
    exclude_kind: Vec<OperationKind>,
    include_kind: Option<Vec<OperationKind>>,
}

impl Iterator for SnapshotIter {
    type Item = Result<Snapshot>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let commit_id = self.next_commit_id.take()?;
            let commit = match self.repo.find_commit(commit_id) {
                Ok(commit) => commit,
                Err(err) => return Some(Err(err.into())),
            };
            let mut parents = commit.parent_ids();
            let (first_parent, second_parent) = (parents.next(), parents.next());
            if second_parent.is_some() {
                return None;
            }
            self.next_commit_id = first_parent.map(|id| id.detach());

            if self.skip_initial_commit {
                self.skip_initial_commit = false;
                continue;
            }

            let tree = match commit.tree() {
                Ok(tree) => tree,
                Err(err) => return Some(Err(err.into())),
            };
            let has_legacy_metadata = match tree.lookup_entry_by_path("virtual_branches.toml") {
                Ok(entry) => entry.is_some(),
                Err(err) => return Some(Err(err.into())),
            };
            if !has_legacy_metadata {
                // We reached a tree that is not a snapshot
                tracing::warn!("Commit {commit_id} didn't seem to be an oplog commit - skipping");
                continue;
            }

            let details = match commit.message_raw() {
                Ok(message) => message
                    .to_str()
                    .ok()
                    .and_then(|msg| SnapshotDetails::from_str(msg).ok()),
                Err(err) => return Some(Err(err.into())),
            };
            let commit_time = match commit.time() {
                Ok(time) => time,
                Err(err) => return Some(Err(err.into())),
            };
            if let Some(details) = &details {
                // Skip if this kind is excluded
                if self.exclude_kind.contains(&details.operation) {
                    continue;
                }
                // Skip if include filter is set and this kind is not included
                if let Some(ref include) = self.include_kind
                    && !include.contains(&details.operation)
                {
                    continue;
                }
            } else if self.include_kind.is_some() {
                // If we require specific kinds but have no details, skip
                continue;
            }

            return Some(Ok(Snapshot {
                commit_id,
                details,
                created_at: commit_time,
            }));
        }
    }
}

/// Find the final snapshot that a restore snapshot will restore from.
///
/// For example if you do a reword and then a series of undos and redos the oplog would look like this:
///
/// 9ea77ad REDO
/// 71c6be6 UNDO
/// c33acf3 REDO
/// 3a0c4d1 UNDO
/// bd1724b REWORD
///
/// and `peel_restore_snapshot` will return the snapshot for `bd1724b`.
///
/// If the given snapshot is not a restore snapshot then the same snapshot will be returned.
pub fn peel_restore_snapshot(ctx: &Context, snapshot: &Snapshot) -> Result<Option<Snapshot>> {
    let mut current = snapshot.clone();

    loop {
        let Some(details) = &current.details else {
            return Ok(None);
        };

        match details.operation {
            OperationKind::RestoreFromSnapshotViaUndo
            | OperationKind::RestoreFromSnapshotViaRedo
            | OperationKind::RestoreFromSnapshot => {}
            _ => return Ok(Some(current)),
        }

        let Some(restored_from) = details.trailers.iter().find_map(|trailer| {
            if let Trailer::RestoredFrom(commit) = trailer {
                Some(*commit)
            } else {
                None
            }
        }) else {
            return Ok(None);
        };

        current = ctx.get_snapshot(restored_from)?;
    }
}
