//! Arguments for `_reword2`.

#![deny(missing_docs)]

use crate::args::atoms::{AllowMergedArg, CliIdArg};

/// Edit a commit message or rename a branch.
///
/// If no message is provided, an editor opens. Rewording a commit recreates it and rebases its
/// dependent commits.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// The commit whose message should be edited, or the branch to rename.
    pub target: CliIdArg,

    /// The new commit message or branch name. If omitted, an editor opens.
    #[clap(short = 'm', long = "message", conflicts_with = "fix_formatting")]
    pub message: Option<String>,

    /// Format the existing commit message to 72-character line wrapping without opening an editor.
    #[clap(
        id = "fix_formatting",
        short = 'f',
        long = "fix-formatting",
        conflicts_with = "message"
    )]
    pub format: bool,

    #[clap(flatten)]
    #[expect(missing_docs)]
    pub allow_merged: AllowMergedArg,
}
