use crate::args::atoms::{AllowMergedArg, CliIdArg};

/// Split a commit in two.
///
/// Sources must all be committed changes from the same commit. Files and hunks may be mixed.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// The committed files and hunks to move into a new commit.
    #[clap(required = true)]
    pub sources: Vec<CliIdArg>,

    #[clap(flatten)]
    #[allow(missing_docs)]
    pub allow_merged: AllowMergedArg,
}
