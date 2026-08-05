//! Arguments for `unapply`.

#![deny(missing_docs)]

use crate::args::atoms::CliIdArg;

/// Unapply a branch.
///
/// If you want to unapply an applied branch from your workspace
/// (effectively stashing it) so you can work on other branches,
/// you can run `but unapply <branch-name>`.
///
/// This will remove the changes in that branch from your working
/// directory and you can re-apply it later when needed. You will then
/// see the branch as unapplied in `but branch list`.
///
/// The identifier can be:
/// - A CLI ID pointing to a stack or branch (e.g., "bu" from `but status`)
/// - A branch name
///
/// If a branch name (or an identifier pointing to a branch) is provided,
/// the entire stack containing that branch will be unapplied.
///
/// For more details about CLI IDs, see `but help cli-ids`.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// The branch or stack to unapply.
    #[clap(value_name = "BRANCH_OR_STACK")]
    pub target: CliIdArg,
}
