use diesel::{
    RunQueryDsl,
    prelude::{Insertable, Queryable, Selectable},
};
use serde::{Deserialize, Serialize};

use crate::DbHandle;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::forge_reviews)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ForgeReview {
    pub html_url: String,
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub author: Option<String>,
    pub labels: String,
    pub draft: bool,
    pub source_branch: String,
    pub target_branch: String,
    pub sha: String,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub modified_at: Option<chrono::NaiveDateTime>,
    pub merged_at: Option<chrono::NaiveDateTime>,
    pub closed_at: Option<chrono::NaiveDateTime>,
    pub repository_ssh_url: Option<String>,
    pub repository_https_url: Option<String>,
    pub repo_owner: Option<String>,
    pub reviewers: String,
    pub unit_symbol: String,
    pub last_sync_at: chrono::NaiveDateTime,
    pub struct_version: i32,
}

impl DbHandle {
    pub fn forge_reviews(&mut self) -> ForgeReviewsHandle<'_> {
        ForgeReviewsHandle { db: self }
    }
}
pub struct ForgeReviewsHandle<'a> {
    db: &'a mut DbHandle,
}

impl ForgeReviewsHandle<'_> {
    /// Lists all forge reviews in the database.
    pub fn list_all(&mut self) -> anyhow::Result<Vec<ForgeReview>> {
        use crate::schema::forge_reviews::dsl::forge_reviews as all_reviews;
        let results = all_reviews.load::<ForgeReview>(&mut self.db.conn)?;
        Ok(results)
    }
    // Sets the forge_reviews table to the provided values.
    // Any existing entries that are not in the provided values are deleted.
    pub fn set_all(&mut self, reviews: Vec<ForgeReview>) -> anyhow::Result<()> {
        use crate::schema::forge_reviews::dsl::forge_reviews as all_reviews;
        use diesel::prelude::*;

        self.db.conn.transaction(|conn| {
            diesel::delete(all_reviews).execute(conn)?;
            for review in reviews {
                diesel::insert_into(all_reviews)
                    .values(&review)
                    .execute(conn)?;
            }
            diesel::result::QueryResult::Ok(())
        })?;
        Ok(())
    }
}
