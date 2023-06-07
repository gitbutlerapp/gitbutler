mod reader;
mod writer;

pub use reader::{BranchReadError as ReadError, BranchReader as Reader};
pub use writer::BranchWriter as Writer;

#[derive(Debug, PartialEq, Clone)]
pub struct Branch {
    pub id: String,
    pub name: String,
    pub applied: bool,
    pub upstream: String,
    pub created_timestamp_ms: u128,
    pub updated_timestamp_ms: u128,
}
