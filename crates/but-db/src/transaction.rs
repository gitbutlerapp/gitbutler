use rusqlite::{ErrorCode, TransactionBehavior};

use crate::{AppCacheHandle, DbHandle, Transaction, migration::BUSY_TIMEOUT};

impl<'conn> From<rusqlite::Transaction<'conn>> for Transaction<'conn> {
    fn from(trans: rusqlite::Transaction<'conn>) -> Self {
        Transaction {
            inner: Some(trans),
            reset_to_blocking_on_drop: false,
        }
    }
}

/// Helpers
impl<'conn> Transaction<'conn> {
    pub(crate) fn inner(&self) -> &rusqlite::Transaction<'conn> {
        self.inner
            .as_ref()
            .expect("BUG: transaction is always set while alive")
    }

    pub(crate) fn inner_mut(&mut self) -> &mut rusqlite::Transaction<'conn> {
        self.inner
            .as_mut()
            .expect("BUG: transaction is always set while alive")
    }
}

/// Transactions
impl DbHandle {
    /// Create a new *deferred* transaction which can be used to create new table-handles on.
    /// *Deferred* means that the transaction does not block other writers until the first
    /// write actually happens, and hold the database lock while it is held.
    /// It will, however, freeze what's read to the current state of the database, so changes
    /// won't be observable until commit/rollback.
    /// Readers will always read from the original data.
    ///
    /// When used while a lock is taken elsewhere, *any read or write at a later time will block at first*,
    /// and fail after a timeout.
    ///
    /// # IMPORTANT: run `commit()`
    /// Don't forget to call [commit()](Transaction::commit()) to actually persist the result.
    /// On drop, no changes will be persisted and the transaction is implicitly rolled back.
    pub fn transaction(&mut self) -> rusqlite::Result<Transaction<'_>> {
        Ok(self
            .conn
            .transaction_with_behavior(TransactionBehavior::Deferred)?
            .into())
    }

    /// Create a new *immediate* transaction which can be used to create new table-handles on,
    /// preventing all writes to the entire database while it is held, or block while the database lock
    /// is held elsewhere, failing after a timeout.
    /// Readers will always read from the original data.
    /// It will freeze what's read to the current state of the database, so changes
    /// won't be observable until commit/rollback.
    ///
    /// When *called* while a lock is taken elsewhere, this call will block at first, and fail after a timeout.
    ///
    /// # IMPORTANT: run `commit()`
    /// Don't forget to call [commit()](Transaction::commit()) to actually persist the result.
    /// On drop, no changes will be persisted and the transaction is implicitly rolled back.
    pub fn immediate_transaction(&mut self) -> rusqlite::Result<Transaction<'_>> {
        Ok(self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?
            .into())
    }

    /// Create a new *immediate* transaction which can be used to create new table-handles on,
    /// preventing all writes to the entire database while it is held, or return `None` while the database lock
    /// is held elsewhere.
    /// It will freeze what's read to the current state of the database, so changes
    /// won't be observable until commit/rollback.
    /// Readers will always read from the original data.
    ///
    /// # IMPORTANT: run `commit()`
    /// Don't forget to call [commit()](Transaction::commit()) to actually persist the result.
    /// On drop, no changes will be persisted and the transaction is implicitly rolled back.
    pub fn immediate_transaction_nonblocking(
        &mut self,
    ) -> rusqlite::Result<Option<Transaction<'_>>> {
        immediate_optional_transaction(&mut self.conn)
    }
}

/// Transactions
impl AppCacheHandle {
    /// Create a new *deferred* transaction which can be used to create new table-handles on.
    /// *Deferred* means that the transaction does not block other writers until the first
    /// write actually happens, and hold the database lock while it is held.
    /// It will, however, freeze what's read to the current state of the database, so changes
    /// won't be observable until commit/rollback.
    /// Readers will always read from the original data.
    ///
    /// # IMPORTANT: run `commit()`
    /// Don't forget to call [commit()](Transaction::commit()) to actually persist the result.
    /// On drop, no changes will be persisted and the transaction is implicitly rolled back.
    pub fn deferred_transaction(&mut self) -> rusqlite::Result<Transaction<'_>> {
        Ok(self
            .conn
            .transaction_with_behavior(TransactionBehavior::Deferred)?
            .into())
    }

    /// Create a new *immediate* transaction which can be used to create new table-handles on,
    /// preventing all writes to the entire database while it is held, or return `None` while the database lock
    /// is held elsewhere.
    /// It will freeze what's read to the current state of the database, so changes
    /// won't be observable until commit/rollback.
    /// Readers will always read from the original data.
    ///
    /// # IMPORTANT: run `commit()`
    /// Don't forget to call [commit()](Transaction::commit()) to actually persist the result.
    /// On drop, no changes will be persisted and the transaction is implicitly rolled back.
    pub fn immediate_transaction_nonblocking(
        &mut self,
    ) -> rusqlite::Result<Option<Transaction<'_>>> {
        immediate_optional_transaction(&mut self.conn)
    }
}

fn immediate_optional_transaction(
    conn: &mut rusqlite::Connection,
) -> rusqlite::Result<Option<Transaction<'_>>> {
    set_connection_to_nonblocking(conn)?;

    // TODO(borrowchk): remove this once Rust can handle this case.
    // SAFETY: We need to reset the connection in both success and error cases.
    // In the success case, the transaction borrows `conn` mutably for its lifetime,
    // preventing us from accessing `conn` in the error case as `match` doesn't shorten the lifetime.
    // We create a raw pointer here that we can use in the error path.
    let conn_ptr = conn as *const rusqlite::Connection;

    match conn.transaction_with_behavior(TransactionBehavior::Immediate) {
        Ok(trans) => {
            // The transaction won't block anymore as we have the lock, so it's
            // OK to reset the blocking behavior later.
            let mut trans = Transaction::from(trans);
            trans.reset_to_blocking_on_drop = true;
            Ok(Some(trans))
        }
        Err(err) => {
            // SAFETY: The transaction creation failed, so the mutable borrow is dropped.
            // We can safely access the connection again.
            unsafe {
                reset_connection_to_blocking(&*conn_ptr)?;
            }

            if err.sqlite_error_code().is_some_and(|code| {
                matches!(code, ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
            }) {
                Ok(None)
            } else {
                Err(err)
            }
        }
    }
}

/// Operations
impl Transaction<'_> {
    /// Change the *connection* underlying the transaction to be non-blocking,
    /// i.e. trying to get a lock on the database will fail immediately if it is already locked, without waiting
    /// to acquire it later.
    ///
    /// This is automatically undone when this instance is dropped.
    pub fn set_nonblocking(&mut self) -> rusqlite::Result<()> {
        set_connection_to_nonblocking(self.inner())?;
        self.reset_to_blocking_on_drop = true;
        Ok(())
    }

    /// Consume the transaction and commit it, without recovery.
    pub fn commit(mut self) -> Result<(), rusqlite::Error> {
        let res = self.reset_connection_to_blocking_if_needed();
        self.inner
            .take()
            .expect("BUG: always set")
            .commit()
            .and(res)
    }

    /// Roll all changes so far back, making this instance unusable.
    pub fn rollback(mut self) -> Result<(), rusqlite::Error> {
        let res = self.reset_connection_to_blocking_if_needed();
        self.inner
            .take()
            .expect("BUG: always set")
            .rollback()
            .and(res)
    }

    fn reset_connection_to_blocking_if_needed(&mut self) -> rusqlite::Result<()> {
        if let Some(trans) = self
            .inner
            .as_ref()
            .filter(|_| self.reset_to_blocking_on_drop)
        {
            reset_connection_to_blocking(trans)
        } else {
            Ok(())
        }
    }
}

fn reset_connection_to_blocking(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    conn.busy_timeout(BUSY_TIMEOUT)
}

fn set_connection_to_nonblocking(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    conn.busy_timeout(std::time::Duration::from_millis(0))
}

impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        self.reset_connection_to_blocking_if_needed().ok();
    }
}
