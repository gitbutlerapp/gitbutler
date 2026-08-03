use crate::args::atoms::CliIdArg;

/// Displays the diff of changes in the repo.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// The CLI ID entity to show the diff for.
    ///
    /// If omitted shows the diff of all uncommitted changes.
    ///
    /// For more details about CLI IDs, see `but help cli-ids`.
    pub target: Option<CliIdArg>,
}
