use anyhow::Result;
use rusqlite::Connection;

pub(crate) struct Migration {
    /// A unique identifier for the migration
    pub name: String,
    /// A function which performs the migration. The up function gets run inside
    /// of a transaction.
    pub up: fn(&Connection) -> Result<()>,
}
