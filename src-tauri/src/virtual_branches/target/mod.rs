mod reader;
mod writer;

pub use reader::{TargetReadError as ReadError, TargetReader as Reader};
pub use writer::TargetWriter as Writer;

#[derive(Debug, PartialEq, Clone)]
pub struct Target {
    pub name: String,
    pub remote: String,
    pub sha: git2::Oid,
}
