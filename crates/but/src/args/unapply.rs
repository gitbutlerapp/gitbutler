//! Arguments for `unapply`.

#![deny(missing_docs)]

use crate::args::atoms::CliIdArg;

/// Remove a branch from the workspace, keeping it to apply again later.
///
/// If you want to unapply an applied branch from your workspace
/// (effectively stashing it) so you can work on other branches,
/// you can run `but unapply <branch-name>`.
///
/// This will remove the changes in that branch from your working
/// directory and you can re-apply it later when needed. You will then
/// see the branch as unapplied in `but branch list`.
///
/// With the single-branch feature enabled, unapplying the last branch safely checks out
/// the target's local tracking branch, if available. The unapplied branch is preserved
/// and stays unapplied when switching back with `but switch --workspace`.
///
/// Several branches or stacks can be given at once: `but unapply <a> <b>`.
///
/// Each identifier can be:
/// - A CLI ID pointing to a stack or branch (e.g., "bu" from `but status`)
/// - A branch name
///
/// If a branch name (or an identifier pointing to a branch) is provided,
/// the entire stack containing that branch will be unapplied. Identifiers that
/// point into the same stack unapply it once.
///
/// For more details about CLI IDs, see `but help cli-ids`.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// The branches or stacks to unapply.
    #[clap(value_name = "BRANCH_OR_STACK", required = true)]
    pub targets: Vec<CliIdArg>,
}
