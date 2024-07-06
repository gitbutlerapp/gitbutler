use anyhow::{Context, Ok, Result};
use rusqlite::{Connection, TransactionBehavior};
use uuid::Uuid;

use super::migration::Migration;

/// SQL required to create the migrations table.
///
/// This should only ever be changed with great caution
const CREATE_MIGRATIONS_TABLE: &str = "
CREATE TABLE IF NOT EXISTS migrations (
    id BLOB PRIMARY KEY NOT NULL, -- A V4 UUID
    name TEXT NOT NULL
)
";

/// Migrator is used to perform migrations on a particular database.
pub(crate) struct Migrator<'l> {
    connection: &'l mut Connection,
}

impl<'l> Migrator<'l> {
    pub(crate) fn new(connection: &'l mut Connection) -> Migrator<'l> {
        Migrator { connection }
    }

    /// Iterates over the list of provided migrations starting at index 0.
    /// If the migration has already been run, it will be skipped.
    /// Run migrations get recorded in the `migrations` table.
    /// The `migrations` table will be created if it doesn't exist.
    pub(crate) fn migrate(&mut self, migrations: Vec<Migration>) -> Result<()> {
        let applied_migrations = self.find_applied_migrations()?;

        for migration in migrations {
            // Don't try to reapply existing migrations
            if applied_migrations.contains(&migration.name) {
                continue;
            }

            // Scope for transaction
            {
                // Using an Immediate transactions as both reads and writes
                // may be performed in a migration.
                let transaction = self
                    .connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)?;
                (migration.up)(&transaction)
                    .context(format!("Failed to run migration {}", migration.name))?;

                transaction
                    .execute(
                        "INSERT INTO migrations (id, name) VALUES (?1, ?2)",
                        (Uuid::new_v4(), migration.name),
                    )
                    .context("Failed to insert migration completion marker")?;

                transaction.commit()?;
            }
        }

        Ok(())
    }

    /// Queries all the applied migrations.
    /// If the `migrations` table doesn't exist, it will be created here
    fn find_applied_migrations(&self) -> Result<Vec<String>> {
        self.connection
            .execute(CREATE_MIGRATIONS_TABLE, [])
            .context("Failed to create migrations table")?;

        let mut statement = self
            .connection
            .prepare("SELECT name FROM migrations")
            .context("Failed to fetch migrations")?;

        let mapped_rows = statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(mapped_rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_migration() -> Migration {
        Migration {
            name: "first".to_string(),
            up: |connection| {
                connection.execute(
                    "
                CREATE TABLE testaroni (
                    id BLOB PRIMARY KEY, -- A V4 UUID
                    name TEXT NOT NULL
                )
                ",
                    [],
                )?;

                Ok(())
            },
        }
    }

    /// Depends on `first_migration`
    fn second_migration() -> Migration {
        Migration {
            name: "second".to_string(),
            up: |connection| {
                connection.execute("ALTER TABLE testaroni ADD potatoes TEXT", [])?;

                Ok(())
            },
        }
    }

    #[test]
    /// Testing the `find_applied_migrations` function.
    /// Ensuring that it creates the `migrations` table if it doesn't exist
    fn find_applied_migrations_creates_migrations_table_if_not_exists() {
        let mut connection = Connection::open_in_memory().unwrap();

        let migrator = Migrator::new(&mut connection);

        // Try to find unapplied migrations
        migrator.find_applied_migrations().unwrap();

        let statement = connection
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='migrations'")
            .unwrap();
        assert_eq!(statement.column_count(), 1);
    }

    #[test]
    /// Testing the `find_applied_migrations` function.
    /// Assuming the `migrations` table exists (which we create by calling
    /// `find_applied_migrations), we expect that any values inserted into
    /// the table will get returned when we call it again.
    fn find_applied_migrations_lists_all_entries_in_table() {
        let mut connection = Connection::open_in_memory().unwrap();

        // Call find_applied_migrations in order to create the migrations table
        {
            let migrator = Migrator::new(&mut connection);
            migrator.find_applied_migrations().unwrap();
        }

        // Insert some entries into the migrations table
        connection
            .execute(
                "INSERT INTO migrations (id, name) VALUES (?1, ?2)",
                (Uuid::new_v4(), "base".to_string()),
            )
            .unwrap();

        connection
            .execute(
                "INSERT INTO migrations (id, name) VALUES (?1, ?2)",
                (Uuid::new_v4(), "other".to_string()),
            )
            .unwrap();

        {
            let migrator = Migrator::new(&mut connection);
            let results = migrator.find_applied_migrations().unwrap();
            assert_eq!(results.len(), 2);
            assert!(results.contains(&"base".to_string()));
            assert!(results.contains(&"other".to_string()));
        }
    }

    #[test]
    /// Testing the `migrate` function.
    /// When given a list of migrations to run, it will perform the migrations in order
    fn migrate_applies_migrations_in_order() {
        let mut connection = Connection::open_in_memory().unwrap();

        let mut migrator = Migrator::new(&mut connection);
        // Runs two migrations, one which creates a table and the other which laters it
        migrator
            .migrate(vec![first_migration(), second_migration()])
            .unwrap();
    }

    #[test]
    /// Testing the `migrate` function.
    /// When calling multiple times on the same database, it will only preform
    /// migrations that haven't been run before.
    ///
    /// Given the provided migrations, we would expect the second call to
    /// `migrate` to return Err if it was run twice.
    fn migrate_only_applies_migrations_once() {
        let mut connection = Connection::open_in_memory().unwrap();

        let mut migrator = Migrator::new(&mut connection);
        migrator.migrate(vec![first_migration()]).unwrap();

        migrator
            .migrate(vec![first_migration(), second_migration()])
            .unwrap();
    }
}
