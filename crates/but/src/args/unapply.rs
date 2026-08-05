//! Arguments for `unapply`.

#![deny(missing_docs)]

use crate::args::atoms::CliIdArg;

/// Unapply a branch or stack.
///
/// For more details about CLI IDs, see `but help cli-ids`.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// The branch or stack to unapply.
    #[clap(value_name = "BRANCH_OR_STACK")]
    pub target: CliIdArg,
}
