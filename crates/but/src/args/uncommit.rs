//! Arguments for `uncommit`.

#![deny(missing_docs)]

use crate::args::atoms::{AllowMergedArg, CliIdArg};

/// Uncommit changes from commits or committed files to the uncommitted area.
///
/// For more details about CLI IDs, see `but help cli-ids`.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// One or more commits or committed files to uncommit.
    ///
    /// A whole commit is uncommitted by its commit ID; a single file is uncommitted with
    /// `<commit-id>:<file-id>`.
    #[clap(required = true)]
    pub sources: Vec<CliIdArg>,

    #[clap(flatten)]
    #[allow(missing_docs)]
    pub allow_merged: AllowMergedArg,
}
