//! Arguments for `switch`.

#![deny(missing_docs)]

use crate::args::atoms::CliIdArg;

/// Switch to a local branch or the GitButler workspace.
///
/// Runs Git's `post-checkout` hooks after a successful switch. Hook failure
/// reports an error but leaves the checkout in place.
///
/// ## Examples
///
/// Switch to a branch:
///
/// ```text
/// but switch my-feature
/// ```
///
/// Show a fuzzy branch picker:
///
/// ```text
/// but switch
/// ```
///
/// Switch back to the GitButler workspace:
///
/// ```text
/// but switch --workspace
/// ```
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
#[clap(group(
    clap::ArgGroup::new("switch_target")
        .args(["target", "workspace"])
        .required(false)
        .multiple(true)
))]
pub struct Platform {
    /// Name of the branch to switch to.
    pub target: Option<CliIdArg>,

    /// Switch back to the GitButler workspace.
    #[clap(long, short = 'w', conflicts_with = "target")]
    pub workspace: bool,
}
