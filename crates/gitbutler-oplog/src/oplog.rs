use std::{
    collections::{BTreeSet, HashMap, hash_map::Entry},
    fs,
    str::{FromStr, from_utf8},
};

use anyhow::{Context as _, Result, anyhow, bail};
use but_core::{
    RefMetadata, RepositoryExt, TreeChange, WORKSPACE_REF_NAME, diff::tree_changes,
    ref_metadata::ProjectMeta,
};
use but_ctx::{
    Context,
    access::{RepoExclusive, RepoShared},
};
use but_meta::virtual_branches_legacy_types::VirtualBranches;
use gitbutler_cherry_pick::GixRepositoryExt as _;
use gitbutler_repo::{
    SignaturePurpose, commit_ids_excluding_reachable_from_with_graph, commit_without_signature_gix,
    signature_gix,
};
use gix::objs::Write as _;
use gix::{
    ObjectId,
    bstr::ByteSlice,
    index::entry::{Flags, Stage},
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

const PROJECT_META_FILE: &str = "project_meta.toml";

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotProjectMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    target_ref: Option<String>,
    target_commit_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    push_remote: Option<String>,
}

#[derive(serde::Deserialize)]
struct SnapshotVirtualBranches {
    #[serde(default)]
    default_target: Option<SnapshotTarget>,
    #[serde(flatten)]
    virtual_branches: VirtualBranches,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotTarget {
    branch_name: String,
    remote_name: String,
    sha: String,
    push_remote_name: Option<String>,
}

impl TryFrom<&ProjectMeta> for SnapshotProjectMeta {
    type Error = anyhow::Error;

    fn try_from(meta: &ProjectMeta) -> Result<Self> {
        let target_commit_id = meta.target_commit_id_or_err()?;
        if target_commit_id.is_null() {
            bail!("cannot snapshot a null target commit id");
        }
        Ok(Self {
            target_ref: meta.target_ref.as_ref().map(ToString::to_string),
            target_commit_id: target_commit_id.to_string(),
            push_remote: meta.push_remote.clone(),
        })
    }
}

impl TryFrom<SnapshotProjectMeta> for ProjectMeta {
    type Error = anyhow::Error;

    fn try_from(meta: SnapshotProjectMeta) -> Result<Self> {
        let target_ref = meta
            .target_ref
            .map(|name| {
                let name: gix::refs::FullName = name
                    .try_into()
                    .context("invalid targetRef in project_meta.toml")?;
                if name.category() != Some(gix::refs::Category::RemoteBranch) {
                    bail!("targetRef in project_meta.toml is not a remote-tracking branch");
                }
                Ok(name)
            })
            .transpose()?;
        let target_commit_id = gix::ObjectId::from_str(&meta.target_commit_id)
            .context("invalid targetCommitId in project_meta.toml")?;
        if target_commit_id.is_null() {
            bail!("targetCommitId in project_meta.toml is null");
        }
        Ok(Self {
            target_ref,
            target_commit_id: Some(target_commit_id),
            push_remote: meta.push_remote,
        })
    }
}

/// The Oplog allows for crating snapshots of the current state of the project as well as restoring to a previous snapshot.
/// Snapshots include the state of the working directory as well as all additional GitButler state (e.g. virtual branches, conflict state).
/// The data is stored as git trees in the following shape:
///
/// ```text
/// .
/// ├── checkout/ (ad-hoc checkouts only)
/// │   ├── commit
/// │   └── ref
/// ├── conflicts/…
/// ├── index/
/// ├── index-conflicts/…
/// ├── target_tree/…
/// ├── project_meta.toml
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
        let before_tree_id = tree_from_applied_vbranches(&repo, sha)?;

        let resolved_child = match child_id {
            Some(id) => Some(id),
            None => find_oplog_child(&repo, self, sha)?,
        };
        let after_tree_id = match resolved_child {
            Some(child_id) => tree_from_applied_vbranches(&repo, child_id)?,
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

    /// Gets the sha of the last snapshot commit if present.
    fn oplog_head(&self) -> Result<Option<gix::ObjectId>> {
        let oplog_state = OplogHandle::new(&self.project_data_dir());
        oplog_state.oplog_head()
    }
}

fn get_v3_workdir_tree(tree: gix::Tree) -> Result<Option<ObjectId>, anyhow::Error> {
    let worktree_entry = tree
        .lookup_entry_by_path("worktree")?
        .map(|entry| entry.id().detach());
    Ok(worktree_entry)
}

/// Get a tree of the working dir (applied branches merged)
fn get_workdir_tree(
    wd_trees_cache: Option<&mut HashMap<gix::ObjectId, gix::ObjectId>>,
    commit_id: impl Into<gix::ObjectId>,
    repo: &gix::Repository,
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
        let worktree_id = get_v3_workdir_tree(snapshot_commit.tree()?)?.context(format!(
            "no entry at 'worktree' on sha {:?}, version: {:?}",
            &snapshot_commit.id(),
            &details.version,
        ))?;
        return Ok(worktree_id);
    }
    match wd_trees_cache {
        Some(cache) => {
            if let Entry::Vacant(entry) = cache.entry(snapshot_commit.id)
                && let Ok(tree_id) = tree_from_applied_vbranches(repo, snapshot_commit.id)
            {
                entry.insert(tree_id);
            }
            cache.get(&snapshot_commit.id).copied().ok_or_else(|| {
                anyhow!("Could not get a tree of all applied virtual branches merged")
            })
        }
        None => tree_from_applied_vbranches(repo, snapshot_commit.id),
    }
}

struct IndexTrees {
    index: gix::ObjectId,
    conflicts: Option<gix::ObjectId>,
}

fn write_index_trees(ctx: &Context) -> Result<IndexTrees> {
    let repo = ctx.repo.get()?;
    let index = repo.index_or_empty()?;
    // The detached editor writes trees without checking that each entry's blob exists
    // locally, which it may not, e.g. for unfetched files in a partial clone with a
    // sparse checkout.
    let mut tree = repo.empty_tree().edit()?.detach();
    let mut conflicts = repo.empty_tree().edit()?.detach();
    let mut has_conflicts = false;
    for entry in index.entries() {
        let stage = entry.stage();
        let mode = entry.mode.to_tree_entry_mode().with_context(|| {
            format!(
                "index entry {} has no tree representation",
                entry.path(&index)
            )
        })?;
        if stage != Stage::Unconflicted {
            has_conflicts = true;
            let mut conflict_path = Vec::with_capacity(entry.path(&index).len() + 2);
            conflict_path.push(b'0' + stage as u8);
            conflict_path.push(b'/');
            conflict_path.extend_from_slice(entry.path(&index));
            conflicts.upsert(
                conflict_path.as_bstr().split_str("/"),
                mode.kind(),
                entry.id,
            )?;
        }
        // Unmerged entries (e.g. a conflict in an uncommitted file left by a workspace
        // update) cannot be represented in a tree. Keep the side with the local changes
        // ('ours', stage 2, absent when locally deleted); the worktree file with conflict
        // markers is captured in the snapshot's `worktree` tree.
        if !matches!(stage, Stage::Unconflicted | Stage::Ours) {
            continue;
        }
        tree.upsert(entry.path(&index).split_str("/"), mode.kind(), entry.id)?;
    }
    Ok(IndexTrees {
        index: tree.write(|tree| repo.write_object(tree).map(|id| id.detach()))?,
        conflicts: has_conflicts
            .then(|| conflicts.write(|tree| repo.write_object(tree).map(|id| id.detach())))
            .transpose()?,
    })
}

fn reset_index_to_tree(
    ctx: &Context,
    tree_id: gix::ObjectId,
    conflicts_tree_id: Option<gix::ObjectId>,
) -> Result<()> {
    let repo = ctx.repo.get()?;
    let tree = repo.find_tree(tree_id)?;
    let mut index = repo.index_from_tree(&tree.id())?;
    if let Some(conflicts_tree_id) = conflicts_tree_id {
        restore_index_conflicts(&mut index, repo.find_tree(conflicts_tree_id)?.id())?;
    }
    index.write(Default::default())?;
    // Keep the legacy libgit2 handle in sync with the index written through gix.
    #[expect(deprecated, reason = "index cache compatibility boundary")]
    ctx.git2_repo.get()?.index()?.read(true)?;
    Ok(())
}

#[expect(clippy::indexing_slicing)]
fn restore_index_conflicts(index: &mut gix::index::State, conflict_tree: gix::Id) -> Result<()> {
    let conflict_tree = conflict_tree.object()?.try_into_tree()?;
    let mut recorder = gix::traverse::tree::Recorder::default();
    conflict_tree.traverse().depthfirst(&mut recorder)?;

    let mut to_remove = BTreeSet::new();
    for record in &recorder.records {
        if record.mode.is_tree() {
            continue;
        }
        let path = &record.filepath;
        let slash = path
            .find_byte(b'/')
            .context("BUG: expecting <stage>/<path>")?;
        let stage = match &path[..slash] {
            b"1" => Stage::Base,
            b"2" => Stage::Ours,
            b"3" => Stage::Theirs,
            stage => bail!("Invalid conflict stage '{}'", stage.as_bstr()),
        };
        let path = path[slash + 1..].as_bstr();

        index.dangerously_push_entry(
            Default::default(),
            record.oid,
            Flags::from_stage(stage),
            record.mode.into(),
            path,
        );
        to_remove.insert(path);
    }
    index.remove_entries(|_idx, path, entry| {
        entry.flags.stage() == Stage::Unconflicted && to_remove.contains(path)
    });
    index.sort_entries();
    Ok(())
}

pub fn prepare_snapshot(ctx: &Context, shared_access: &RepoShared) -> Result<gix::ObjectId> {
    prepare_snapshot_with_target(ctx, shared_access).map(|prepared| prepared.tree_id)
}

struct PreparedSnapshot {
    tree_id: gix::ObjectId,
    target_base_oid: gix::ObjectId,
}

/// The branch checkout to restore from a snapshot.
///
/// Snapshots created outside the managed workspace branch store this explicitly. Managed workspace
/// snapshots derive it from their stored workspace commit instead, while legacy snapshots may have
/// neither form of checkout identity.
struct SnapshotCheckout {
    /// The local branch checked out when the snapshot was created.
    ref_name: gix::refs::FullName,
    /// The commit `ref_name` pointed to when the snapshot was created.
    commit_id: gix::ObjectId,
}

fn snapshot_checkout(
    snapshot_tree: &gix::Tree<'_>,
    repo: &gix::Repository,
) -> Result<Option<SnapshotCheckout>> {
    let Some(ref_entry) = snapshot_tree.lookup_entry_by_path("checkout/ref")? else {
        return Ok(None);
    };
    let commit_entry = snapshot_tree
        .lookup_entry_by_path("checkout/commit")?
        .context("snapshot checkout ref has no commit")?;
    let ref_blob = repo
        .find_blob(ref_entry.id())
        .context("failed to read snapshot checkout ref")?;
    let ref_name = gix::refs::FullName::try_from(ref_blob.data.as_bstr())
        .context("snapshot checkout ref is invalid")?;
    if ref_name.category() != Some(gix::refs::Category::LocalBranch) {
        bail!("snapshot checkout ref is not a local branch");
    }
    let commit_blob = repo
        .find_blob(commit_entry.id())
        .context("failed to read snapshot checkout commit")?;
    let commit_id = gix::ObjectId::from_hex(&commit_blob.data)
        .context("snapshot checkout commit is invalid")?;
    if commit_id.is_null() {
        bail!("snapshot checkout commit is null");
    }
    Ok(Some(SnapshotCheckout {
        ref_name,
        commit_id,
    }))
}

fn snapshot_metadata(
    snapshot_tree: &gix::Tree<'_>,
    repo: &gix::Repository,
) -> Result<(ProjectMeta, VirtualBranches)> {
    let vb_toml_entry = snapshot_tree
        .lookup_entry_by_path("virtual_branches.toml")?
        .context("failed to get virtual_branches.toml blob")?;
    let vb_toml_blob = repo
        .find_blob(vb_toml_entry.id())
        .context("failed to convert virtual_branches.toml tree entry to blob")?;
    let SnapshotVirtualBranches {
        default_target,
        virtual_branches,
    } = toml::from_str(
        from_utf8(&vb_toml_blob.data).context("virtual_branches.toml is not UTF-8")?,
    )
    .context("failed to parse virtual_branches.toml")?;

    let project_meta = match snapshot_tree.lookup_entry_by_path(PROJECT_META_FILE)? {
        Some(entry) => {
            let blob = repo
                .find_blob(entry.id())
                .context("failed to convert project_meta.toml tree entry to blob")?;
            let stored: SnapshotProjectMeta =
                toml::from_str(from_utf8(&blob.data).context("project_meta.toml is not UTF-8")?)
                    .context("failed to parse project_meta.toml")?;
            stored.try_into()?
        }
        None => {
            let target = default_target
                .as_ref()
                .context("snapshot has neither project_meta.toml nor a legacy default target")?;
            ProjectMeta {
                target_ref: Some(
                    format!(
                        "refs/remotes/{remote}/{branch}",
                        remote = target.remote_name,
                        branch = target.branch_name
                    )
                    .try_into()?,
                ),
                target_commit_id: Some(
                    gix::ObjectId::from_str(&target.sha)
                        .context("invalid legacy default target sha")?,
                ),
                push_remote: target.push_remote_name.clone(),
            }
        }
    };
    project_meta.target_commit_id_or_err()?;
    Ok((project_meta, virtual_branches))
}

mod legacy_virtual_branches {
    use std::path::PathBuf;

    use anyhow::{Result, bail};
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
        let Some(mut reference) = repo.try_find_reference(&branch.name)? else {
            bail!("branch '{}' no longer exists", branch.name);
        };
        Ok(reference.peel_to_commit()?.id)
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
    shared_access: &RepoShared,
) -> Result<PreparedSnapshot> {
    let repo = ctx.repo.get()?;
    let empty_tree_id = repo.empty_tree().id;
    let workspace_ref: &gix::refs::FullNameRef = WORKSPACE_REF_NAME.try_into()?;
    let workspace_ref_exists = repo.try_find_reference(workspace_ref)?.is_some();

    // Grab the target once so every representation in this snapshot agrees.
    let project_meta = ctx.project_meta()?;
    let default_target_commit_id = project_meta.target_commit_id_or_err()?;
    let target_tree_id = repo
        .find_commit(default_target_commit_id)?
        .tree_id()?
        .detach();

    // Create a tree out of the conflicts state if present
    let conflicts_tree_id = write_conflicts_tree(&repo)?;

    let commit_graph_cache = repo.commit_graph_if_enabled()?;
    let mut graph = repo.revision_graph(commit_graph_cache.as_ref());

    // write out the index as a tree to store
    let index_trees = write_index_trees(ctx)?;

    // start building our snapshot tree
    let mut snapshot_tree = repo.empty_tree().edit()?;
    snapshot_tree.upsert("index", EntryKind::Tree, index_trees.index)?;
    if let Some(conflicts) = index_trees.conflicts {
        snapshot_tree.upsert("index-conflicts", EntryKind::Tree, conflicts)?;
    }
    snapshot_tree.upsert("target_tree", EntryKind::Tree, target_tree_id)?;
    snapshot_tree.upsert("conflicts", EntryKind::Tree, conflicts_tree_id)?;
    snapshot_tree.upsert("virtual_branches", EntryKind::Tree, empty_tree_id)?;
    let project_meta_blob = repo.write_blob(toml::to_string(&SnapshotProjectMeta::try_from(
        &project_meta,
    )?)?)?;
    snapshot_tree.upsert(PROJECT_META_FILE, EntryKind::Blob, project_meta_blob)?;

    let mut head = repo.head()?;
    if let Some(head_ref) = head
        .referent_name()
        .filter(|head_ref| head_ref.as_bstr() != WORKSPACE_REF_NAME)
        .map(ToOwned::to_owned)
    {
        let head_commit = head.peel_to_commit()?.id;
        snapshot_tree.upsert(
            "checkout/ref",
            EntryKind::Blob,
            repo.write_blob(head_ref.as_bstr())?,
        )?;
        snapshot_tree.upsert(
            "checkout/commit",
            EntryKind::Blob,
            repo.write_blob(head_commit.to_string().as_bytes())?,
        )?;
    }

    let vb_content = {
        // TODO(perf): use the cached version on `ctx`, why is the cache stale?
        let ws = if workspace_ref_exists {
            ctx.workspace_from_ref_uncached(workspace_ref, shared_access)?
        } else {
            ctx.workspace_from_head_uncached(shared_access)?
        };
        // Open this read-only so there is no write-back, there should be no side-effects when
        // preparing a snapshot.
        let mut legacy_meta = but_meta::VirtualBranchesTomlMetadata::from_path_read_only(
            ctx.project_data_dir().join("virtual_branches.toml"),
        )?;
        // Overlay *projected* workspace metadata onto the legacy snapshot format.
        // We want the metadata to represent what's actually there.
        if let Some(workspace_meta) = ws.metadata_from_projection()? {
            let workspace_ref = ws
                .ref_name()
                .context("workspace metadata requires a workspace reference")?;
            let mut handle = legacy_meta.workspace(workspace_ref)?;
            *handle = workspace_meta;
            legacy_meta.set_workspace(&handle)?;
        } else {
            for stack in legacy_virtual_branches::in_workspace_stacks_mut(legacy_meta.data_mut()) {
                stack.in_workspace = false;
            }
        }
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

            // Keep the snapshot-local legacy metadata in sync with the references.
            let _ = legacy_virtual_branches::sync_stack_heads_from_refs(stack, &repo);

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

        toml::to_string(legacy_meta.data())?
    };

    let vb_blob_id = repo.write_blob(vb_content.as_bytes())?;
    snapshot_tree.upsert("virtual_branches.toml", EntryKind::Blob, vb_blob_id)?;
    // Add the worktree tree
    #[expect(deprecated)]
    let worktree = repo.create_wd_tree(AUTO_TRACK_LIMIT_BYTES)?;
    snapshot_tree.upsert("worktree", EntryKind::Tree, worktree)?;

    // Preserve the managed workspace independently of HEAD: when an ad-hoc branch is checked out,
    // restoring its checkout identity must not lose the commit needed to restore the workspace ref.
    if let Some(mut workspace_ref) = repo.try_find_reference(workspace_ref)? {
        let workspace_commit = workspace_ref.peel_to_commit()?;
        let workspace_tree_id = workspace_commit.tree_id()?.detach();
        let workspace_commit_id = workspace_commit.id;
        let commit_data_blob = repo.write_blob(&workspace_commit.data)?;

        snapshot_tree.upsert(
            "virtual_branches/workspace/tree",
            EntryKind::Tree,
            workspace_tree_id,
        )?;
        snapshot_tree.upsert(
            format!("virtual_branches/workspace/commits/{workspace_commit_id}/commit"),
            EntryKind::Blob,
            commit_data_blob,
        )?;
        snapshot_tree.upsert(
            format!("virtual_branches/workspace/commits/{workspace_commit_id}/tree"),
            EntryKind::Tree,
            workspace_tree_id,
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
    let snapshot_commit = repo.find_commit(snapshot_commit_id)?;
    let snapshot_tree = snapshot_commit.tree()?;
    // Validate both metadata formats before creating the before-restore snapshot or mutating the
    // worktree, refs, config, TOML, or database.
    let (restored_project_meta, restored_virtual_branches) =
        snapshot_metadata(&snapshot_tree, &repo)?;
    let restored_checkout = snapshot_checkout(&snapshot_tree, &repo)?;
    let restored_target = restored_project_meta.target_commit_id_or_err()?;
    let restored_vb_toml = toml::to_string(&restored_virtual_branches)?;

    let before_restore_snapshot_tree_id =
        prepare_snapshot(ctx, exclusive_access.read_permission())?;
    let before_restore_snapshot_workdir_tree_id =
        get_v3_workdir_tree(repo.find_tree(before_restore_snapshot_tree_id)?)?
            .context("Could not get workdir tree of snapshot created before the restore")?;

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
    if let Some(checkout) = restored_checkout.as_ref() {
        if !repo.has_object(checkout.commit_id) {
            bail!(
                "snapshot checkout commit {} is unavailable",
                checkout.commit_id
            );
        }
        if checkout.ref_name.as_ref() == workspace_ref
            && restored_workspace_commit != Some(checkout.commit_id)
        {
            bail!("snapshot checkout and workspace commits disagree");
        }
    }
    // Managed snapshots already identify their checkout through the workspace commit entry.
    let restored_checkout = restored_checkout.or_else(|| {
        restored_workspace_commit.map(|commit_id| SnapshotCheckout {
            ref_name: workspace_ref.to_owned(),
            commit_id,
        })
    });

    let head = repo.head()?;
    let head_ref = head
        .referent_name()
        .context("We will not change a worktree in detached HEAD state")?;
    // Snapshots with neither checkout identity nor a workspace commit retain the old guard.
    if restored_checkout.is_none() && head_ref != workspace_ref {
        bail!("cannot restore a snapshot without checkout identity outside the workspace branch");
    }

    let gix_repo = ctx.clone_repo_for_merging()?;
    let workdir_tree_id = get_workdir_tree(None, snapshot_commit_id, &gix_repo)?;

    // Check out the snapshot's worktree while HEAD still points at the pre-restore commit:
    // safe_checkout diffs from `before_restore_snapshot_workdir_tree_id`, so
    // the workspace ref is repointed only afterwards (below).
    but_core::worktree::safe_checkout_from_head(
        workdir_tree_id,
        &gix_repo,
        but_core::worktree::checkout::Options {
            // `workdir_tree_id` is the restored snapshot's full workdir tree, so it already
            // contains the uncommitted changes captured at that point. `safe_checkout_from_head`
            // otherwise re-applies the current uncommitted changes on top via a 3-way merge whose
            // base is `HEAD^{tree}`, which makes those changes collide with the identical ones
            // already in the destination. Using the pre-restore workdir tree as the merge base
            // means the current uncommitted changes equal the base and cancel out, so the restored
            // tree is checked out as-is.
            merge_base_override: Some(before_restore_snapshot_workdir_tree_id),
            ..Default::default()
        },
    )?;

    // Tracked content now matches the snapshot (untracked files outside the restored diff are
    // left in place); repoint gitbutler/workspace at the restored commit.
    match restored_workspace_commit {
        Some(commit_oid) => {
            repo.reference(
                workspace_ref,
                commit_oid,
                gix::refs::transaction::PreviousValue::Any,
                "restore snapshot workspace ref",
            )?;
        }
        // A new-format snapshot with no workspace commit records that the ref did not exist.
        None if restored_checkout.is_some() => {
            if let Some(workspace_ref) = repo.try_find_reference(workspace_ref)? {
                workspace_ref.delete()?;
            }
        }
        None => {}
    }

    // Update virtual_branches.toml with the state from the snapshot
    let vb_state = legacy_virtual_branches::restore_legacy_metadata_from_toml(
        ctx,
        restored_vb_toml.as_bytes(),
    )?;

    // Now that legacy metadata has been restored, update references to reflect the restored heads.
    for stack in legacy_virtual_branches::in_workspace_stacks(vb_state.data()) {
        for branch in &stack.heads {
            legacy_virtual_branches::set_reference_to_stored_head(branch, &gix_repo).ok();
        }
    }
    ctx.set_project_meta(restored_project_meta)?;

    // reset the repo index to our index tree
    let index_tree_entry = snapshot_tree
        .lookup_entry_by_path("index")?
        .context("failed to get index tree")?;
    let index_conflicts_tree_id = snapshot_tree
        .lookup_entry_by_path("index-conflicts")?
        .map(|entry| entry.id().detach());
    reset_index_to_tree(ctx, index_tree_entry.id().detach(), index_conflicts_tree_id)?;

    if let Some(checkout) = restored_checkout {
        if checkout.ref_name.as_ref() != workspace_ref {
            repo.reference(
                checkout.ref_name.as_ref(),
                checkout.commit_id,
                gix::refs::transaction::PreviousValue::Any,
                "restore snapshot checkout ref",
            )?;
        }
        if repo.head_name()?.as_ref() != Some(&checkout.ref_name) {
            but_core::update_head_reference(
                &repo,
                gix::refs::Target::Symbolic(checkout.ref_name),
                false,
                "restore snapshot",
                b"checkout identity".as_bstr(),
                0,
            )?;
        }
    }

    let restored_operation = snapshot_commit
        .message_raw()?
        .to_str()
        .ok()
        .and_then(|msg| SnapshotDetails::from_str(msg).ok())
        .map(|d| d.operation)
        .unwrap_or(OperationKind::Unknown);

    // create new snapshot
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
    commit_snapshot(
        ctx,
        &repo,
        before_restore_snapshot_tree_id,
        details,
        exclusive_access,
        restored_target,
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

    let (project_meta, vbs_from_toml) = snapshot_metadata(&snapshot_tree, repo)?;
    let default_target_oid = project_meta.target_commit_id_or_err()?;
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
