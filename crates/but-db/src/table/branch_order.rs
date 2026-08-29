#![allow(missing_docs)]

use std::collections::BTreeSet;

use rusqlite::OptionalExtension;

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
    conn: &'conn rusqlite::Connection,
}

/// Mutating accessor for ad-hoc branch ordering metadata.
pub struct BranchOrderHandleMut<'conn> {
    trans: crate::WriteScope<'conn>,
}

impl DbHandle {
    /// Return a read-only handle for ad-hoc branch ordering metadata.
    pub fn branch_order(&self) -> BranchOrderHandle<'_> {
        BranchOrderHandle { conn: &self.conn }
    }

    /// Return a mutating handle for ad-hoc branch ordering metadata.
    pub fn branch_order_mut(&mut self) -> rusqlite::Result<BranchOrderHandleMut<'_>> {
        Ok(BranchOrderHandleMut {
            trans: crate::WriteScope::Owned(
                self.conn
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?,
            ),
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
            trans: crate::WriteScope::Nested(self.inner_mut().savepoint()?),
        })
    }
}

impl BranchOrderHandle<'_> {
    /// Return the ordered chain containing `ref_name`, from tip to base.
    pub fn order_for_reference(&self, ref_name: &str) -> rusqlite::Result<Option<Vec<String>>> {
        let has_row = match self.has_reference(ref_name) {
            Ok(has_row) => has_row,
            Err(err) if is_missing_branch_order_table(&err) => return Ok(None),
            Err(err) => return Err(err),
        };
        let has_child = self.child_of(ref_name)?.is_some();
        if !has_row && !has_child {
            return Ok(None);
        }

        let mut seen = BTreeSet::from([ref_name.to_owned()]);
        let mut above = Vec::new();
        let mut cursor = ref_name.to_owned();
        while let Some(child) = self.child_of(&cursor)? {
            if !seen.insert(child.clone()) {
                return Ok(None);
            }
            above.push(child.clone());
            cursor = child;
        }

        let mut below = vec![ref_name.to_owned()];
        let mut cursor = ref_name.to_owned();
        while let Some(parent) = self.parent_of(&cursor)? {
            if !seen.insert(parent.clone()) {
                return Ok(None);
            }
            below.push(parent.clone());
            cursor = parent;
        }

        above.reverse();
        above.extend(below);
        Ok(Some(above))
    }

    fn has_reference(&self, ref_name: &str) -> rusqlite::Result<bool> {
        self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM branch_order WHERE branch_ref_name = ?1)",
            [ref_name],
            |row| row.get(0),
        )
    }

    fn parent_of(&self, ref_name: &str) -> rusqlite::Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT parent_ref_name FROM branch_order WHERE branch_ref_name = ?1",
                [ref_name],
                |row| row.get(0),
            )
            .optional()
            .map(Option::flatten)
    }

    fn child_of(&self, ref_name: &str) -> rusqlite::Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT branch_ref_name FROM branch_order WHERE parent_ref_name = ?1",
                [ref_name],
                |row| row.get(0),
            )
            .optional()
    }

    fn all_references(&self) -> rusqlite::Result<Vec<String>> {
        self.conn
            .prepare("SELECT branch_ref_name FROM branch_order")?
            .query_map([], |row| row.get(0))?
            .collect()
    }
}

fn is_missing_branch_order_table(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(_, Some(message))
            if message.contains("no such table: branch_order")
    )
}

impl BranchOrderHandleMut<'_> {
    /// Convert this mutating handle into a read-only view on the same savepoint.
    pub fn to_ref(&self) -> BranchOrderHandle<'_> {
        BranchOrderHandle { conn: &self.trans }
    }

    /// Replace the persisted chain containing `branches` with `branches` in tip-to-base order.
    pub fn set_order(self, branches: &[String]) -> rusqlite::Result<()> {
        let trans = self.trans;
        if branches.is_empty() {
            trans.commit()?;
            return Ok(());
        }

        let mut seen = BTreeSet::new();
        for branch in branches {
            if !seen.insert(branch.as_str()) {
                return Err(rusqlite::Error::InvalidQuery);
            }
        }

        let order = BranchOrderHandle { conn: &trans };
        let mut refs_to_replace = BTreeSet::new();
        for branch in branches {
            refs_to_replace.insert(branch.as_str().to_owned());
            if let Some(order) = order.order_for_reference(branch)? {
                refs_to_replace.extend(order);
            }
        }

        for branch in refs_to_replace {
            trans.execute(
                "DELETE FROM branch_order WHERE branch_ref_name = ?1",
                [branch],
            )?;
        }

        let mut insert = trans.prepare(
            "INSERT INTO branch_order (branch_ref_name, parent_ref_name) VALUES (?1, ?2)",
        )?;
        for (idx, branch) in branches.iter().enumerate() {
            let parent = branches.get(idx + 1);
            insert.execute(rusqlite::params![branch, parent])?;
        }
        drop(insert);

        trans.commit()
    }

    /// Remove `ref_name` from its chain, connecting its child directly to its parent if needed.
    pub fn remove_reference(self, ref_name: &str) -> rusqlite::Result<()> {
        let trans = self.trans;
        remove_reference_in_savepoint(&trans, ref_name)?;
        trans.commit()
    }

    /// Rename `old_ref_name` to `new_ref_name` everywhere it appears in branch-order metadata.
    pub fn rename_reference(self, old_ref_name: &str, new_ref_name: &str) -> rusqlite::Result<()> {
        let trans = self.trans;
        if old_ref_name == new_ref_name {
            trans.commit()?;
            return Ok(());
        }

        let order = BranchOrderHandle { conn: &trans };
        let has_old = order.has_reference(old_ref_name)? || order.child_of(old_ref_name)?.is_some();
        if !has_old {
            trans.commit()?;
            return Ok(());
        }
        if order.has_reference(new_ref_name)? || order.child_of(new_ref_name)?.is_some() {
            return Err(rusqlite::Error::InvalidQuery);
        }

        trans.execute(
            "UPDATE branch_order SET parent_ref_name = ?1 WHERE parent_ref_name = ?2",
            [new_ref_name, old_ref_name],
        )?;
        trans.execute(
            "UPDATE branch_order SET branch_ref_name = ?1 WHERE branch_ref_name = ?2",
            [new_ref_name, old_ref_name],
        )?;

        trans.commit()
    }

    /// Remove branch-order rows for references not present in `existing_ref_names`.
    pub fn remove_missing_references(self, existing_ref_names: &[String]) -> rusqlite::Result<()> {
        let trans = self.trans;
        let existing = existing_ref_names
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let refs = BranchOrderHandle { conn: &trans }.all_references()?;
        for ref_name in refs {
            if !existing.contains(ref_name.as_str()) {
                remove_reference_in_savepoint(&trans, &ref_name)?;
            }
        }
        trans.commit()
    }
}

fn remove_reference_in_savepoint(
    conn: &rusqlite::Connection,
    ref_name: &str,
) -> rusqlite::Result<()> {
    let parent = BranchOrderHandle { conn }.parent_of(ref_name)?;
    let child = BranchOrderHandle { conn }.child_of(ref_name)?;

    conn.execute(
        "DELETE FROM branch_order WHERE branch_ref_name = ?1",
        [ref_name],
    )?;
    if let Some(child) = child.as_ref() {
        conn.execute(
            "UPDATE branch_order SET parent_ref_name = ?1 WHERE branch_ref_name = ?2",
            rusqlite::params![parent, child],
        )?;
    }
    let singleton = match (parent.as_deref(), child.as_deref()) {
        (Some(parent), None) => Some(parent),
        (None, Some(child)) => Some(child),
        _ => None,
    };
    if let Some(singleton) = singleton {
        let order = BranchOrderHandle { conn };
        if order.parent_of(singleton)?.is_none() && order.child_of(singleton)?.is_none() {
            conn.execute(
                "DELETE FROM branch_order WHERE branch_ref_name = ?1",
                [singleton],
            )?;
        }
    }
    Ok(())
}
