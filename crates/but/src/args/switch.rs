//! Arguments for `switch`.

#![deny(missing_docs)]

use crate::args::atoms::CliIdArg;

/// Switch to a local branch, workspace branch ID, or the GitButler workspace.
///
/// ## Examples
///
/// Switch to a branch:
///
/// ```text
/// but switch my-feature
/// ```
///
/// Switch back to the GitButler workspace:
///
/// ```text
/// but switch --workspace
/// ```
///
/// Create a new branch at the project target and switch to it:
///
/// ```text
/// but switch --new
/// ```
///
/// Create a named branch at the project target and switch to it:
///
/// ```text
/// but switch --new my-feature
/// ```
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
#[clap(group(
    clap::ArgGroup::new("switch_target")
        .args(["target", "workspace", "new"])
        .required(true)
        .multiple(true)
))]
pub struct Platform {
    /// Branch name, full local branch ref, workspace CLI branch ID, or new branch name with --new.
    pub target: Option<CliIdArg>,

    /// Switch back to gitbutler/workspace.
    #[clap(long, short = 'w', conflicts_with_all = &["target", "new"])]
    pub workspace: bool,

    /// Create a branch at the project target and switch to it.
    #[clap(long = "new", short = 'n')]
    pub new: bool,
}
