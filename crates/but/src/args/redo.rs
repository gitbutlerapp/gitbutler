//! Arguments for `redo`.

#![deny(missing_docs)]

/// Redo the last undo.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {}
