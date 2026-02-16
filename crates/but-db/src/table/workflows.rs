use serde::{Deserialize, Serialize};

use crate::{DbHandle, M, Transaction};

pub(crate) const M: &[M<'static>] = &[M::up(
    20250619192246,
    "CREATE TABLE `workflows`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`created_at` TIMESTAMP NOT NULL,
	`kind` TEXT NOT NULL,
	`triggered_by` TEXT NOT NULL,
	`status` TEXT NOT NULL,
	`input_commits` TEXT NOT NULL,
	`output_commits` TEXT NOT NULL,
	`summary` TEXT
);",
)];
/// Tests are in `but-db/tests/db/table/workflows.rs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workflow {
    /// Unique identifier for the workflow.
    pub id: String,
    /// The time when the workflow was captured.
    pub created_at: chrono::NaiveDateTime,
    /// The type of the workflow performed.
    pub kind: String,
    /// The trigger that initiated the workflow.
    pub triggered_by: String,
    /// The status of the workflow.
    pub status: String,
    /// Input commits
    pub input_commits: String,
    /// Output commits
    pub output_commits: String,
    /// Optional summary of the workflow
    pub summary: Option<String>,
}

impl DbHandle {
    pub fn workflows(&self) -> WorkflowsHandle<'_> {
        WorkflowsHandle { conn: &self.conn }
    }

    pub fn workflows_mut(&mut self) -> WorkflowsHandleMut<'_> {
        WorkflowsHandleMut { conn: &self.conn }
    }
}

impl<'conn> Transaction<'conn> {
    pub fn workflows(&self) -> WorkflowsHandle<'_> {
        WorkflowsHandle { conn: self.inner() }
    }

    pub fn workflows_mut(&mut self) -> WorkflowsHandleMut<'_> {
        WorkflowsHandleMut { conn: self.inner() }
    }
}

pub struct WorkflowsHandle<'conn> {
    conn: &'conn rusqlite::Connection,
}

pub struct WorkflowsHandleMut<'conn> {
    conn: &'conn rusqlite::Connection,
}

impl WorkflowsHandle<'_> {
    /// Lists workflows with pagination.
    /// Returns (total_count, workflows).
    pub fn list(&self, offset: i64, limit: i64) -> rusqlite::Result<(i64, Vec<Workflow>)> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, kind, triggered_by, status, input_commits, output_commits, summary \
             FROM workflows ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
        )?;

        let workflows = stmt
            .query_map([limit, offset], |row| {
                Ok(Workflow {
                    id: row.get(0)?,
                    created_at: row.get(1)?,
                    kind: row.get(2)?,
                    triggered_by: row.get(3)?,
                    status: row.get(4)?,
                    input_commits: row.get(5)?,
                    output_commits: row.get(6)?,
                    summary: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let total: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM workflows", [], |row| row.get(0))?;

        Ok((total, workflows))
    }
}

impl WorkflowsHandleMut<'_> {
    /// Enable read-only access functions.
    pub fn to_ref(&self) -> WorkflowsHandle<'_> {
        WorkflowsHandle { conn: self.conn }
    }

    /// Insert a new workflow.
    pub fn insert(&mut self, workflow: Workflow) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO workflows (id, created_at, kind, triggered_by, status, input_commits, output_commits, summary) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                workflow.id,
                workflow.created_at,
                workflow.kind,
                workflow.triggered_by,
                workflow.status,
                workflow.input_commits,
                workflow.output_commits,
                workflow.summary,
            ],
        )?;
        Ok(())
    }
}
