use crate::{DbHandle, Transaction, table};

/// A read-only view of a project connection, including an enclosing transaction's writes.
#[derive(Clone, Copy)]
pub struct Connection<'db> {
    pub(crate) conn: &'db rusqlite::Connection,
}

/// Exclusive access to a project database or an already-open transaction.
///
/// Passing this view through an operation keeps all its table access on one connection.
/// Transaction ownership stays with the caller, which decides whether to commit.
pub struct ConnectionMut<'db, 'conn> {
    pub(crate) inner: ConnectionMutInner<'db, 'conn>,
}

pub(crate) enum ConnectionMutInner<'db, 'conn> {
    Database(&'db mut DbHandle),
    Transaction(&'db mut Transaction<'conn>),
}

impl std::fmt::Debug for ConnectionMut<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionMut")
            .field(
                "transaction",
                &matches!(self.inner, ConnectionMutInner::Transaction(_)),
            )
            .finish_non_exhaustive()
    }
}

impl DbHandle {
    /// Borrow the project connection for reads.
    pub fn connection(&self) -> Connection<'_> {
        Connection { conn: &self.conn }
    }

    /// Borrow the project connection for an operation that may change metadata or other tables.
    pub fn connection_mut(&mut self) -> ConnectionMut<'_, 'static> {
        ConnectionMut {
            inner: ConnectionMutInner::Database(self),
        }
    }
}

impl<'conn> Transaction<'conn> {
    /// Read the connection within this transaction.
    pub fn connection(&self) -> Connection<'_> {
        Connection { conn: self.inner() }
    }

    /// Borrow this transaction without transferring its commit/rollback responsibility.
    pub fn connection_mut(&mut self) -> ConnectionMut<'_, 'conn> {
        ConnectionMut {
            inner: ConnectionMutInner::Transaction(self),
        }
    }
}

impl<'conn> ConnectionMut<'_, 'conn> {
    /// Temporarily lend this same connection to a nested operation.
    pub fn reborrow(&mut self) -> ConnectionMut<'_, 'conn> {
        ConnectionMut {
            inner: match &mut self.inner {
                ConnectionMutInner::Database(db) => ConnectionMutInner::Database(db),
                ConnectionMutInner::Transaction(tx) => ConnectionMutInner::Transaction(tx),
            },
        }
    }

    /// Read the same connection, including its uncommitted writes.
    pub fn as_ref(&self) -> Connection<'_> {
        Connection {
            conn: match &self.inner {
                ConnectionMutInner::Database(db) => &db.conn,
                ConnectionMutInner::Transaction(tx) => tx.inner(),
            },
        }
    }

    pub(crate) fn savepoint(&mut self) -> rusqlite::Result<rusqlite::Savepoint<'_>> {
        match &mut self.inner {
            ConnectionMutInner::Database(db) => db.conn.savepoint(),
            ConnectionMutInner::Transaction(tx) => tx.inner_mut().savepoint(),
        }
    }
}

// The view exposes the same typed table handles as its owners. Only construction is forwarded;
// query and mutation implementations remain in their existing table modules.
macro_rules! read_table {
    ($method:ident, $module:ident, $handle:ident) => {
        impl Connection<'_> {
            #[doc = concat!("Read the `", stringify!($module), "` table on this connection.")]
            pub fn $method(&self) -> table::$module::$handle<'_> {
                table::$module::$handle { conn: self.conn }
            }
        }
        impl ConnectionMut<'_, '_> {
            #[doc = concat!("Read the `", stringify!($module), "` table on this connection.")]
            pub fn $method(&self) -> table::$module::$handle<'_> {
                table::$module::$handle {
                    conn: self.as_ref().conn,
                }
            }
        }
    };
}

macro_rules! write_table {
    ($method:ident, $module:ident, $handle:ident) => {
        impl ConnectionMut<'_, '_> {
            #[doc = concat!("Mutate the `", stringify!($module), "` table in a nested savepoint.")]
            pub fn $method(&mut self) -> rusqlite::Result<table::$module::$handle<'_>> {
                Ok(table::$module::$handle {
                    sp: self.savepoint()?,
                })
            }
        }
    };
}

macro_rules! write_single_table {
    ($method:ident, $module:ident, $handle:ident) => {
        impl ConnectionMut<'_, '_> {
            #[doc = concat!("Mutate the `", stringify!($module), "` table on this connection.")]
            pub fn $method(&mut self) -> table::$module::$handle<'_> {
                table::$module::$handle {
                    conn: self.as_ref().conn,
                }
            }
        }
    };
}

read_table!(virtual_branches, virtual_branches, VirtualBranchesHandle);
read_table!(branch_order, branch_order, BranchOrderHandle);
read_table!(hunk_assignments, hunk_assignments, HunkAssignmentsHandle);
read_table!(worktree_meta, worktree_meta, WorktreeMetaHandle);
read_table!(gerrit_metadata, gerrit_metadata, GerritMetadataHandle);
read_table!(forge_reviews, forge_reviews, ForgeReviewsHandle);
read_table!(ci_checks, ci_checks, CiChecksHandle);
write_table!(
    virtual_branches_mut,
    virtual_branches,
    VirtualBranchesHandleMut
);
write_table!(branch_order_mut, branch_order, BranchOrderHandleMut);
write_table!(
    hunk_assignments_mut,
    hunk_assignments,
    HunkAssignmentsHandleMut
);
write_table!(forge_reviews_mut, forge_reviews, ForgeReviewsHandleMut);
write_table!(ci_checks_mut, ci_checks, CiChecksHandleMut);
write_single_table!(worktree_meta_mut, worktree_meta, WorktreeMetaHandleMut);
write_single_table!(
    gerrit_metadata_mut,
    gerrit_metadata,
    GerritMetadataHandleMut
);
