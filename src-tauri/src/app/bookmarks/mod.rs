mod database;

use serde::Serialize;

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub id: String,
    pub project_id: String,
    pub timestamp_ms: u128,
    pub note: String,
}

pub use database::Database;
