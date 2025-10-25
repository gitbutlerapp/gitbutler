use diesel::{
    ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl,
    prelude::{Insertable, Queryable, Selectable},
};
use serde::{Deserialize, Serialize};

use crate::{DbHandle, schema::gerrit_metadata::dsl::gerrit_metadata};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::gerrit_metadata)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct GerritMeta {
    /// Unique Gerrit change ID (primary key)
    pub change_id: String,
    /// Git commit ID
    pub commit_id: String,
    /// Gerrit review URL
    pub review_url: String,
    /// The time when the metadata was created
    pub created_at: chrono::NaiveDateTime,
    /// The time when the metadata was last updated
    pub updated_at: chrono::NaiveDateTime,
}

impl DbHandle {
    pub fn gerrit_metadata(&mut self) -> GerritMetadataHandle<'_> {
        GerritMetadataHandle { db: self }
    }
}

pub struct GerritMetadataHandle<'a> {
    db: &'a mut DbHandle,
}

impl GerritMetadataHandle<'_> {
    /// Get a GerritMeta entry by change_id (primary key)
    pub fn get(&mut self, change_id: &str) -> anyhow::Result<Option<GerritMeta>> {
        use crate::schema::gerrit_metadata::change_id as change_id_col;

        let result = gerrit_metadata
            .filter(change_id_col.eq(change_id))
            .first::<GerritMeta>(&mut self.db.conn)
            .optional()?;

        Ok(result)
    }

    /// Insert a new GerritMeta entry
    pub fn insert(&mut self, meta: GerritMeta) -> anyhow::Result<()> {
        diesel::insert_into(gerrit_metadata)
            .values(&meta)
            .execute(&mut self.db.conn)?;
        Ok(())
    }

    /// Update an existing GerritMeta entry
    pub fn update(&mut self, meta: GerritMeta) -> anyhow::Result<()> {
        use crate::schema::gerrit_metadata::{change_id, commit_id, review_url, updated_at};

        diesel::update(gerrit_metadata.filter(change_id.eq(&meta.change_id)))
            .set((
                commit_id.eq(&meta.commit_id),
                review_url.eq(&meta.review_url),
                updated_at.eq(&meta.updated_at),
            ))
            .execute(&mut self.db.conn)?;
        Ok(())
    }
}
