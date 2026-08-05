//! Arguments for `undo`.

#![deny(missing_docs)]

/// Undo the last operation.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {}
