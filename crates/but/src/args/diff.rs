use crate::args::atoms::CliIdArg;

/// Show the diff of changes in the repo.
///
/// Without any arguments, it shows the diff of all uncommitted changes of the checkout `but` runs
/// in: the main worktree's, or a linked worktree's when run from inside one. Optionally, provide
/// CLI IDs or paths to show only those changes, one after the other.
///
/// A commit or branch must be the only target. To compare two revisions, use `git diff <a> <b>`.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// What to diff, by CLI ID: a commit, branch, committed file or hunk, uncommitted file or
    /// hunk, path of an uncommitted file or directory, or a worktree's uncommitted area. A commit
    /// lists its files and hunks with their IDs.
    ///
    /// If omitted shows the diff of all uncommitted changes of the checkout `but` runs in, with file
    /// and hunk IDs.
    ///
    /// For more details about CLI IDs, see `but help cli-ids`.
    pub targets: Vec<CliIdArg>,
    /// Show the changed files and their line counts instead of the patches.
    #[clap(long)]
    pub stat: bool,
}

/// Example invocations appended to a `but diff` parse error.
pub(crate) const ERROR_EXAMPLES: &str = "\
Examples:
  but diff                    # all uncommitted changes, staged or not
  but diff <file-or-hunk>...  # only the given changes
  but diff --stat             # only the changed files, with line counts
  git diff <flag>...          # any other Git diff flag
";
