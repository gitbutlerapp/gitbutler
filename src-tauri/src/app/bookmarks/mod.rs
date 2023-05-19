mod database;
mod reader;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub id: String,
    pub project_id: String,
    pub timestamp_ms: u128,
    pub note: String,
}

pub use database::Database;
pub use reader::BookmarksReader as Reader;
