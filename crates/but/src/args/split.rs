use crate::args::atoms::{AllowMergedArg, CliIdArg};

/// Split a commit in two.
///
/// Sources must all be committed changes from the same commit. Files and hunks may be mixed.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// The message to use for the new commit.
    ///
    /// Can be supplied any amount of times, each value being appended to the preceding ones with a
    /// blank line in between. Without `-m`, the new commit will get an empty message.
    #[clap(short, long)]
    pub message: Option<Vec<String>>,

    /// The committed files and hunks to move into a new commit.
    #[clap(required = true)]
    pub sources: Vec<CliIdArg>,

    #[clap(flatten)]
    #[allow(missing_docs)]
    pub allow_merged: AllowMergedArg,
}
