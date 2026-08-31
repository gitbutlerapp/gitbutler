use crate::args::atoms::{AllowMergedArg, CliIdArg};

/// Split a commit in two.
///
/// Sources must all be committed files from the same commit.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// The committed files to move into a new commit.
    #[clap(required = true)]
    pub sources: Vec<CliIdArg>,

    #[clap(flatten)]
    #[allow(missing_docs)]
    pub allow_merged: AllowMergedArg,
}
