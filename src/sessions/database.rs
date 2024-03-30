use anyhow::{Context, Result};

use super::session::{self, SessionId};
use crate::{database, projects::ProjectId};

#[derive(Clone)]
pub struct Database {
    database: database::Database,
}

impl Database {
    pub fn new(database: database::Database) -> Database {
        Database { database }
    }

    pub fn insert(&self, project_id: &ProjectId, sessions: &[&session::Session]) -> Result<()> {
        self.database.transaction(|tx| -> Result<()> {
            let mut stmt = insert_stmt(tx).context("Failed to prepare insert statement")?;
            for session in sessions {
                stmt.execute(rusqlite::named_params! {
                    ":id": session.id,
                    ":project_id": project_id,
                    ":hash": session.hash.map(|hash| hash.to_string()),
                    ":branch": session.meta.branch,
                    ":commit": session.meta.commit,
                    ":start_timestamp_ms": session.meta.start_timestamp_ms.to_string(),
                    ":last_timestamp_ms": session.meta.last_timestamp_ms.to_string(),
                })
                .context("Failed to execute insert statement")?;
            }
            Ok(())
        })?;

        Ok(())
    }

    pub fn list_by_project_id(
        &self,
        project_id: &ProjectId,
        earliest_timestamp_ms: Option<u128>,
    ) -> Result<Vec<session::Session>> {
        self.database.transaction(|tx| {
            let mut stmt = list_by_project_id_stmt(tx)
                .context("Failed to prepare list_by_project_id statement")?;
            let mut rows = stmt
                .query(rusqlite::named_params! {
                    ":project_id": project_id,
                })
                .context("Failed to execute list_by_project_id statement")?;

            let mut sessions = Vec::new();
            while let Some(row) = rows
                .next()
                .context("Failed to iterate over list_by_project_id results")?
            {
                let session = parse_row(row)?;

                if let Some(earliest_timestamp_ms) = earliest_timestamp_ms {
                    if session.meta.last_timestamp_ms < earliest_timestamp_ms {
                        continue;
                    }
                }

                sessions.push(session);
            }
            Ok(sessions)
        })
    }

    pub fn get_by_project_id_id(
        &self,
        project_id: &ProjectId,
        id: &SessionId,
    ) -> Result<Option<session::Session>> {
        self.database.transaction(|tx| {
            let mut stmt = get_by_project_id_id_stmt(tx)
                .context("Failed to prepare get_by_project_id_id statement")?;
            let mut rows = stmt
                .query(rusqlite::named_params! {
                    ":project_id": project_id,
                    ":id": id,
                })
                .context("Failed to execute get_by_project_id_id statement")?;
            if let Some(row) = rows
                .next()
                .context("Failed to iterate over get_by_project_id_id results")?
            {
                Ok(Some(parse_row(row)?))
            } else {
                Ok(None)
            }
        })
    }

    pub fn get_by_id(&self, id: &SessionId) -> Result<Option<session::Session>> {
        self.database.transaction(|tx| {
            let mut stmt = get_by_id_stmt(tx).context("Failed to prepare get_by_id statement")?;
            let mut rows = stmt
                .query(rusqlite::named_params! {
                    ":id": id,
                })
                .context("Failed to execute get_by_id statement")?;
            if let Some(row) = rows
                .next()
                .context("Failed to iterate over get_by_id results")?
            {
                Ok(Some(parse_row(row)?))
            } else {
                Ok(None)
            }
        })
    }
}

fn parse_row(row: &rusqlite::Row) -> Result<session::Session> {
    Ok(session::Session {
        id: row.get(0).context("Failed to get id")?,
        hash: row
            .get::<usize, Option<String>>(2)
            .context("Failed to get hash")?
            .map(|hash| hash.parse().context("Failed to parse hash"))
            .transpose()?,
        meta: session::Meta {
            branch: row.get(3).context("Failed to get branch")?,
            commit: row.get(4).context("Failed to get commit")?,
            start_timestamp_ms: row
                .get::<usize, String>(5)
                .context("Failed to get start_timestamp_ms")?
                .parse()
                .context("Failed to parse start_timestamp_ms")?,
            last_timestamp_ms: row
                .get::<usize, String>(6)
                .context("Failed to get last_timestamp_ms")?
                .parse()
                .context("Failed to parse last_timestamp_ms")?,
        },
    })
}

fn list_by_project_id_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "SELECT `id`, `project_id`, `hash`, `branch`, `commit`, `start_timestamp_ms`, `last_timestamp_ms` FROM `sessions` WHERE `project_id` = :project_id ORDER BY `start_timestamp_ms` DESC",
    )?)
}

fn get_by_project_id_id_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "SELECT `id`, `project_id`, `hash`, `branch`, `commit`, `start_timestamp_ms`, `last_timestamp_ms` FROM `sessions` WHERE `project_id` = :project_id AND `id` = :id",
    )?)
}

fn get_by_id_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "SELECT `id`, `project_id`, `hash`, `branch`, `commit`, `start_timestamp_ms`, `last_timestamp_ms` FROM `sessions` WHERE `id` = :id",
    )?)
}

fn insert_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "INSERT INTO 'sessions' (
            `id`, `project_id`, `hash`, `branch`, `commit`, `start_timestamp_ms`, `last_timestamp_ms`
        ) VALUES (
            :id, :project_id, :hash, :branch, :commit, :start_timestamp_ms, :last_timestamp_ms
        ) ON CONFLICT(`id`) DO UPDATE SET
            `project_id` = :project_id,
            `hash` = :hash,
            `branch` = :branch,
            `commit` = :commit,
            `start_timestamp_ms` = :start_timestamp_ms,
            `last_timestamp_ms` = :last_timestamp_ms
        ",
    )?)
}
