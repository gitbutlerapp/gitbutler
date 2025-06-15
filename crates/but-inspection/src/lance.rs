use std::sync::Arc;

use anyhow::Result;
use arrow_array::{
    FixedSizeListArray, RecordBatch, RecordBatchIterator, StringArray, types::Float32Type,
};
use bstr::ByteSlice;
use gitbutler_command_context::CommandContext;
use lancedb::{
    Connection,
    arrow::arrow_schema::{DataType, Field, Schema},
    index::Index,
};
use tracing::instrument;

use crate::db::{Commit, Hunk};

fn hunk_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("oid", DataType::Utf8, false),
        Field::new("header", DataType::Utf8, false),
        Field::new("path", DataType::Utf8, false),
        Field::new("previous_path", DataType::Utf8, true),
        Field::new(
            "vector",
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, false)), 1536),
            false,
        ),
    ]))
}

fn commit_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![Field::new("oid", DataType::Utf8, false)]))
}

pub struct LanceHandle {
    /// The connection to the LanceDB database
    conn: Connection,
}

impl LanceHandle {
    /// Create a new connection to the LanceDB database and run any migrations
    pub async fn try_new(ctx: &CommandContext) -> Result<Self> {
        let path = ctx.project().gb_dir().join("lance").canonicalize()?;
        let db = lancedb::connect(&path.to_string_lossy()).execute().await?;
        let myself = Self { conn: db };
        // We want the tables to be created/migrated before use
        myself.create_tables().await?;
        Ok(myself)
    }

    /// Create the tables and indexes
    async fn create_tables(&self) -> Result<()> {
        // We probably will want some sort of migration system here

        let tables = self.conn.table_names().execute().await?;

        if !tables.contains(&"hunks".to_string()) {
            let table = self
                .conn
                .create_empty_table("hunks", hunk_schema())
                .execute()
                .await?;

            table
                .create_index(&["vector", "oid"], Index::Auto)
                .execute()
                .await?;
        }

        // Used to indicate which commits have been embedded
        if !tables.contains(&"commits".to_string()) {
            let table = self
                .conn
                .create_empty_table("commits", commit_schema())
                .execute()
                .await?;

            table.create_index(&["oid"], Index::Auto).execute().await?;
        }

        Ok(())
    }

    #[instrument(skip_all)]
    pub async fn upsert_many_hunks(&self, entries: &[Hunk]) -> Result<Vec<Hunk>> {
        let hunks = self.conn.open_table("hunks").execute().await?;

        let records = RecordBatchIterator::new(
            entries
                .iter()
                .map(|hunk| {
                    let batch = RecordBatch::try_new(
                        hunk_schema(),
                        vec![
                            Arc::new(StringArray::from(vec![hunk.oid.to_string()])),
                            Arc::new(StringArray::from(vec![hunk.header.clone()])),
                            Arc::new(StringArray::from(vec![
                                hunk.path.to_str_lossy().to_string(),
                            ])),
                            Arc::new(StringArray::from(vec![
                                hunk.previous_path
                                    .as_ref()
                                    .map(|p| p.to_str_lossy().to_string()),
                            ])),
                            Arc::new(
                                FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
                                    [Some(
                                        hunk.vector
                                            .clone()
                                            .into_iter()
                                            .map(Some)
                                            .collect::<Vec<_>>(),
                                    )],
                                    1,
                                ),
                            ),
                        ],
                    )?;
                    Ok(Ok(batch))
                })
                .collect::<Result<Vec<_>>>()?,
            hunk_schema(),
        );

        let mut merge = hunks.merge_insert(&["oid", "header", "path", "previous_path"]);
        merge.when_not_matched_insert_all();
        merge.when_matched_update_all(None);
        merge.execute(Box::new(records)).await?;

        self.upsert_many_commits(
            &entries
                .iter()
                .map(|hunk| Commit { oid: hunk.oid })
                .collect::<Vec<_>>(),
        )
        .await?;

        Ok(vec![])
    }

    pub async fn upsert_many_commits(&self, entries: &[Commit]) -> Result<Vec<Commit>> {
        let commits = self.conn.open_table("commits").execute().await?;

        let records = RecordBatchIterator::new(
            entries
                .iter()
                .map(|commit| {
                    let batch = RecordBatch::try_new(
                        commit_schema(),
                        vec![Arc::new(StringArray::from(vec![commit.oid.to_string()]))],
                    )?;
                    Ok(Ok(batch))
                })
                .collect::<Result<Vec<_>>>()?,
            commit_schema(),
        );

        let mut merge = commits.merge_insert(&["oid"]);
        merge.when_not_matched_insert_all();
        merge.when_matched_update_all(None);
        merge.execute(Box::new(records)).await?;

        Ok(vec![])
    }
}
