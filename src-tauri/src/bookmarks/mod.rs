mod database;
mod reader;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub id: String,
    pub project_id: String,
    pub created_timestamp_ms: u128,
    pub updated_timestamp_ms: u128,
    pub note: String,
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Event {
    Created(Bookmark),
    Updated {
        id: String,
        note: Option<String>,
        deleted: Option<bool>,
        timestamp_ms: u128,
    },
}

pub use database::Database;
pub use reader::BookmarksReader as Reader;
