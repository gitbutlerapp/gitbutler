//! Arguments for `open`.

#![deny(missing_docs)]

use crate::args::atoms::CliIdArg;

/// Open the project in GitButler.
///
/// With no argument this opens the workspace. Given a branch or a commit, the
/// app opens with that selected, so a link can point at the thing you are
/// talking about rather than at the app.
///
/// Commits are addressed by their change ID where they have one, which
/// survives amending and rebasing — the link keeps working after the commit is
/// rewritten.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// The branch or commit to select, defaulting to the workspace itself.
    pub target: Option<CliIdArg>,

    /// Print the link instead of opening it.
    #[clap(long)]
    pub print: bool,
}
