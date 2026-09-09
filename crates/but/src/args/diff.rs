use crate::args::atoms::CliIdArg;

/// Show the diff of changes in the repo.
///
/// Without any arguments, it shows the diff of all uncommitted changes. Optionally, provide one
/// CLI ID to show the diff for an uncommitted file, branch, commit, committed file, or worktree.
///
/// `TARGET` accepts at most one entity. To show several entities, run this command once per entity.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// What to diff, by CLI ID: a commit, branch, committed file or hunk, uncommitted file or
    /// hunk, path prefix, or a worktree's uncommitted area. A commit lists its files and hunks
    /// with their IDs.
    ///
    /// If omitted shows the diff of all uncommitted changes, with file and hunk IDs.
    ///
    /// For more details about CLI IDs, see `but help cli-ids`.
    pub target: Option<CliIdArg>,
}
