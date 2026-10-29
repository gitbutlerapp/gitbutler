//! Arguments for `apply`.

#![deny(missing_docs)]

use crate::args::atoms::BranchArg;

/// Apply a branch.
///
/// If you want to apply an unapplied branch to your workspace so you
/// can work on it, you can run `but apply <branch-name>`.
///
/// This will apply the changes in that branch into your working directory
/// as a parallel applied branch.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// The branch to apply.
    pub branch: BranchArg,
}
