use std::{collections::BTreeSet, path::Path};

use anyhow::{Context as _, Result, ensure};
use but_core::ref_metadata::{
    Branch, RefInfo, Review, Workspace, WorkspaceCommitRelation, WorkspaceStack,
    WorkspaceStackBranch,
};
use gix::{
    bstr::ByteSlice as _,
    refs::{FullName, FullNameRef},
};
use rusqlite::params;

use crate::{
    BranchOrderSnapshot, Connection, ConnectionMut, DbHandle, Transaction,
    connection::ConnectionMutInner,
    table::branch_order::{BranchOrderHandle, BranchOrderHandleMut},
};

pub(crate) const M: &[crate::M<'static>] = &[crate::M::up(
    20260907120000,
    crate::SchemaVersion::One,
    "CREATE TABLE workspace_metadata (
    ref_name BLOB NOT NULL PRIMARY KEY,
    created_at INTEGER,
    created_at_offset INTEGER,
    updated_at INTEGER,
    updated_at_offset INTEGER,
    CHECK ((created_at IS NULL) = (created_at_offset IS NULL)),
    CHECK ((updated_at IS NULL) = (updated_at_offset IS NULL))
);
CREATE TABLE workspace_stacks (
    workspace_ref BLOB NOT NULL,
    id TEXT NOT NULL,
    position INTEGER NOT NULL,
    relation TEXT NOT NULL CHECK (relation IN ('merged', 'merge-from', 'outside')),
    merge_commit BLOB,
    PRIMARY KEY (workspace_ref, id),
    FOREIGN KEY (workspace_ref) REFERENCES workspace_metadata(ref_name) ON DELETE CASCADE ON UPDATE CASCADE,
    CHECK (relation = 'merge-from' OR merge_commit IS NULL)
);
CREATE TABLE workspace_stack_branches (
    workspace_ref BLOB NOT NULL,
    stack_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    ref_name BLOB NOT NULL,
    archived INTEGER NOT NULL,
    PRIMARY KEY (workspace_ref, stack_id, position),
    FOREIGN KEY (workspace_ref, stack_id) REFERENCES workspace_stacks(workspace_ref, id) ON DELETE CASCADE ON UPDATE CASCADE
);
CREATE TABLE branch_metadata (
    ref_name BLOB NOT NULL PRIMARY KEY,
    created_at INTEGER,
    created_at_offset INTEGER,
    updated_at INTEGER,
    updated_at_offset INTEGER,
    pull_request INTEGER CHECK (pull_request >= 0),
    review_id TEXT,
    CHECK ((created_at IS NULL) = (created_at_offset IS NULL)),
    CHECK ((updated_at IS NULL) = (updated_at_offset IS NULL))
);

-- The singleton previously represented only this workspace. Preserve its managed marker,
-- existing stack identities and ordering, and base-to-tip heads in the new tip-to-base order.
INSERT INTO workspace_metadata (ref_name, created_at, created_at_offset)
SELECT CAST('refs/heads/gitbutler/workspace' AS BLOB), 1675176957, 0
WHERE EXISTS (SELECT 1 FROM vb_stack_heads);
INSERT INTO workspace_stacks (workspace_ref, id, position, relation)
SELECT CAST('refs/heads/gitbutler/workspace' AS BLOB), id,
       ROW_NUMBER() OVER (ORDER BY sort_order,
           (SELECT name FROM vb_stack_heads WHERE stack_id = vb_stacks.id ORDER BY position DESC LIMIT 1), id) - 1,
       CASE WHEN in_workspace THEN 'merged' ELSE 'outside' END
FROM vb_stacks WHERE EXISTS (SELECT 1 FROM vb_stack_heads WHERE stack_id = vb_stacks.id);
INSERT INTO workspace_stack_branches (workspace_ref, stack_id, position, ref_name, archived)
SELECT CAST('refs/heads/gitbutler/workspace' AS BLOB), stack_id,
       ROW_NUMBER() OVER (PARTITION BY stack_id ORDER BY position DESC) - 1,
       CAST('refs/heads/' || name AS BLOB), archived
FROM vb_stack_heads;
INSERT INTO branch_metadata (ref_name, pull_request, review_id)
SELECT ref_name, pr_number, review_id FROM (
    SELECT CAST('refs/heads/' || h.name AS BLOB) AS ref_name, h.pr_number, h.review_id,
           ROW_NUMBER() OVER (PARTITION BY h.name ORDER BY s.position, h.position) AS occurrence
    FROM vb_stack_heads h JOIN workspace_stacks s ON s.id = h.stack_id
) WHERE occurrence = 1;
-- Old workspace saves also populated branch_order. Exact whole-chain matches
-- should now follow workspace edits; preserve independent ad-hoc chains.
WITH workspace_order AS (
    SELECT workspace_ref, stack_id, position, CAST(ref_name AS TEXT) AS branch_ref_name,
           LEAD(CAST(ref_name AS TEXT)) OVER (
               PARTITION BY workspace_ref, stack_id ORDER BY position
           ) AS parent_ref_name
    FROM workspace_stack_branches
), inherited_stacks AS (
    SELECT w.workspace_ref, w.stack_id
    FROM workspace_order w LEFT JOIN branch_order b ON b.branch_ref_name = w.branch_ref_name
    GROUP BY w.workspace_ref, w.stack_id
    HAVING COUNT(b.branch_ref_name) = COUNT(*)
       AND SUM(b.parent_ref_name IS NOT w.parent_ref_name) = 0
       AND NOT EXISTS (
           SELECT 1 FROM workspace_order tip
           JOIN branch_order child ON child.parent_ref_name = tip.branch_ref_name
           WHERE tip.workspace_ref = w.workspace_ref AND tip.stack_id = w.stack_id AND tip.position = 0
       )
)
DELETE FROM branch_order WHERE branch_ref_name IN (
    SELECT w.branch_ref_name FROM workspace_order w JOIN inherited_stacks s
      ON s.workspace_ref = w.workspace_ref AND s.stack_id = w.stack_id
);
DROP TABLE vb_branch_targets;
DROP TABLE vb_stack_heads;
DROP TABLE vb_stacks;
DROP TABLE vb_state;",
)];

/// An owned, consistent snapshot of reference metadata from the project database.
///
/// Clone a workspace or branch, change its Rust fields, and save the whole value with
/// [`MetadataMut::set_workspace()`] or [`MetadataMut::set_branch()`]. Reads borrow this snapshot
/// without keeping a connection borrowed. Obtain a fresh snapshot after writing metadata.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Metadata {
    workspaces: Vec<(FullName, Workspace)>,
    branches: Vec<(FullName, Branch)>,
    branch_orders: Vec<Vec<FullName>>,
    #[serde(skip)]
    workspace_orders: std::sync::OnceLock<Vec<Vec<FullName>>>,
}

/// An atomic metadata mutation. Each operation commits on success.
///
/// Standalone mutations acquire a write lock before reading. Inside an existing transaction,
/// a nested savepoint isolates the mutation and the outer transaction controls persistence and
/// notification. On error or drop the mutation's writes roll back.
pub struct MetadataMut<'db> {
    transaction: MetadataTransaction<'db>,
    refresh: Refresh<'db>,
}

enum MetadataTransaction<'db> {
    Transaction(rusqlite::Transaction<'db>),
    Savepoint(rusqlite::Savepoint<'db>),
}

impl MetadataTransaction<'_> {
    fn connection(&self) -> &rusqlite::Connection {
        match self {
            Self::Transaction(tx) => tx,
            Self::Savepoint(sp) => sp,
        }
    }

    fn savepoint(&mut self) -> rusqlite::Result<rusqlite::Savepoint<'_>> {
        match self {
            Self::Transaction(tx) => tx.savepoint(),
            Self::Savepoint(sp) => sp.savepoint(),
        }
    }

    fn commit(self) -> rusqlite::Result<()> {
        match self {
            Self::Transaction(tx) => tx.commit(),
            Self::Savepoint(sp) => sp.commit(),
        }
    }
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
            transaction: MetadataTransaction::Transaction(
                self.conn
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?,
            ),
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
            transaction: MetadataTransaction::Savepoint(
                self.inner
                    .as_mut()
                    .expect("transaction is alive")
                    .savepoint()?,
            ),
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
    /// Build a snapshot from complete per-reference values and explicit ad-hoc branch orders.
    /// [`Self::validate()`] checks it before restoration or other external side effects.
    pub fn from_parts(
        workspaces: Vec<(FullName, Workspace)>,
        branches: Vec<(FullName, Branch)>,
        branch_orders: Vec<Vec<FullName>>,
    ) -> Self {
        Self {
            workspaces,
            branches,
            branch_orders,
            workspace_orders: Default::default(),
        }
    }

    /// Validate data from a snapshot before changing the database or Git repository.
    pub fn validate(&self) -> Result<()> {
        let mut names = BTreeSet::new();
        for (name, workspace) in &self.workspaces {
            FullName::try_from(name.as_bstr())?;
            ensure!(names.insert(name), "Duplicate workspace reference '{name}'");
            validate_workspace(workspace)?;
        }
        names.clear();
        for (name, branch) in &self.branches {
            FullName::try_from(name.as_bstr())?;
            ensure!(names.insert(name), "Duplicate branch reference '{name}'");
            branch.review.pull_request.map(i64::try_from).transpose()?;
        }
        names.clear();
        for chain in &self.branch_orders {
            ensure!(
                !chain.is_empty(),
                "An explicit branch order cannot be empty"
            );
            for name in chain {
                FullName::try_from(name.as_bstr())?;
                name.as_bstr().to_str()?;
                ensure!(names.insert(name), "Duplicate ordered reference '{name}'");
            }
        }
        Ok(())
    }

    fn read(conn: &rusqlite::Connection) -> Result<Self> {
        let read_tx = conn
            .is_autocommit()
            .then(|| conn.unchecked_transaction())
            .transpose()?;
        let mut workspaces = Vec::new();
        let mut stmt = conn.prepare("SELECT ref_name, created_at, created_at_offset, updated_at, updated_at_offset FROM workspace_metadata ORDER BY ref_name")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let name = full_name(row, 0)?;
            workspaces.push((
                name.clone(),
                Workspace {
                    ref_info: read_ref_info(row, 1)?,
                    stacks: read_stacks(conn, name.as_ref())?,
                },
            ));
        }
        let mut branches = Vec::new();
        let mut stmt = conn.prepare("SELECT ref_name, created_at, created_at_offset, updated_at, updated_at_offset, pull_request, review_id FROM branch_metadata ORDER BY ref_name")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            branches.push((
                full_name(row, 0)?,
                Branch {
                    ref_info: read_ref_info(row, 1)?,
                    review: Review {
                        pull_request: row
                            .get::<_, Option<i64>>(5)?
                            .map(usize::try_from)
                            .transpose()?,
                        review_id: row.get(6)?,
                    },
                },
            ));
        }
        let mut branch_orders: Vec<Vec<FullName>> = Vec::new();
        let order = BranchOrderHandle { conn };
        for entry in order.get_snapshot()?.entries {
            if branch_orders.iter().any(|chain| {
                chain
                    .iter()
                    .any(|name| name.as_bstr() == entry.branch_ref_name.as_str())
            }) {
                continue;
            }
            if let Some(chain) = order.order_for_reference(&entry.branch_ref_name)? {
                branch_orders.push(
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
        Ok(Self::from_parts(workspaces, branches, branch_orders))
    }

    /// Borrow the value saved for this exact workspace reference, including empty workspaces.
    pub fn workspace(&self, ref_name: &FullNameRef) -> Option<&Workspace> {
        self.workspaces
            .iter()
            .find_map(|(name, value)| (name.as_ref() == ref_name).then_some(value))
    }

    /// Borrow metadata saved for this exact branch reference.
    pub fn branch(&self, ref_name: &FullNameRef) -> Option<&Branch> {
        self.branches
            .iter()
            .find_map(|(name, value)| (name.as_ref() == ref_name).then_some(value))
    }

    /// Enumerate persisted workspaces in reference-name order.
    pub fn workspaces(&self) -> impl Iterator<Item = (&FullNameRef, &Workspace)> {
        self.workspaces
            .iter()
            .map(|(name, value)| (name.as_ref(), value))
    }

    /// Enumerate persisted branches in reference-name order.
    pub fn branches(&self) -> impl Iterator<Item = (&FullNameRef, &Branch)> {
        self.branches
            .iter()
            .map(|(name, value)| (name.as_ref(), value))
    }

    /// Enumerate explicitly stored ad-hoc branch orders, excluding workspace-derived orders.
    pub fn branch_orders(&self) -> impl Iterator<Item = &[FullName]> {
        self.branch_orders.iter().map(Vec::as_slice)
    }

    /// Borrow a tip-to-base branch order. Explicit ad-hoc ordering takes precedence.
    /// Workspace ordering is also available if all workspaces containing this reference agree.
    pub fn branch_stack_order(&self, ref_name: &FullNameRef) -> Option<&[FullName]> {
        if let Some(chain) = self
            .branch_orders
            .iter()
            .find(|chain| chain.iter().any(|name| name.as_ref() == ref_name))
        {
            return Some(chain);
        }
        let orders = self.workspace_orders.get_or_init(|| {
            self.workspaces
                .iter()
                .flat_map(|(_, ws)| {
                    ws.stacks.iter().map(|stack| {
                        stack
                            .branches
                            .iter()
                            .map(|branch| branch.ref_name.clone())
                            .collect()
                    })
                })
                .collect()
        });
        let mut matching = orders
            .iter()
            .filter(|chain| chain.iter().any(|name| name.as_ref() == ref_name));
        let first = matching.next()?;
        matching
            .all(|chain| chain == first)
            .then_some(first.as_slice())
    }
}

impl PartialEq for Metadata {
    fn eq(&self, other: &Self) -> bool {
        self.workspaces == other.workspaces
            && self.branches == other.branches
            && self.branch_orders == other.branch_orders
    }
}
impl Eq for Metadata {}

impl MetadataMut<'_> {
    fn finish(self) -> Result<()> {
        self.transaction.commit()?;
        match self.refresh {
            Refresh::Database(Some(path)) => but_project_handle::write_refresh_sentinel(path),
            Refresh::Database(None) => {}
            Refresh::Transaction(changed) => *changed = true,
        }
        Ok(())
    }

    /// Atomically replace all per-reference metadata and explicit branch ordering.
    pub fn replace_snapshot(mut self, snapshot: &Metadata) -> Result<()> {
        snapshot.validate()?;
        self.transaction.connection().execute_batch("DELETE FROM workspace_metadata; DELETE FROM branch_metadata; DELETE FROM branch_order;")?;
        for (name, value) in &snapshot.workspaces {
            write_workspace(self.transaction.connection(), name.as_ref(), value)?;
        }
        for (name, value) in &snapshot.branches {
            write_branch(self.transaction.connection(), name.as_ref(), value)?;
        }
        for chain in &snapshot.branch_orders {
            let names = chain
                .iter()
                .map(|name| name.as_bstr().to_str().map(ToOwned::to_owned))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            BranchOrderHandleMut {
                sp: self.transaction.savepoint()?,
            }
            .set_order(&names)?;
        }
        self.finish()
    }

    /// Restore explicit branch ordering atomically.
    pub fn replace_branch_order(mut self, snapshot: &BranchOrderSnapshot) -> Result<()> {
        snapshot.validate().map_err(anyhow::Error::msg)?;
        BranchOrderHandleMut {
            sp: self.transaction.savepoint()?,
        }
        .replace_snapshot(snapshot)?;
        self.finish()
    }

    /// Save this workspace's exact value, leaving other workspaces and branch metadata untouched.
    pub fn set_workspace(self, ref_name: &FullNameRef, value: &Workspace) -> Result<()> {
        validate_workspace(value)?;
        write_workspace(self.transaction.connection(), ref_name, value)?;
        self.finish()
    }

    /// Save this branch's complete value without inventing workspace or stack membership.
    pub fn set_branch(self, ref_name: &FullNameRef, value: &Branch) -> Result<()> {
        write_branch(self.transaction.connection(), ref_name, value)?;
        self.finish()
    }

    /// Delete this reference's metadata, workspace memberships, and explicit branch-order entry.
    /// Removing a workspace leaves other workspaces and all independent branch metadata intact.
    pub fn remove(mut self, ref_name: &FullNameRef) -> Result<bool> {
        let before = Metadata::read(self.transaction.connection())?;
        let had_order = before.branch_stack_order(ref_name).is_some();
        if let Ok(name) = ref_name.as_bstr().to_str() {
            BranchOrderHandleMut {
                sp: self.transaction.savepoint()?,
            }
            .remove_reference(name)?;
        }
        let conn = self.transaction.connection();
        let key = ref_name.as_bstr().as_bytes();
        let mut removed =
            conn.execute("DELETE FROM workspace_metadata WHERE ref_name = ?1", [key])? > 0;
        removed |= conn.execute("DELETE FROM branch_metadata WHERE ref_name = ?1", [key])? > 0;
        for (name, mut workspace) in before.workspaces {
            if name.as_ref() == ref_name {
                continue;
            }
            let mut changed = false;
            while workspace.remove_segment(ref_name) {
                changed = true;
            }
            if changed {
                write_workspace(conn, name.as_ref(), &workspace)?;
                removed = true;
            }
        }
        self.finish()?;
        Ok(removed || had_order)
    }

    /// Rename a reference and all of its workspace memberships and ordering atomically.
    pub fn rename(mut self, old: &FullNameRef, new: &FullNameRef) -> Result<()> {
        if old == new {
            return Ok(());
        }
        let before = Metadata::read(self.transaction.connection())?;
        ensure!(
            before.workspace(new).is_none()
                && before.branch(new).is_none()
                && before.branch_stack_order(new).is_none()
                && !before
                    .workspaces
                    .iter()
                    .any(|(_, ws)| ws.stacks.iter().any(|stack| stack
                        .branches
                        .iter()
                        .any(|branch| branch.ref_name.as_ref() == new))),
            "Cannot rename to '{}': a reference with that name already has metadata",
            new.shorten()
        );
        let conn = self.transaction.connection();
        for table in [
            "workspace_metadata",
            "branch_metadata",
            "workspace_stack_branches",
        ] {
            conn.execute(
                &format!("UPDATE {table} SET ref_name = ?1 WHERE ref_name = ?2"),
                params![new.as_bstr().as_bytes(), old.as_bstr().as_bytes()],
            )?;
        }
        if let Ok(old) = old.as_bstr().to_str() {
            if let Ok(new) = new.as_bstr().to_str() {
                BranchOrderHandleMut {
                    sp: self.transaction.savepoint()?,
                }
                .rename_reference(old, new)?;
            } else {
                ensure!(
                    !before
                        .branch_orders
                        .iter()
                        .flatten()
                        .any(|name| name.as_bstr() == old),
                    "Cannot store a non-UTF-8 reference in legacy ad-hoc branch ordering"
                );
            }
        }
        self.finish()
    }
    /// Replace the branch-order chains containing these references with this tip-to-base chain.
    pub fn set_branch_stack_order(mut self, branches: &[FullName]) -> Result<()> {
        let names = branches
            .iter()
            .map(|name| name.as_bstr().to_str().map(ToOwned::to_owned))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        BranchOrderHandleMut {
            sp: self.transaction.savepoint()?,
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
            sp: self.transaction.savepoint()?,
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
            sp: self.transaction.savepoint()?,
        }
        .remove_missing_references(&names)?;
        self.finish()
    }
}

fn full_name(row: &rusqlite::Row<'_>, index: usize) -> Result<FullName> {
    let bytes: Vec<u8> = row.get(index)?;
    Ok(FullName::try_from(bytes.as_bstr())?)
}

fn read_ref_info(row: &rusqlite::Row<'_>, start: usize) -> rusqlite::Result<RefInfo> {
    let time = |column| -> rusqlite::Result<Option<gix::date::Time>> {
        row.get::<_, Option<i64>>(column)?
            .map(|seconds| {
                row.get::<_, i32>(column + 1)
                    .map(|offset| gix::date::Time::new(seconds, offset))
            })
            .transpose()
    };
    Ok(RefInfo {
        created_at: time(start)?,
        updated_at: time(start + 2)?,
    })
}

fn read_stacks(conn: &rusqlite::Connection, name: &FullNameRef) -> Result<Vec<WorkspaceStack>> {
    let mut stmt = conn.prepare("SELECT id, relation, merge_commit FROM workspace_stacks WHERE workspace_ref = ?1 ORDER BY position, id")?;
    let mut rows = stmt.query([name.as_bstr().as_bytes()])?;
    let mut stacks = Vec::new();
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let relation: String = row.get(1)?;
        let merge_commit: Option<Vec<u8>> = row.get(2)?;
        let relation = match relation.as_str() {
            "merged" => WorkspaceCommitRelation::Merged,
            "outside" => WorkspaceCommitRelation::Outside,
            "merge-from" => WorkspaceCommitRelation::MergeFrom {
                commit_id: merge_commit
                    .as_deref()
                    .map(gix::ObjectId::try_from)
                    .transpose()
                    .context("Invalid workspace merge commit")?,
            },
            _ => anyhow::bail!("Invalid workspace commit relation '{relation}'"),
        };
        let mut stmt = conn.prepare("SELECT ref_name, archived FROM workspace_stack_branches WHERE workspace_ref = ?1 AND stack_id = ?2 ORDER BY position")?;
        let mut rows = stmt.query(params![name.as_bstr().as_bytes(), id])?;
        let mut branches = Vec::new();
        while let Some(row) = rows.next()? {
            branches.push(WorkspaceStackBranch {
                ref_name: full_name(row, 0)?,
                archived: row.get(1)?,
            });
        }
        stacks.push(WorkspaceStack {
            id: id.parse().context("Invalid metadata stack id")?,
            branches,
            workspacecommit_relation: relation,
        });
    }
    Ok(stacks)
}

fn validate_workspace(workspace: &Workspace) -> Result<()> {
    let mut ids = BTreeSet::new();
    for stack in &workspace.stacks {
        ensure!(
            ids.insert(stack.id),
            "A workspace cannot contain the same stack ID twice"
        );
        ensure!(
            !stack.branches.is_empty(),
            "Cannot save an empty metadata stack"
        );
        let mut names = BTreeSet::new();
        for branch in &stack.branches {
            FullName::try_from(branch.ref_name.as_bstr())?;
            ensure!(
                names.insert(&branch.ref_name),
                "A stack cannot contain the same branch twice"
            );
        }
    }
    Ok(())
}

fn write_workspace(
    conn: &rusqlite::Connection,
    name: &FullNameRef,
    value: &Workspace,
) -> Result<()> {
    let previous = read_stacks(conn, name)?;
    let key = name.as_bstr().as_bytes();
    let info = &value.ref_info;
    conn.execute(
        "INSERT INTO workspace_metadata (ref_name, created_at, created_at_offset, updated_at, updated_at_offset)
         VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(ref_name) DO UPDATE SET
         created_at = excluded.created_at, created_at_offset = excluded.created_at_offset,
         updated_at = excluded.updated_at, updated_at_offset = excluded.updated_at_offset
         WHERE created_at IS NOT excluded.created_at OR created_at_offset IS NOT excluded.created_at_offset
            OR updated_at IS NOT excluded.updated_at OR updated_at_offset IS NOT excluded.updated_at_offset",
        params![key, info.created_at.map(|t| t.seconds), info.created_at.map(|t| t.offset), info.updated_at.map(|t| t.seconds), info.updated_at.map(|t| t.offset)],
    )?;
    for stack in &previous {
        if !value.stacks.iter().any(|new| new.id == stack.id) {
            conn.execute(
                "DELETE FROM workspace_stacks WHERE workspace_ref = ?1 AND id = ?2",
                params![key, stack.id.to_string()],
            )?;
        }
    }
    for (position, stack) in value.stacks.iter().enumerate() {
        let old = previous
            .iter()
            .enumerate()
            .find(|(_, old)| old.id == stack.id);
        if old.is_some_and(|(old_position, old)| old_position == position && old == stack) {
            continue;
        }
        let (relation, merge_commit) = match &stack.workspacecommit_relation {
            WorkspaceCommitRelation::Merged => ("merged", None),
            WorkspaceCommitRelation::Outside => ("outside", None),
            WorkspaceCommitRelation::MergeFrom { commit_id } => {
                ("merge-from", commit_id.as_ref().map(|id| id.as_bytes()))
            }
        };
        let id = stack.id.to_string();
        if old.is_none_or(|(old_position, old)| {
            old_position != position
                || old.workspacecommit_relation != stack.workspacecommit_relation
        }) {
            conn.execute(
            "INSERT INTO workspace_stacks (workspace_ref, id, position, relation, merge_commit) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(workspace_ref, id) DO UPDATE SET position = excluded.position, relation = excluded.relation, merge_commit = excluded.merge_commit",
            params![key, id, i64::try_from(position)?, relation, merge_commit],
        )?;
        }
        conn.execute("DELETE FROM workspace_stack_branches WHERE workspace_ref = ?1 AND stack_id = ?2 AND position >= ?3", params![key, id, i64::try_from(stack.branches.len())?])?;
        for (position, branch) in stack.branches.iter().enumerate() {
            if old.and_then(|(_, old)| old.branches.get(position)) == Some(branch) {
                continue;
            }
            conn.execute(
                "INSERT INTO workspace_stack_branches (workspace_ref, stack_id, position, ref_name, archived) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(workspace_ref, stack_id, position) DO UPDATE SET ref_name = excluded.ref_name, archived = excluded.archived",
                params![key, id, i64::try_from(position)?, branch.ref_name.as_bstr().as_bytes(), branch.archived],
            )?;
        }
    }
    Ok(())
}

fn write_branch(conn: &rusqlite::Connection, name: &FullNameRef, value: &Branch) -> Result<()> {
    let info = &value.ref_info;
    conn.execute(
        "INSERT INTO branch_metadata (ref_name, created_at, created_at_offset, updated_at, updated_at_offset, pull_request, review_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT(ref_name) DO UPDATE SET
         created_at = excluded.created_at, created_at_offset = excluded.created_at_offset,
         updated_at = excluded.updated_at, updated_at_offset = excluded.updated_at_offset,
         pull_request = excluded.pull_request, review_id = excluded.review_id
         WHERE created_at IS NOT excluded.created_at OR created_at_offset IS NOT excluded.created_at_offset
            OR updated_at IS NOT excluded.updated_at OR updated_at_offset IS NOT excluded.updated_at_offset
            OR pull_request IS NOT excluded.pull_request OR review_id IS NOT excluded.review_id",
        params![name.as_bstr().as_bytes(), info.created_at.map(|t| t.seconds), info.created_at.map(|t| t.offset), info.updated_at.map(|t| t.seconds), info.updated_at.map(|t| t.offset), value.review.pull_request.map(i64::try_from).transpose()?, value.review.review_id],
    )?;
    Ok(())
}
