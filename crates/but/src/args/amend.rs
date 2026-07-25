//! Arguments for `amend`.

#![deny(missing_docs)]

use crate::args::atoms::CliIdArg;

/// Amend uncommitted changes into a commit or branch.
///
/// Sources must be uncommitted files or hunks. To move changes that are already committed, or to
/// combine commits, use `but squash`.
///
/// If the target is a branch, the changes are amended into that branch's newest commit (its tip).
/// Name the commit explicitly to amend into anything below the tip.
///
/// For more details about CLI IDs, see `but help cli-ids`.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// The commit or branch to amend into.
    #[clap(short, long, value_name = "COMMIT_OR_BRANCH")]
    pub target: CliIdArg,

    /// One or more uncommitted files or hunks to amend.
    #[clap(required = true)]
    pub sources: Vec<CliIdArg>,
}
