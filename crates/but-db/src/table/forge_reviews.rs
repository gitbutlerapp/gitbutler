use serde::{Deserialize, Serialize};

use crate::{DbHandle, M, Transaction};

pub(crate) const M: &[M<'static>] = &[M::up(
    20260101223932,
    "-- Your SQL goes here
CREATE TABLE `forge_reviews`(
	`html_url` TEXT NOT NULL,
	`number` BIGINT NOT NULL PRIMARY KEY,
	`title` TEXT NOT NULL,
	`body` TEXT,
	`author` TEXT,
	`labels` TEXT NOT NULL,
	`draft` BOOL NOT NULL,
	`source_branch` TEXT NOT NULL,
	`target_branch` TEXT NOT NULL,
	`sha` TEXT NOT NULL,
	`created_at` TIMESTAMP,
	`modified_at` TIMESTAMP,
	`merged_at` TIMESTAMP,
	`closed_at` TIMESTAMP,
	`repository_ssh_url` TEXT,
	`repository_https_url` TEXT,
	`repo_owner` TEXT,
	`reviewers` TEXT NOT NULL,
	`unit_symbol` TEXT NOT NULL,
	`last_sync_at` TIMESTAMP NOT NULL,
	`struct_version` INTEGER NOT NULL
);",
)];

/// Tests are in `but-db/tests/db/table/forge_reviews.rs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    pub fn forge_reviews(&self) -> ForgeReviewsHandle<'_> {
        ForgeReviewsHandle { conn: &self.conn }
    }

    pub fn forge_reviews_mut(&mut self) -> rusqlite::Result<ForgeReviewsHandleMut<'_>> {
        Ok(ForgeReviewsHandleMut {
            sp: self.conn.savepoint()?,
        })
    }
}

impl<'conn> Transaction<'conn> {
    pub fn forge_reviews(&self) -> ForgeReviewsHandle<'_> {
        ForgeReviewsHandle { conn: self.inner() }
    }

    pub fn forge_reviews_mut(&mut self) -> rusqlite::Result<ForgeReviewsHandleMut<'_>> {
        Ok(ForgeReviewsHandleMut {
            sp: self.inner_mut().savepoint()?,
        })
    }
}

pub struct ForgeReviewsHandle<'conn> {
    conn: &'conn rusqlite::Connection,
}

pub struct ForgeReviewsHandleMut<'conn> {
    sp: rusqlite::Savepoint<'conn>,
}

impl ForgeReviewsHandle<'_> {
    /// Lists all forge reviews in the database.
    pub fn list_all(&self) -> rusqlite::Result<Vec<ForgeReview>> {
        let mut stmt = self.conn.prepare(
            "SELECT html_url, number, title, body, author, labels, draft, source_branch, \
             target_branch, sha, created_at, modified_at, merged_at, closed_at, \
             repository_ssh_url, repository_https_url, repo_owner, reviewers, unit_symbol, \
             last_sync_at, struct_version FROM forge_reviews",
        )?;

        let results = stmt.query_map([], |row| {
            Ok(ForgeReview {
                html_url: row.get(0)?,
                number: row.get(1)?,
                title: row.get(2)?,
                body: row.get(3)?,
                author: row.get(4)?,
                labels: row.get(5)?,
                draft: row.get(6)?,
                source_branch: row.get(7)?,
                target_branch: row.get(8)?,
                sha: row.get(9)?,
                created_at: row.get(10)?,
                modified_at: row.get(11)?,
                merged_at: row.get(12)?,
                closed_at: row.get(13)?,
                repository_ssh_url: row.get(14)?,
                repository_https_url: row.get(15)?,
                repo_owner: row.get(16)?,
                reviewers: row.get(17)?,
                unit_symbol: row.get(18)?,
                last_sync_at: row.get(19)?,
                struct_version: row.get(20)?,
            })
        })?;

        results.collect::<Result<Vec<_>, _>>()
    }
}

impl ForgeReviewsHandleMut<'_> {
    /// Enable read-only access functions.
    pub fn to_ref(&self) -> ForgeReviewsHandle<'_> {
        ForgeReviewsHandle { conn: &self.sp }
    }

    /// Sets the forge_reviews table to the provided values.
    /// Any existing entries that are not in the provided values are deleted.
    pub fn set_all(self, reviews: Vec<ForgeReview>) -> rusqlite::Result<()> {
        self.sp.execute("DELETE FROM forge_reviews", [])?;

        for review in reviews {
            self.sp.execute(
                "INSERT INTO forge_reviews (html_url, number, title, body, author, labels, draft, \
                 source_branch, target_branch, sha, created_at, modified_at, merged_at, closed_at, \
                 repository_ssh_url, repository_https_url, repo_owner, reviewers, unit_symbol, \
                 last_sync_at, struct_version) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
                rusqlite::params![
                    review.html_url,
                    review.number,
                    review.title,
                    review.body,
                    review.author,
                    review.labels,
                    review.draft,
                    review.source_branch,
                    review.target_branch,
                    review.sha,
                    review.created_at,
                    review.modified_at,
                    review.merged_at,
                    review.closed_at,
                    review.repository_ssh_url,
                    review.repository_https_url,
                    review.repo_owner,
                    review.reviewers,
                    review.unit_symbol,
                    review.last_sync_at,
                    review.struct_version,
                ],
            )?;
        }

        self.sp.commit()?;
        Ok(())
    }
}
