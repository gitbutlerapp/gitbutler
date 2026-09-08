#![allow(missing_docs)]

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context as _, Result, ensure};
use gix::{
    bstr::ByteSlice as _,
    refs::{FullName, FullNameRef},
};

use crate::{DbHandle, M, SchemaVersion, Transaction};

pub(crate) const M: &[M<'static>] = &[M::up(
    20260626120100,
    SchemaVersion::Zero,
    "CREATE TABLE IF NOT EXISTS `branch_order`(
    `branch_ref_name` TEXT NOT NULL PRIMARY KEY,
    `parent_ref_name` TEXT UNIQUE,
    CHECK (`parent_ref_name` IS NULL OR `branch_ref_name` != `parent_ref_name`)
);

CREATE INDEX IF NOT EXISTS `idx_branch_order_parent_ref_name` ON `branch_order`(`parent_ref_name`);",
)];

/// Read-only accessor for ad-hoc branch ordering metadata.
pub struct BranchOrderHandle<'conn> {
    pub(crate) conn: &'conn rusqlite::Connection,
}

/// Mutating accessor for ad-hoc branch ordering metadata.
pub struct BranchOrderHandleMut<'conn> {
    pub(crate) sp: rusqlite::Savepoint<'conn>,
}

/// Complete persisted branch-order state, suitable for snapshots and restoration.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BranchOrderSnapshot {
    /// All branch-order rows, sorted by branch reference name.
    pub entries: Vec<BranchOrderEntry>,
}

/// One persisted branch-order relationship.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BranchOrderEntry {
    /// The ordered branch reference.
    pub branch_ref_name: FullName,
    /// The branch immediately below this one, or `None` for the base.
    pub parent_ref_name: Option<FullName>,
}

impl BranchOrderSnapshot {
    /// Validate reference names and disjoint, acyclic branch chains before restoration starts.
    pub fn validate(&self) -> Result<()> {
        self.clone().into_chains().map(|_| ())
    }

    /// Validate and collect chains in tip-to-base order, sorted by their tips.
    /// A parent without its own row is retained as the terminal branch.
    pub fn into_chains(self) -> Result<Vec<Vec<FullName>>> {
        let mut branches = BTreeMap::new();
        let mut parents = BTreeSet::new();
        for BranchOrderEntry {
            branch_ref_name,
            parent_ref_name,
        } in self.entries
        {
            // FullName's serde implementation does not enforce its validity invariant.
            <&FullNameRef>::try_from(branch_ref_name.as_bstr())
                .context("invalid branch reference in branch order")?;
            if let Some(parent) = &parent_ref_name {
                <&FullNameRef>::try_from(parent.as_bstr())
                    .context("invalid parent reference in branch order")?;
                ensure!(
                    parent != &branch_ref_name,
                    "branch '{branch_ref_name}' cannot be its own parent"
                );
                ensure!(
                    parents.insert(parent.clone()),
                    "duplicate parent reference '{parent}'"
                );
            }
            ensure!(
                branches
                    .insert(branch_ref_name.clone(), parent_ref_name)
                    .is_none(),
                "duplicate branch reference '{branch_ref_name}'"
            );
        }

        let tips = branches
            .keys()
            .filter(|name| !parents.contains(*name))
            .cloned()
            .collect::<Vec<_>>();
        let mut chains = Vec::new();
        for tip in tips {
            let mut next = Some(tip);
            let mut chain = Vec::new();
            while let Some(name) = next {
                next = branches.remove(&name).flatten();
                chain.push(name);
            }
            chains.push(chain);
        }
        ensure!(branches.is_empty(), "cycle in branch-order metadata");
        Ok(chains)
    }
}

impl DbHandle {
    /// Return a read-only handle for ad-hoc branch ordering metadata.
    pub fn branch_order(&self) -> BranchOrderHandle<'_> {
        BranchOrderHandle { conn: &self.conn }
    }

    /// Return a mutating handle for ad-hoc branch ordering metadata.
    pub fn branch_order_mut(&mut self) -> rusqlite::Result<BranchOrderHandleMut<'_>> {
        Ok(BranchOrderHandleMut {
            sp: self.conn.savepoint()?,
        })
    }
}

impl<'conn> Transaction<'conn> {
    /// Return a read-only handle for ad-hoc branch ordering metadata.
    pub fn branch_order(&self) -> BranchOrderHandle<'_> {
        BranchOrderHandle { conn: self.inner() }
    }

    /// Return a mutating handle for ad-hoc branch ordering metadata.
    pub fn branch_order_mut(&mut self) -> rusqlite::Result<BranchOrderHandleMut<'_>> {
        Ok(BranchOrderHandleMut {
            sp: self.inner_mut().savepoint()?,
        })
    }
}

impl BranchOrderHandle<'_> {
    /// Read the complete branch-order table in deterministic order.
    pub fn get_snapshot(&self) -> Result<BranchOrderSnapshot> {
        let mut statement = self.conn.prepare(
            "SELECT branch_ref_name, parent_ref_name FROM branch_order ORDER BY branch_ref_name",
        )?;
        let mut rows = statement.query([])?;
        let mut entries = Vec::new();
        while let Some(row) = rows.next()? {
            entries.push(BranchOrderEntry {
                branch_ref_name: row.get_ref(0)?.as_bytes()?.as_bstr().try_into()?,
                parent_ref_name: row
                    .get_ref(1)?
                    .as_bytes_or_null()?
                    .map(|name| name.as_bstr().try_into())
                    .transpose()?,
            });
        }
        Ok(BranchOrderSnapshot { entries })
    }

    /// Return the ordered chain containing `ref_name`, from tip to base.
    pub fn order_for_reference(&self, ref_name: &FullNameRef) -> Result<Option<Vec<FullName>>> {
        let snapshot = match self.get_snapshot() {
            Ok(snapshot) => snapshot,
            Err(err)
                if matches!(err.downcast_ref::<rusqlite::Error>(),
                Some(rusqlite::Error::SqliteFailure(_, Some(message)))
                    if message.contains("no such table: branch_order")) =>
            {
                return Ok(None);
            }
            Err(err) => return Err(err),
        };
        Ok(snapshot
            .into_chains()?
            .into_iter()
            .find(|chain| chain.iter().any(|name| name.as_ref() == ref_name)))
    }
}

impl BranchOrderHandleMut<'_> {
    /// Convert this mutating handle into a read-only view on the same savepoint.
    pub fn to_ref(&self) -> BranchOrderHandle<'_> {
        BranchOrderHandle { conn: &self.sp }
    }

    /// Replace the complete branch-order table with `snapshot` atomically.
    pub fn replace_snapshot(self, snapshot: &BranchOrderSnapshot) -> Result<()> {
        snapshot.validate()?;
        let sp = self.sp;
        sp.execute("DELETE FROM branch_order", [])?;
        let mut insert = sp.prepare(
            "INSERT INTO branch_order (branch_ref_name, parent_ref_name) VALUES (?1, ?2)",
        )?;
        for entry in &snapshot.entries {
            insert.execute(rusqlite::params![
                entry.branch_ref_name.as_bstr().as_bytes(),
                entry
                    .parent_ref_name
                    .as_ref()
                    .map(|name| name.as_bstr().as_bytes()),
            ])?;
        }
        drop(insert);
        Ok(sp.commit()?)
    }

    /// Replace the persisted chain containing `branches` with `branches` in tip-to-base order.
    pub fn set_order(self, branches: &[FullName]) -> Result<()> {
        let mut seen = BTreeSet::new();
        for branch in branches {
            <&FullNameRef>::try_from(branch.as_bstr())
                .context("invalid branch reference in branch order")?;
            ensure!(
                seen.insert(branch.as_ref()),
                "duplicate branch reference '{branch}'"
            );
        }
        let existing = self.to_ref().get_snapshot()?.into_chains()?;
        let refs_to_replace = existing
            .into_iter()
            .filter(|chain| chain.iter().any(|name| seen.contains(name.as_ref())))
            .flatten()
            .collect::<Vec<_>>();
        replace_order(&self.sp, &refs_to_replace, branches)?;
        Ok(self.sp.commit()?)
    }

    /// Remove `ref_name` from its chain, connecting its child directly to its parent if needed.
    pub fn remove_reference(self, ref_name: &FullNameRef) -> Result<()> {
        self.retain_references(|name| name != ref_name)
    }

    /// Rename `old_ref_name` to `new_ref_name` everywhere it appears in branch-order metadata.
    pub fn rename_reference(
        self,
        old_ref_name: &FullNameRef,
        new_ref_name: &FullNameRef,
    ) -> Result<()> {
        <&FullNameRef>::try_from(new_ref_name.as_bstr()).context("invalid new branch reference")?;
        if old_ref_name == new_ref_name {
            return Ok(self.sp.commit()?);
        }
        let chains = self.to_ref().get_snapshot()?.into_chains()?;
        let names = chains
            .iter()
            .flatten()
            .map(|name| name.as_ref())
            .collect::<BTreeSet<_>>();
        if !names.contains(old_ref_name) {
            return Ok(self.sp.commit()?);
        }
        ensure!(
            !names.contains(new_ref_name),
            "branch '{new_ref_name}' already has order metadata"
        );
        let names = [
            new_ref_name.as_bstr().as_bytes(),
            old_ref_name.as_bstr().as_bytes(),
        ];
        self.sp.execute(
            "UPDATE branch_order SET parent_ref_name = ?1 WHERE parent_ref_name = ?2",
            names,
        )?;
        self.sp.execute(
            "UPDATE branch_order SET branch_ref_name = ?1 WHERE branch_ref_name = ?2",
            names,
        )?;
        Ok(self.sp.commit()?)
    }

    /// Remove branch-order rows for references not present in `existing_ref_names`.
    pub fn remove_missing_references(self, existing_ref_names: &[FullName]) -> Result<()> {
        let existing = existing_ref_names
            .iter()
            .map(|name| name.as_ref())
            .collect::<BTreeSet<_>>();
        self.retain_references(|name| existing.contains(name))
    }

    fn retain_references(self, keep: impl Fn(&FullNameRef) -> bool) -> Result<()> {
        for chain in self.to_ref().get_snapshot()?.into_chains()? {
            let mut remaining = chain
                .iter()
                .filter(|name| keep(name.as_ref()))
                .cloned()
                .collect::<Vec<_>>();
            if remaining.len() == chain.len() {
                continue;
            }
            // Removing a branch leaves no ordering to remember for a singleton.
            if remaining.len() == 1 {
                remaining.clear();
            }
            replace_order(&self.sp, &chain, &remaining)?;
        }
        Ok(self.sp.commit()?)
    }
}

fn replace_order(
    sp: &rusqlite::Savepoint<'_>,
    old_refs: &[FullName],
    branches: &[FullName],
) -> Result<()> {
    for name in old_refs {
        sp.execute(
            "DELETE FROM branch_order WHERE branch_ref_name = ?1",
            [name.as_bstr().as_bytes()],
        )?;
    }
    let mut insert =
        sp.prepare("INSERT INTO branch_order (branch_ref_name, parent_ref_name) VALUES (?1, ?2)")?;
    for (index, branch) in branches.iter().enumerate() {
        insert.execute(rusqlite::params![
            branch.as_bstr().as_bytes(),
            branches
                .get(index + 1)
                .map(|name| name.as_bstr().as_bytes()),
        ])?;
    }
    Ok(())
}
