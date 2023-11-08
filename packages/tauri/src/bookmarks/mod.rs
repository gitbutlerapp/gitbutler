mod database;
mod reader;
mod writer;

pub mod commands;

mod controller;
pub use controller::Controller;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub project_id: ProjectId,
    pub timestamp_ms: u128,
    pub created_timestamp_ms: u128,
    pub updated_timestamp_ms: u128,
    pub note: String,
    pub deleted: bool,
}

pub use reader::BookmarksReader as Reader;
pub use writer::BookmarksWriter as Writer;

use crate::projects::ProjectId;
