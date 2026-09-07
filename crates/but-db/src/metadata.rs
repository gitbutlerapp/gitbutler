use std::{
    collections::BTreeSet,
    ops::{Deref, DerefMut},
    path::Path,
};

use anyhow::{Context as _, Result, ensure};
use but_core::{
    WORKSPACE_REF_NAME, is_workspace_ref_name,
    ref_metadata::{
        Branch, RefInfo, Review, StackId, Workspace, WorkspaceCommitRelation, WorkspaceStack,
        WorkspaceStackBranch,
    },
};
use gix::{
    bstr::ByteSlice as _,
    refs::{FullName, FullNameRef},
};

use crate::{
    BranchOrderSnapshot, Connection, ConnectionMut, DbHandle, Transaction, VbStack, VbStackHead,
    VirtualBranchesSnapshot,
    connection::ConnectionMutInner,
    table::{
        branch_order::{BranchOrderHandle, BranchOrderHandleMut},
        virtual_branches::{VirtualBranchesHandle, VirtualBranchesHandleMut},
    },
};

/// An owned, consistent snapshot of reference metadata from the project database.
///
/// Reads do not keep a connection borrowed. Obtain a fresh snapshot after writing metadata.
#[derive(Clone, Debug)]
pub struct Metadata {
    workspace: Workspace,
    branches: Vec<(FullName, StackId, Branch)>,
    branch_orders: Vec<Vec<FullName>>,
}

impl Default for Metadata {
    fn default() -> Self {
        Self {
            workspace: Workspace {
                ref_info: managed_ref_info(),
                stacks: Vec::new(),
            },
            branches: Vec::new(),
            branch_orders: Vec::new(),
        }
    }
}

/// A metadata value associated with its reference name.
///
/// The value is detached from the database. Saving it resolves its reference against current rows,
/// so a workspace edit cannot leave a branch handle pointing at a stale stack.
#[derive(Debug)]
pub struct MetadataHandle<T> {
    ref_name: FullName,
    value: T,
    is_default: bool,
    stack_id: Option<StackId>,
}

impl<T> MetadataHandle<T> {
    /// Whether this value was absent when the snapshot was read.
    pub fn is_default(&self) -> bool {
        self.is_default
    }

    /// The stack containing this branch when the snapshot was read, if any.
    pub fn stack_id(&self) -> Option<StackId> {
        self.stack_id
    }
}

impl<T> AsRef<FullNameRef> for MetadataHandle<T> {
    fn as_ref(&self) -> &FullNameRef {
        self.ref_name.as_ref()
    }
}
impl<T> Deref for MetadataHandle<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}
impl<T> DerefMut for MetadataHandle<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

/// A metadata mutation isolated by a savepoint. Each operation commits on success.
///
/// On error or drop its writes roll back. When created from a transaction, the outer transaction
/// still controls persistence and notification.
pub struct MetadataMut<'db> {
    sp: rusqlite::Savepoint<'db>,
    refresh: Refresh<'db>,
}

enum Refresh<'db> {
    Database(Option<&'db Path>),
    Transaction(&'db mut bool),
}

impl DbHandle {
    /// Read reference metadata without consulting or creating a TOML file.
    pub fn meta(&self) -> Result<Metadata> {
        Metadata::read(&self.conn)
    }

    /// Start an atomic reference metadata update.
    pub fn meta_mut(&mut self) -> Result<MetadataMut<'_>> {
        Ok(MetadataMut {
            sp: self.conn.savepoint()?,
            refresh: Refresh::Database(
                (self.path != Path::new(":memory:")).then_some(self.path.as_path()),
            ),
        })
    }
}

impl Transaction<'_> {
    /// Read metadata including this transaction's pending updates.
    pub fn meta(&self) -> Result<Metadata> {
        Metadata::read(self.inner())
    }

    /// Start a metadata update inside this transaction.
    pub fn meta_mut(&mut self) -> Result<MetadataMut<'_>> {
        Ok(MetadataMut {
            sp: self
                .inner
                .as_mut()
                .expect("transaction is alive")
                .savepoint()?,
            refresh: Refresh::Transaction(&mut self.metadata_changed),
        })
    }
}

impl Connection<'_> {
    /// Read reference metadata from this connection.
    pub fn meta(&self) -> Result<Metadata> {
        Metadata::read(self.conn)
    }
}

impl ConnectionMut<'_, '_> {
    /// Read reference metadata from the current transaction or database.
    pub fn meta(&self) -> Result<Metadata> {
        self.as_ref().meta()
    }

    /// Start an atomic update on this same connection.
    pub fn meta_mut(&mut self) -> Result<MetadataMut<'_>> {
        match &mut self.inner {
            ConnectionMutInner::Database(db) => db.meta_mut(),
            ConnectionMutInner::Transaction(tx) => tx.meta_mut(),
        }
    }
}

impl Metadata {
    fn read(conn: &rusqlite::Connection) -> Result<Self> {
        // Keep VB and branch-order reads in one SQLite snapshot, composing with an outer transaction.
        let read_tx = conn
            .is_autocommit()
            .then(|| conn.unchecked_transaction())
            .transpose()?;
        let snapshot = VirtualBranchesHandle { conn }
            .get_snapshot()?
            .unwrap_or_default();
        let mut metadata = Self::default();
        for stack in stored_stacks(&snapshot) {
            let id: StackId = stack.row.id.parse().context("Invalid metadata stack id")?;
            let branches = stack
                .heads
                .iter()
                .rev()
                .filter_map(|head| {
                    full_branch_name(&head.name).map(|ref_name| WorkspaceStackBranch {
                        ref_name,
                        archived: head.archived,
                    })
                })
                .collect();
            if !stack.heads.is_empty() {
                metadata.workspace.stacks.push(WorkspaceStack {
                    id,
                    branches,
                    workspacecommit_relation: if stack.row.in_workspace {
                        WorkspaceCommitRelation::Merged
                    } else {
                        WorkspaceCommitRelation::Outside
                    },
                });
            }
            for head in stack.heads {
                let Some(name) = full_branch_name(&head.name) else {
                    continue;
                };
                metadata.branches.push((
                    name,
                    id,
                    Branch {
                        ref_info: RefInfo::default(),
                        review: Review {
                            pull_request: head
                                .pr_number
                                .map(usize::try_from)
                                .transpose()
                                .context("Invalid pull request number")?,
                            review_id: head.review_id,
                        },
                    },
                ));
            }
        }
        let order = BranchOrderHandle { conn };
        for entry in order.get_snapshot()?.entries {
            if metadata.branch_orders.iter().any(|chain| {
                chain
                    .iter()
                    .any(|name| name.as_bstr() == entry.branch_ref_name.as_str())
            }) {
                continue;
            }
            if let Some(chain) = order.order_for_reference(&entry.branch_ref_name)? {
                metadata.branch_orders.push(
                    chain
                        .into_iter()
                        .map(FullName::try_from)
                        .collect::<std::result::Result<_, _>>()?,
                );
            }
        }
        if let Some(tx) = read_tx {
            tx.commit()?;
        }
        Ok(metadata)
    }

    /// Read workspace metadata, returning a detached default when absent.
    pub fn workspace(&self, ref_name: &FullNameRef) -> Result<MetadataHandle<Workspace>> {
        let value = if is_workspace_ref_name(ref_name) {
            self.workspace.clone()
        } else {
            Workspace::default()
        };
        Ok(MetadataHandle {
            ref_name: ref_name.to_owned(),
            is_default: value.stacks.is_empty(),
            value,
            stack_id: None,
        })
    }

    /// Read branch metadata, returning a detached default when absent.
    pub fn branch(&self, ref_name: &FullNameRef) -> Result<MetadataHandle<Branch>> {
        let value = self
            .branches
            .iter()
            .find(|(name, _, _)| name.as_ref() == ref_name);
        Ok(MetadataHandle {
            ref_name: ref_name.to_owned(),
            value: value.map(|(_, _, value)| value.clone()).unwrap_or_default(),
            is_default: value.is_none(),
            stack_id: value.map(|(_, id, _)| *id),
        })
    }

    /// Read a workspace only when metadata exists for it.
    pub fn workspace_opt(
        &self,
        ref_name: &FullNameRef,
    ) -> Result<Option<MetadataHandle<Workspace>>> {
        let value = self.workspace(ref_name)?;
        Ok((!value.is_default()).then_some(value))
    }

    /// Read a branch only when metadata exists for it.
    pub fn branch_opt(&self, ref_name: &FullNameRef) -> Result<Option<MetadataHandle<Branch>>> {
        let value = self.branch(ref_name)?;
        Ok((!value.is_default()).then_some(value))
    }

    /// Enumerate persisted workspaces by name and value.
    pub fn workspaces(&self) -> impl Iterator<Item = (FullName, Workspace)> + '_ {
        (!self.workspace.stacks.is_empty())
            .then(|| {
                (
                    WORKSPACE_REF_NAME.try_into().expect("valid workspace ref"),
                    self.workspace.clone(),
                )
            })
            .into_iter()
    }

    /// Enumerate persisted branches by name and value, in stack order.
    pub fn branches(&self) -> impl Iterator<Item = (FullName, Branch)> + '_ {
        self.branches
            .iter()
            .map(|(name, _, value)| (name.clone(), value.clone()))
    }

    /// Return the stored chain containing this reference, from tip to base.
    pub fn branch_stack_order(&self, ref_name: &FullNameRef) -> Result<Option<Vec<FullName>>> {
        Ok(self
            .branch_orders
            .iter()
            .find(|chain| chain.iter().any(|name| name.as_ref() == ref_name))
            .cloned())
    }
}

impl MetadataMut<'_> {
    fn snapshot(&self) -> Result<VirtualBranchesSnapshot> {
        Ok(VirtualBranchesHandle { conn: &self.sp }
            .get_snapshot()?
            .unwrap_or_default())
    }

    fn store_snapshot(&mut self, snapshot: &VirtualBranchesSnapshot) -> Result<()> {
        VirtualBranchesHandleMut {
            sp: self.sp.savepoint()?,
        }
        .replace_snapshot(snapshot)?;
        Ok(())
    }

    fn finish(self) -> Result<()> {
        self.sp.commit()?;
        match self.refresh {
            Refresh::Database(Some(path)) => but_project_handle::write_refresh_sentinel(path),
            Refresh::Database(None) => {}
            Refresh::Transaction(changed) => *changed = true,
        }
        Ok(())
    }

    /// Restore a complete VB payload without reading or writing a legacy file.
    pub fn replace_snapshot(mut self, snapshot: &VirtualBranchesSnapshot) -> Result<()> {
        self.store_snapshot(snapshot)?;
        self.finish()
    }

    /// Restore branch ordering within this mutation's savepoint.
    pub fn replace_branch_order(mut self, snapshot: &BranchOrderSnapshot) -> Result<()> {
        snapshot.validate().map_err(anyhow::Error::msg)?;
        BranchOrderHandleMut {
            sp: self.sp.savepoint()?,
        }
        .replace_snapshot(snapshot)?;
        self.finish()
    }

    /// Save workspace grouping and branch order together, preserving unrelated fields in existing rows.
    pub fn set_workspace(mut self, value: &MetadataHandle<Workspace>) -> Result<()> {
        ensure!(
            is_workspace_ref_name(value.as_ref()),
            "This backend doesn't support saving arbitrary workspaces"
        );
        let mut snapshot = self.snapshot()?;
        let mut stored = stored_stacks(&snapshot);
        let mut seen = BTreeSet::new();
        for (position, stack) in value.stacks.iter().enumerate() {
            ensure!(
                !stack.branches.is_empty(),
                "Cannot save an empty metadata stack"
            );
            let mut names = BTreeSet::new();
            ensure!(
                stack
                    .branches
                    .iter()
                    .all(|branch| names.insert(branch.ref_name.clone())),
                "A stack cannot contain the same branch twice"
            );
            let incoming_id = stack.id.to_string();
            let mut target = stored
                .iter()
                .position(|existing| existing.row.id == incoming_id);
            if target.is_none() {
                target = stack.branches.iter().find_map(|branch| {
                    find_branch(&stored, branch.ref_name.as_ref()).map(|(idx, _)| idx)
                });
            }
            let target = match target {
                Some(idx) => idx,
                None => {
                    stored.push(StoredStack {
                        row: new_stack(stack.id, position, stack.is_in_workspace()),
                        heads: Vec::new(),
                    });
                    stored.len() - 1
                }
            };
            let target_id = stored[target].row.id.clone();
            seen.insert(target_id.clone());
            let mut heads = Vec::with_capacity(stack.branches.len());
            for branch in &stack.branches {
                let own = stored[target].heads.iter().position(|head| {
                    full_branch_name(&head.name)
                        .as_ref()
                        .is_some_and(|name| name.as_ref() == branch.ref_name.as_ref())
                });
                let source = own
                    .map(|idx| (target, idx))
                    .or_else(|| find_branch(&stored, branch.ref_name.as_ref()));
                let mut head = match source {
                    Some((stack_idx, head_idx)) => {
                        seen.insert(stored[stack_idx].row.id.clone());
                        stored[stack_idx].heads.remove(head_idx)
                    }
                    None => new_head(branch.ref_name.as_ref(), &Branch::default())?,
                };
                head.stack_id = target_id.clone();
                head.archived = branch.archived;
                heads.push(head);
            }
            heads.reverse();
            stored[target].heads = heads;
            stored[target].row.in_workspace = stack.is_in_workspace();
            // A projected stack can reuse a stored identity. Keep that row's order until
            // the incoming workspace names its stored identity explicitly.
            if target_id == incoming_id {
                stored[target].row.sort_order = i64::try_from(position)?;
            }
            let names = stack
                .branches
                .iter()
                .map(|branch| branch.ref_name.as_bstr().to_str().map(ToOwned::to_owned))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            BranchOrderHandleMut {
                sp: self.sp.savepoint()?,
            }
            .set_order(&names)?;
        }
        stored.retain(|stack| seen.contains(&stack.row.id) && !stack.heads.is_empty());
        replace_stacks(&mut snapshot, stored);
        snapshot.state.initialized = true;
        self.store_snapshot(&snapshot)?;
        self.finish()
    }

    /// Save branch review data, resolving its current stack instead of using a stale snapshot index.
    pub fn set_branch(mut self, value: &MetadataHandle<Branch>) -> Result<()> {
        let mut snapshot = self.snapshot()?;
        let mut stored = stored_stacks(&snapshot);
        match find_branch(&stored, value.as_ref()) {
            Some((stack_idx, head_idx)) => {
                let head = &mut stored[stack_idx].heads[head_idx];
                head.pr_number = value.review.pull_request.map(i64::try_from).transpose()?;
                head.review_id = value.review.review_id.clone();
            }
            None => {
                let row = new_stack(StackId::generate(), stored.len(), false);
                let mut head = new_head(value.as_ref(), value)?;
                head.stack_id = row.id.clone();
                stored.push(StoredStack {
                    row,
                    heads: vec![head],
                });
            }
        }
        replace_stacks(&mut snapshot, stored);
        snapshot.state.initialized = true;
        self.store_snapshot(&snapshot)?;
        self.finish()
    }

    /// Delete a reference's metadata and its branch-order entry atomically.
    pub fn remove(mut self, ref_name: &FullNameRef) -> Result<bool> {
        let mut snapshot = self.snapshot()?;
        let had_order = BranchOrderHandle { conn: &self.sp }
            .order_for_reference(ref_name.as_bstr().to_str()?)?
            .is_some();
        BranchOrderHandleMut {
            sp: self.sp.savepoint()?,
        }
        .remove_reference(ref_name.as_bstr().to_str()?)?;
        let removed = if is_workspace_ref_name(ref_name) {
            let existed = !snapshot.stacks.is_empty();
            snapshot = VirtualBranchesSnapshot::default();
            existed
        } else {
            let mut stored = stored_stacks(&snapshot);
            let removed = if let Some((stack_idx, head_idx)) = find_branch(&stored, ref_name) {
                stored[stack_idx].heads.remove(head_idx);
                true
            } else {
                false
            };
            stored.retain(|stack| !stack.heads.is_empty());
            replace_stacks(&mut snapshot, stored);
            removed
        };
        snapshot.state.initialized = true;
        self.store_snapshot(&snapshot)?;
        self.finish()?;
        Ok(removed || had_order)
    }

    /// Rename branch data in place and update its branch-order chain in the same savepoint.
    pub fn rename(mut self, old: &FullNameRef, new: &FullNameRef) -> Result<()> {
        if old == new {
            return Ok(());
        }
        let mut snapshot = self.snapshot()?;
        let mut stored = stored_stacks(&snapshot);
        ensure!(
            find_branch(&stored, new).is_none()
                && BranchOrderHandle { conn: &self.sp }
                    .order_for_reference(new.as_bstr().to_str()?)?
                    .is_none(),
            "Cannot rename to '{}': a branch with that name already exists",
            new.shorten()
        );
        if let Some((stack_idx, head_idx)) = find_branch(&stored, old) {
            stored[stack_idx].heads[head_idx].name = new.shorten().to_str()?.to_owned();
            replace_stacks(&mut snapshot, stored);
            self.store_snapshot(&snapshot)?;
        }
        BranchOrderHandleMut {
            sp: self.sp.savepoint()?,
        }
        .rename_reference(old.as_bstr().to_str()?, new.as_bstr().to_str()?)?;
        self.finish()
    }

    /// Replace the branch-order chains containing these references with this tip-to-base chain.
    pub fn set_branch_stack_order(mut self, branches: &[FullName]) -> Result<()> {
        let names = branches
            .iter()
            .map(|name| name.as_bstr().to_str().map(ToOwned::to_owned))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        BranchOrderHandleMut {
            sp: self.sp.savepoint()?,
        }
        .set_order(&names)?;
        self.finish()
    }

    /// Rename a reference in branch ordering without changing its managed metadata.
    pub fn rename_branch_stack_order_reference(
        mut self,
        old: &FullNameRef,
        new: &FullNameRef,
    ) -> Result<()> {
        BranchOrderHandleMut {
            sp: self.sp.savepoint()?,
        }
        .rename_reference(old.as_bstr().to_str()?, new.as_bstr().to_str()?)?;
        self.finish()
    }

    /// Remove branch-order entries whose references no longer exist.
    pub fn remove_missing_branch_stack_order_references(
        mut self,
        names: &[FullName],
    ) -> Result<()> {
        let names = names
            .iter()
            .map(|name| name.as_bstr().to_str().map(ToOwned::to_owned))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        BranchOrderHandleMut {
            sp: self.sp.savepoint()?,
        }
        .remove_missing_references(&names)?;
        self.finish()
    }
}

struct StoredStack {
    row: VbStack,
    heads: Vec<VbStackHead>,
}

fn stored_stacks(snapshot: &VirtualBranchesSnapshot) -> Vec<StoredStack> {
    let mut stacks = snapshot
        .stacks
        .iter()
        .map(|row| {
            let mut heads: Vec<_> = snapshot
                .heads
                .iter()
                .filter(|head| head.stack_id == row.id)
                .cloned()
                .collect();
            heads.sort_by_key(|head| head.position);
            StoredStack {
                row: row.clone(),
                heads,
            }
        })
        .collect::<Vec<_>>();
    stacks.sort_by(|a, b| {
        a.row
            .sort_order
            .cmp(&b.row.sort_order)
            .then_with(|| {
                a.heads
                    .last()
                    .map(|head| &head.name)
                    .cmp(&b.heads.last().map(|head| &head.name))
            })
            .then_with(|| a.row.id.cmp(&b.row.id))
    });
    stacks
}

fn find_branch(stacks: &[StoredStack], name: &FullNameRef) -> Option<(usize, usize)> {
    stacks.iter().enumerate().find_map(|(stack_idx, stack)| {
        stack
            .heads
            .iter()
            .position(|head| {
                full_branch_name(&head.name)
                    .as_ref()
                    .is_some_and(|full| full.as_ref() == name)
            })
            .map(|head_idx| (stack_idx, head_idx))
    })
}

fn replace_stacks(snapshot: &mut VirtualBranchesSnapshot, stacks: Vec<StoredStack>) {
    snapshot.stacks.clear();
    snapshot.heads.clear();
    for stack in stacks {
        for (position, mut head) in stack.heads.into_iter().enumerate() {
            head.position = position as i64;
            snapshot.heads.push(head);
        }
        snapshot.stacks.push(stack.row);
    }
}

fn new_stack(id: StackId, position: usize, in_workspace: bool) -> VbStack {
    VbStack {
        id: id.to_string(),
        source_refname: None,
        upstream_remote_name: None,
        upstream_branch_name: None,
        sort_order: position as i64,
        in_workspace,
        legacy_name: String::new(),
        legacy_notes: String::new(),
        legacy_ownership: String::new(),
        legacy_allow_rebasing: true,
        legacy_post_commits: false,
        legacy_tree_sha: gix::ObjectId::null(gix::hash::Kind::Sha1).to_string(),
        legacy_head_sha: gix::ObjectId::null(gix::hash::Kind::Sha1).to_string(),
        legacy_created_timestamp_ms: "0".into(),
        legacy_updated_timestamp_ms: "0".into(),
    }
}

fn new_head(name: &FullNameRef, value: &Branch) -> Result<VbStackHead> {
    Ok(VbStackHead {
        stack_id: String::new(),
        position: 0,
        // Legacy rows store shortened names, including remote refs, and read them as local branches.
        name: name.shorten().to_str()?.to_owned(),
        head_sha: gix::ObjectId::null(gix::hash::Kind::Sha1).to_string(),
        pr_number: value.review.pull_request.map(i64::try_from).transpose()?,
        archived: false,
        review_id: value.review.review_id.clone(),
    })
}

fn full_branch_name(name: &str) -> Option<FullName> {
    FullName::try_from(format!("refs/heads/{name}")).ok()
}

fn managed_ref_info() -> RefInfo {
    // The schema has no workspace creation time; retain the existing deterministic marker.
    RefInfo {
        created_at: Some(gix::date::Time::new(1675176957, 0)),
        updated_at: None,
    }
}
