//! Arguments for `uncommit`.

#![deny(missing_docs)]

use crate::args::atoms::{AllowMergedArg, CliIdArg};

/// Uncommit commits, branches, or committed files.
///
/// For more details about CLI IDs, see `but help cli-ids`.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// One or more commits, branches, or committed files to uncommit.
    ///
    /// Sources must all be the same kind.
    #[clap(required = true)]
    pub sources: Vec<CliIdArg>,

    #[clap(flatten)]
    #[allow(missing_docs)]
    pub allow_merged: AllowMergedArg,
}
