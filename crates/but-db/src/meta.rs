//! Concrete metadata views over the project's virtual-branch and branch-order tables.

use std::{
    any::Any,
    cell::Cell,
    ops::{Deref, DerefMut},
};

use anyhow::bail;
use bstr::ByteSlice as _;
use but_core::{
    WORKSPACE_REF_NAME, is_workspace_ref_name,
    ref_metadata::{
        Branch, RefInfo, Review, StackId, ValueInfo, Workspace, WorkspaceCommitRelation,
        WorkspaceStack, WorkspaceStackBranch,
    },
};
use gix::refs::{FullName, FullNameRef};
use rusqlite::OptionalExtension;

use crate::{
    DbHandle, Transaction, VbStack, VbStackHead, VirtualBranchesSnapshot,
    table::{
        branch_order::{BranchOrderHandle, BranchOrderHandleMut},
        virtual_branches::{VirtualBranchesHandle, VirtualBranchesHandleMut},
    },
};

pub(crate) const M: &[crate::M<'static>] = &[crate::M::up(
    20261106120000,
    crate::SchemaVersion::Zero,
    "CREATE TABLE ref_info (
        ref_name BLOB NOT NULL,
        is_workspace INTEGER NOT NULL,
        created_at_seconds INTEGER,
        created_at_offset INTEGER,
        updated_at_seconds INTEGER,
        updated_at_offset INTEGER,
        PRIMARY KEY (ref_name, is_workspace),
        CHECK ((created_at_seconds IS NULL) = (created_at_offset IS NULL)),
        CHECK ((updated_at_seconds IS NULL) = (updated_at_offset IS NULL))
    );
    CREATE INDEX idx_vb_stack_heads_name ON vb_stack_heads(name);",
)];

/// Reference timestamps and explicit workspace identities, for snapshot restoration.
#[derive(Default, Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RefInfoSnapshot {
    entries: Vec<RefInfoEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct RefInfoEntry {
    ref_name: Vec<u8>,
    is_workspace: bool,
    created_at_seconds: Option<i64>,
    created_at_offset: Option<i32>,
    updated_at_seconds: Option<i64>,
    updated_at_offset: Option<i32>,
}

impl RefInfoSnapshot {
    /// Validate persisted names, timestamp pairs, and uniqueness before restoring anything.
    pub fn validate(&self) -> anyhow::Result<()> {
        let mut seen = std::collections::HashSet::new();
        for entry in &self.entries {
            let name: &FullNameRef = entry.ref_name.as_bstr().try_into()?;
            anyhow::ensure!(
                !entry.is_workspace || is_workspace_ref_name(name),
                "Invalid workspace metadata reference: {name}"
            );
            anyhow::ensure!(
                seen.insert((&entry.ref_name, entry.is_workspace)),
                "Duplicate reference metadata: {name}"
            );
            anyhow::ensure!(
                entry.created_at_seconds.is_some() == entry.created_at_offset.is_some()
                    && entry.updated_at_seconds.is_some() == entry.updated_at_offset.is_some(),
                "Incomplete timestamp for {name}"
            );
        }
        Ok(())
    }
}

impl RefInfoEntry {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            ref_name: row.get(0)?,
            is_workspace: row.get(1)?,
            created_at_seconds: row.get(2)?,
            created_at_offset: row.get(3)?,
            updated_at_seconds: row.get(4)?,
            updated_at_offset: row.get(5)?,
        })
    }

    fn ref_info(&self) -> RefInfo {
        RefInfo {
            created_at: self
                .created_at_seconds
                .zip(self.created_at_offset)
                .map(|(seconds, offset)| gix::date::Time::new(seconds, offset)),
            updated_at: self
                .updated_at_seconds
                .zip(self.updated_at_offset)
                .map(|(seconds, offset)| gix::date::Time::new(seconds, offset)),
        }
    }
}

/// Read-only workspace and branch metadata on a database connection or transaction.
pub struct Meta<'conn> {
    conn: &'conn rusqlite::Connection,
}

/// Atomic metadata mutations on a database connection or transaction.
pub struct MetaMut<'conn> {
    sp: rusqlite::Savepoint<'conn>,
    mirror_on_commit: bool,
    metadata_changed: Option<&'conn Cell<bool>>,
}

/// A metadata value bound to the reference from which it was read.
pub struct MetaHandle<T> {
    ref_name: FullName,
    value: T,
    is_default: bool,
    stack_id: Cell<Option<StackId>>,
}

impl<T> MetaHandle<T> {
    /// The owning stack, if this value represents a persisted branch.
    pub fn stack_id(&self) -> Option<StackId> {
        self.stack_id.get()
    }
}
impl<T> Deref for MetaHandle<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}
impl<T> DerefMut for MetaHandle<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}
impl<T> AsRef<FullNameRef> for MetaHandle<T> {
    fn as_ref(&self) -> &FullNameRef {
        self.ref_name.as_ref()
    }
}
impl<T> ValueInfo for MetaHandle<T> {
    fn is_default(&self) -> bool {
        self.is_default
    }
}

impl DbHandle {
    /// Access workspace and branch metadata from this database.
    pub fn meta(&self) -> Meta<'_> {
        Meta { conn: &self.conn }
    }
    /// Access atomic workspace and branch metadata mutations.
    pub fn meta_mut(&mut self) -> rusqlite::Result<MetaMut<'_>> {
        let mirror_on_commit = self.conn.is_autocommit();
        Ok(MetaMut {
            sp: self.conn.savepoint()?,
            mirror_on_commit,
            metadata_changed: None,
        })
    }
}
impl Transaction<'_> {
    /// Access metadata within this transaction.
    pub fn meta(&self) -> Meta<'_> {
        Meta { conn: self.inner() }
    }
    /// Access metadata mutations within this transaction.
    pub fn meta_mut(&mut self) -> rusqlite::Result<MetaMut<'_>> {
        Ok(MetaMut {
            sp: self
                .inner
                .as_mut()
                .expect("transaction is active")
                .savepoint()?,
            mirror_on_commit: false,
            metadata_changed: Some(&self.metadata_changed),
        })
    }
}

impl Meta<'_> {
    fn snapshot(&self) -> anyhow::Result<VirtualBranchesSnapshot> {
        Ok(VirtualBranchesHandle { conn: self.conn }
            .get_snapshot()?
            .unwrap_or_default())
    }

    fn ref_info(
        &self,
        name: &FullNameRef,
        is_workspace: bool,
    ) -> rusqlite::Result<Option<RefInfo>> {
        let entry = self.conn.query_row(
            "SELECT ref_name, is_workspace, created_at_seconds, created_at_offset, updated_at_seconds, updated_at_offset FROM ref_info WHERE ref_name = ?1 AND is_workspace = ?2",
            rusqlite::params![name.as_bstr().as_bytes(), is_workspace],
            RefInfoEntry::from_row,
        ).optional();
        match entry {
            // Read-only observers can open older databases without running this migration.
            Err(rusqlite::Error::SqliteFailure(_, Some(message)))
                if message == "no such table: ref_info" =>
            {
                Ok(None)
            }
            entry => entry.map(|entry| entry.map(|entry| entry.ref_info())),
        }
    }

    /// Read reference timestamps and explicit workspace identities for an operation snapshot.
    pub fn ref_info_snapshot(&self) -> anyhow::Result<RefInfoSnapshot> {
        let entries = self.conn.prepare(
            "SELECT ref_name, is_workspace, created_at_seconds, created_at_offset, updated_at_seconds, updated_at_offset FROM ref_info ORDER BY ref_name, is_workspace",
        )?.query_map([], RefInfoEntry::from_row)?.collect::<Result<Vec<_>, _>>()?;
        Ok(RefInfoSnapshot { entries })
    }

    /// Read workspace metadata, returning a default value for an unknown workspace.
    pub fn workspace(&self, ref_name: &FullNameRef) -> anyhow::Result<MetaHandle<Workspace>> {
        let mut value = Workspace::default();
        let persisted_ref_info = self.ref_info(ref_name, true)?;
        if is_workspace_ref_name(ref_name) {
            value.ref_info = persisted_ref_info.clone().unwrap_or(RefInfo {
                created_at: Some(gix::date::Time::new(1675176957, 0)),
                updated_at: None,
            });
            let snapshot = self.snapshot()?;
            for stack in &snapshot.stacks {
                let branches = snapshot
                    .heads
                    .iter()
                    .filter(|head| head.stack_id == stack.id)
                    .rev()
                    .filter_map(|head| {
                        full_name(&head.name).map(|ref_name| WorkspaceStackBranch {
                            ref_name,
                            archived: head.archived,
                        })
                    })
                    .collect::<Vec<_>>();
                if !branches.is_empty() {
                    value.stacks.push(WorkspaceStack {
                        id: stack.id.parse()?,
                        branches,
                        workspacecommit_relation: if stack.in_workspace {
                            WorkspaceCommitRelation::Merged
                        } else {
                            WorkspaceCommitRelation::Outside
                        },
                    });
                }
            }
        }
        Ok(MetaHandle {
            ref_name: ref_name.to_owned(),
            is_default: persisted_ref_info.is_none() && value.stacks.is_empty(),
            value,
            stack_id: Cell::new(None),
        })
    }

    /// Read branch metadata, returning a default value for an unknown branch.
    pub fn branch(&self, ref_name: &FullNameRef) -> anyhow::Result<MetaHandle<Branch>> {
        let name = ref_name.as_bstr().to_str().ok();
        let short_name = (ref_name.category() == Some(gix::refs::Category::LocalBranch))
            .then(|| ref_name.shorten().to_str().ok())
            .flatten();
        let found = self
            .conn
            .query_row(
                "SELECT heads.pr_number, heads.review_id, stacks.id
             FROM vb_stack_heads AS heads JOIN vb_stacks AS stacks ON heads.stack_id = stacks.id
             WHERE heads.name = ?1 OR heads.name = ?2
             ORDER BY stacks.sort_order, stacks.id, heads.position LIMIT 1",
                rusqlite::params![name, short_name],
                |row| {
                    Ok((
                        row.get::<_, Option<i64>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;
        let (value, stack_id) = match found {
            Some((pr_number, review_id, stack_id)) => (
                Branch {
                    ref_info: self.ref_info(ref_name, false)?.unwrap_or_default(),
                    review: Review {
                        pull_request: pr_number.map(usize::try_from).transpose()?,
                        review_id,
                    },
                },
                Some(stack_id.parse()?),
            ),
            None => (Branch::default(), None),
        };
        Ok(MetaHandle {
            ref_name: ref_name.to_owned(),
            is_default: stack_id.is_none(),
            value,
            stack_id: Cell::new(stack_id),
        })
    }

    /// Read a branch only if metadata exists for it.
    pub fn branch_opt(&self, ref_name: &FullNameRef) -> anyhow::Result<Option<MetaHandle<Branch>>> {
        let value = self.branch(ref_name)?;
        Ok((!value.is_default()).then_some(value))
    }

    /// Read a workspace only if metadata exists for it.
    pub fn workspace_opt(
        &self,
        ref_name: &FullNameRef,
    ) -> anyhow::Result<Option<MetaHandle<Workspace>>> {
        let value = self.workspace(ref_name)?;
        Ok((!value.is_default()).then_some(value))
    }

    /// Iterate persisted branches and their workspace, for metadata maintenance.
    pub fn iter(&self) -> impl Iterator<Item = anyhow::Result<(FullName, Box<dyn Any>)>> {
        let values = (|| -> anyhow::Result<Vec<(FullName, Box<dyn Any>)>> {
            let snapshot = self.snapshot()?;
            let mut values = Vec::new();
            for stack in &snapshot.stacks {
                for head in snapshot
                    .heads
                    .iter()
                    .filter(|head| head.stack_id == stack.id)
                {
                    if let Some(name) = full_name(&head.name) {
                        values.push((
                            name.clone(),
                            Box::new(self.branch(name.as_ref())?.value) as Box<dyn Any>,
                        ));
                    }
                }
            }
            if self
                .workspace_opt(WORKSPACE_REF_NAME.try_into()?)?
                .is_some()
            {
                let name = FullName::try_from(WORKSPACE_REF_NAME)?;
                values.push((name.clone(), Box::new(self.workspace(name.as_ref())?.value)));
            }
            Ok(values)
        })();
        match values {
            Ok(values) => values.into_iter().map(Ok).collect::<Vec<_>>(),
            Err(err) => vec![Err(err)],
        }
        .into_iter()
    }

    /// Return the persisted stack order, which may still contain deleted Git refs.
    pub fn branch_stack_order(
        &self,
        ref_name: &FullNameRef,
    ) -> anyhow::Result<Option<Vec<FullName>>> {
        BranchOrderHandle { conn: self.conn }
            .order_for_reference(ref_name.as_bstr().to_str()?)?
            .map(|names| {
                names
                    .into_iter()
                    .map(|name| FullName::try_from(name).map_err(Into::into))
                    .collect()
            })
            .transpose()
    }
}

impl MetaMut<'_> {
    /// Read metadata on the same connection.
    pub fn to_ref(&self) -> Meta<'_> {
        Meta { conn: &self.sp }
    }

    /// Restore reference timestamps and workspace identities as one atomic mutation.
    pub fn restore_ref_info(self, snapshot: &RefInfoSnapshot) -> anyhow::Result<()> {
        snapshot.validate()?;
        self.sp.execute("DELETE FROM ref_info", [])?;
        for entry in &snapshot.entries {
            store_ref_info(
                &self.sp,
                entry.ref_name.as_bstr().try_into()?,
                entry.is_workspace,
                &entry.ref_info(),
            )?;
        }
        crate::legacy::commit_with_mirror(self.sp, self.mirror_on_commit, self.metadata_changed)?;
        Ok(())
    }

    /// Replace workspace membership and order, preserving branch review and legacy data.
    pub fn set_workspace(mut self, value: &MetaHandle<Workspace>) -> anyhow::Result<()> {
        if !is_workspace_ref_name(value.as_ref()) {
            bail!("Workspace metadata requires a workspace reference");
        }
        let mut sp = self.sp.savepoint()?;
        let previous = Meta { conn: &sp }.snapshot()?;
        let mut next = VirtualBranchesSnapshot {
            state: previous.state.clone(),
            ..Default::default()
        };
        next.state.initialized = true;
        for (order, stack) in value.stacks.iter().enumerate() {
            if stack.branches.is_empty() {
                bail!("BUG: incoming stack is empty; remove the entire stack instead");
            }
            let id = stack.id.to_string();
            let old = previous.stacks.iter().find(|old| old.id == id);
            let mut row = old.cloned().unwrap_or_else(|| new_stack(stack.id, order));
            row.sort_order = order.try_into()?;
            row.in_workspace = stack.is_in_workspace();
            let mut seen = std::collections::HashSet::new();
            let mut branches = Vec::new();
            for branch in stack.branches.iter().rev() {
                // Shared segments across stacks are valid, repeats inside one stack are stale data.
                if !seen.insert(&branch.ref_name) {
                    continue;
                }
                let existing = previous
                    .heads
                    .iter()
                    .find(|head| {
                        head.stack_id == id && matches_name(&head.name, branch.ref_name.as_ref())
                    })
                    .or_else(|| {
                        previous
                            .heads
                            .iter()
                            .find(|head| matches_name(&head.name, branch.ref_name.as_ref()))
                    });
                let mut head = existing
                    .cloned()
                    .unwrap_or(new_head(&id, branch.ref_name.as_ref())?);
                head.stack_id = id.clone();
                head.position = branches.len().try_into()?;
                head.archived = branch.archived;
                next.heads.push(head);
                branches.push(branch.ref_name.as_bstr().to_str()?.to_owned());
            }
            next.stacks.push(row);
            branches.reverse();
            BranchOrderHandleMut {
                sp: sp.savepoint()?,
            }
            .set_order(&branches)?;
        }
        VirtualBranchesHandleMut {
            sp: sp.savepoint()?,
            mirror_on_commit: false,
            metadata_changed: None,
        }
        .replace_snapshot(&next)?;
        store_ref_info(&sp, value.as_ref(), true, &value.ref_info)?;
        sp.commit()?;
        crate::legacy::commit_with_mirror(self.sp, self.mirror_on_commit, self.metadata_changed)?;
        Ok(())
    }

    /// Store branch review metadata, creating an unapplied stack for a new branch.
    pub fn set_branch(mut self, value: &MetaHandle<Branch>) -> anyhow::Result<()> {
        let mut sp = self.sp.savepoint()?;
        let mut snapshot = Meta { conn: &sp }.snapshot()?;
        snapshot.state.initialized = true;
        let index = if let Some(stack_id) = value.stack_id() {
            let id = stack_id.to_string();
            Some(
                snapshot
                    .heads
                    .iter()
                    .position(|head| {
                        head.stack_id == id && matches_name(&head.name, value.as_ref())
                    })
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Branch metadata for '{}' no longer belongs to stack {stack_id}",
                            value.as_ref()
                        )
                    })?,
            )
        } else {
            snapshot.stacks.iter().find_map(|stack| {
                snapshot.heads.iter().position(|head| {
                    head.stack_id == stack.id && matches_name(&head.name, value.as_ref())
                })
            })
        };
        let index = match index {
            Some(index) => index,
            None => {
                let id = StackId::generate();
                snapshot.stacks.push(new_stack(id, snapshot.stacks.len()));
                snapshot
                    .heads
                    .push(new_head(&id.to_string(), value.as_ref())?);
                snapshot.heads.len() - 1
            }
        };
        let head = &mut snapshot.heads[index];
        head.pr_number = value.review.pull_request.map(i64::try_from).transpose()?;
        head.review_id = value.review.review_id.clone();
        let stack_id = head.stack_id.parse()?;
        VirtualBranchesHandleMut {
            sp: sp.savepoint()?,
            mirror_on_commit: false,
            metadata_changed: None,
        }
        .replace_snapshot(&snapshot)?;
        store_ref_info(&sp, value.as_ref(), false, &value.ref_info)?;
        sp.commit()?;
        value.stack_id.set(Some(stack_id));
        crate::legacy::commit_with_mirror(self.sp, self.mirror_on_commit, self.metadata_changed)?;
        Ok(())
    }

    /// Persist an ad-hoc stack's branch order from tip to base.
    pub fn set_branch_stack_order(mut self, branches: &[FullName]) -> anyhow::Result<()> {
        let names = branches
            .iter()
            .map(|name| name.as_bstr().to_str().map(str::to_owned))
            .collect::<Result<Vec<_>, _>>()?;
        BranchOrderHandleMut {
            sp: self.sp.savepoint()?,
        }
        .set_order(&names)?;
        crate::legacy::commit_with_mirror(self.sp, self.mirror_on_commit, self.metadata_changed)?;
        Ok(())
    }

    /// Rename a reference in persisted ad-hoc stack order.
    pub fn rename_branch_stack_order_reference(
        mut self,
        old: &FullNameRef,
        new: &FullNameRef,
    ) -> anyhow::Result<()> {
        BranchOrderHandleMut {
            sp: self.sp.savepoint()?,
        }
        .rename_reference(old.as_bstr().to_str()?, new.as_bstr().to_str()?)?;
        crate::legacy::commit_with_mirror(self.sp, self.mirror_on_commit, self.metadata_changed)?;
        Ok(())
    }

    /// Prune ad-hoc stack entries whose Git refs no longer exist.
    pub fn remove_missing_branch_stack_order_references(
        mut self,
        existing: &[FullName],
    ) -> anyhow::Result<()> {
        let names = existing
            .iter()
            .map(|name| name.as_bstr().to_str().map(str::to_owned))
            .collect::<Result<Vec<_>, _>>()?;
        BranchOrderHandleMut {
            sp: self.sp.savepoint()?,
        }
        .remove_missing_references(&names)?;
        crate::legacy::commit_with_mirror(self.sp, self.mirror_on_commit, self.metadata_changed)?;
        Ok(())
    }

    /// Remove metadata associated with a reference, including its branch-order entry.
    pub fn remove(mut self, name: &FullNameRef) -> anyhow::Result<bool> {
        let mut sp = self.sp.savepoint()?;
        let mut snapshot = Meta { conn: &sp }.snapshot()?;
        let before = snapshot.heads.len();
        let removed_ref_info = if is_workspace_ref_name(name) {
            sp.execute("DELETE FROM ref_info", [])?
        } else {
            sp.execute(
                "DELETE FROM ref_info WHERE ref_name = ?1",
                [name.as_bstr().as_bytes()],
            )?
        };
        if is_workspace_ref_name(name) {
            snapshot.heads.clear();
        } else {
            snapshot
                .heads
                .retain(|head| !matches_name(&head.name, name));
        }
        snapshot
            .stacks
            .retain(|stack| snapshot.heads.iter().any(|head| head.stack_id == stack.id));
        let removed = removed_ref_info != 0
            || before != snapshot.heads.len()
            || Meta { conn: &sp }.branch_stack_order(name)?.is_some();
        BranchOrderHandleMut {
            sp: sp.savepoint()?,
        }
        .remove_reference(name.as_bstr().to_str()?)?;
        VirtualBranchesHandleMut {
            sp: sp.savepoint()?,
            mirror_on_commit: false,
            metadata_changed: None,
        }
        .replace_snapshot(&snapshot)?;
        sp.commit()?;
        crate::legacy::commit_with_mirror(self.sp, self.mirror_on_commit, self.metadata_changed)?;
        Ok(removed)
    }

    /// Rename branch metadata and stack order atomically, rejecting an occupied destination.
    pub fn rename(mut self, old: &FullNameRef, new: &FullNameRef) -> anyhow::Result<()> {
        if old == new {
            return Ok(());
        }
        let mut sp = self.sp.savepoint()?;
        let meta = Meta { conn: &sp };
        if meta.branch_opt(new)?.is_some() || meta.branch_stack_order(new)?.is_some() {
            bail!(
                "Cannot rename to '{}': a branch with that name already exists",
                new.shorten()
            );
        }
        let mut snapshot = meta.snapshot()?;
        for head in &mut snapshot.heads {
            if matches_name(&head.name, old) {
                head.name = stored_name(new)?;
            }
        }
        // Legacy snapshot edits may leave timestamps after removing a branch's normalized head.
        sp.execute(
            "DELETE FROM ref_info WHERE ref_name = ?1 AND is_workspace = 0",
            [new.as_bstr().as_bytes()],
        )?;
        sp.execute(
            "UPDATE ref_info SET ref_name = ?1 WHERE ref_name = ?2 AND is_workspace = 0",
            rusqlite::params![new.as_bstr().as_bytes(), old.as_bstr().as_bytes()],
        )?;
        BranchOrderHandleMut {
            sp: sp.savepoint()?,
        }
        .rename_reference(old.as_bstr().to_str()?, new.as_bstr().to_str()?)?;
        VirtualBranchesHandleMut {
            sp: sp.savepoint()?,
            mirror_on_commit: false,
            metadata_changed: None,
        }
        .replace_snapshot(&snapshot)?;
        sp.commit()?;
        crate::legacy::commit_with_mirror(self.sp, self.mirror_on_commit, self.metadata_changed)?;
        Ok(())
    }
}

fn store_ref_info(
    conn: &rusqlite::Connection,
    name: &FullNameRef,
    is_workspace: bool,
    info: &RefInfo,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO ref_info (ref_name, is_workspace, created_at_seconds, created_at_offset, updated_at_seconds, updated_at_offset)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(ref_name, is_workspace) DO UPDATE SET
         created_at_seconds = excluded.created_at_seconds, created_at_offset = excluded.created_at_offset,
         updated_at_seconds = excluded.updated_at_seconds, updated_at_offset = excluded.updated_at_offset",
        rusqlite::params![name.as_bstr().as_bytes(), is_workspace,
            info.created_at.map(|time| time.seconds), info.created_at.map(|time| time.offset),
            info.updated_at.map(|time| time.seconds), info.updated_at.map(|time| time.offset)],
    )?;
    Ok(())
}

fn full_name(name: &str) -> Option<FullName> {
    FullName::try_from(if name.starts_with("refs/") {
        name.to_owned()
    } else {
        format!("refs/heads/{name}")
    })
    .ok()
}
fn matches_name(name: &str, reference: &FullNameRef) -> bool {
    full_name(name).is_some_and(|name| name.as_ref() == reference)
}
fn stored_name(name: &FullNameRef) -> anyhow::Result<String> {
    Ok(
        if name.category() == Some(gix::refs::Category::LocalBranch) {
            name.shorten()
        } else {
            name.as_bstr()
        }
        .to_str()?
        .to_owned(),
    )
}
fn new_head(stack_id: &str, name: &FullNameRef) -> anyhow::Result<VbStackHead> {
    Ok(VbStackHead {
        stack_id: stack_id.to_owned(),
        position: 0,
        name: stored_name(name)?,
        head_sha: gix::hash::Kind::Sha1.null().to_string(),
        pr_number: None,
        archived: false,
        review_id: None,
    })
}
fn new_stack(id: StackId, order: usize) -> VbStack {
    VbStack {
        id: id.to_string(),
        source_refname: None,
        upstream_remote_name: None,
        upstream_branch_name: None,
        sort_order: order as i64,
        in_workspace: false,
        legacy_name: String::new(),
        legacy_notes: String::new(),
        legacy_ownership: String::new(),
        legacy_allow_rebasing: true,
        legacy_post_commits: false,
        legacy_tree_sha: gix::hash::Kind::Sha1.null().to_string(),
        legacy_head_sha: gix::hash::Kind::Sha1.null().to_string(),
        legacy_created_timestamp_ms: "0".into(),
        legacy_updated_timestamp_ms: "0".into(),
    }
}
