use crate::args::atoms::{AllowMergedArg, CliIdArg};

/// Move commits and changes around.
///
/// Move a set of `<SOURCES>` around relative to a `TARGET`.
///
/// `<SOURCES>` is a set of commits, a set of committed files or a single branch. You are not
/// allowed to mix kinds of sources (e.g. commits and committed files) in a single command.
///
/// `TARGET` is one of `--above`, `--below`, `--unstack` or `--branch` and defines how `<SOURCES>`
/// should be moved. Depending on how `<SOURCES>` and `TARGET` are combined, a commit and/or branch
/// may be created as part of the move.
///
/// **A branch is created when:**
///
/// * You move a commit or committed file relative to a branch
/// * You unstack a commit or committed file
///
/// **A commit is created when:**
///
/// * You move a committed file relative to a commit or branch
/// * You unstack a committed file
///
/// Note the overlap between the above conditions. For example, unstacking a committed file both
/// creates a new commit for the file and a branch for the commit.
///
/// For more details about CLI IDs, see `but help cli-ids`.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
#[clap(group(
    clap::ArgGroup::new("targeting")
        .args(["above", "below", "branch", "unstack"])
        .required(true)
))]
pub struct Platform {
    /// Place `<SOURCES>` on the branch `BRANCH`.
    ///
    /// If `BRANCH` does not exist, it is created as an unstacked branch.
    ///
    /// If `BRANCH` is omitted, an unstacked branch with a generated name is created. This is
    /// exactly equivalent to `--unstack` and is allowed for any source kind.
    ///
    /// If `BRANCH` is provided, `<SOURCES>` must be either commits or committed files.
    ///
    /// Attempting to place `<SOURCES>` on a branch that exists but is not applied is an error.
    #[clap(short, long, value_name = "BRANCH")]
    pub branch: Option<Option<CliIdArg>>,
    /// Place `<SOURCES>` above `BRANCH_OR_COMMIT`, which must be an applied branch or commit.
    ///
    /// If `BRANCH_OR_COMMIT` is a commit, `<SOURCES>` are placed on the same branch as the targeted
    /// commit.
    ///
    /// If `BRANCH_OR_COMMIT` is a branch, the sources are placed on a new branch above the targeted
    /// branch.
    ///
    /// This target is applicable for all kinds of `<SOURCES>`.
    #[clap(short = 'A', long, value_name = "BRANCH_OR_COMMIT")]
    pub above: Option<CliIdArg>,
    /// Place `<SOURCES>` below `BRANCH_OR_COMMIT`, which must be an applied branch or commit.
    ///
    /// If `BRANCH_OR_COMMIT` is a commit, the `<SOURCES>` are placed on the same branch as the
    /// targeted commit.
    ///
    /// If `BRANCH_OR_COMMIT` is a branch, `<SOURCES>` are placed on a new branch below the targeted
    /// branch. Branches are treated as buckets, meaning that "below a branch" is treated as below
    /// the oldest ancestor on that branch.
    ///
    /// This target is only applicable for `<SOURCES>` that are commits or committed files.
    #[clap(short = 'B', long, value_name = "BRANCH_OR_COMMIT")]
    pub below: Option<CliIdArg>,
    /// Unstack `<SOURCES>` from their current stacks.
    ///
    /// `--unstack` does not take an argument, so `--unstack <SOURCES>` and `<SOURCES> --unstack`
    /// are equivalent.
    #[clap(long)]
    pub unstack: bool,
    /// One or more sources to move.
    ///
    /// You may provide one of the following kinds of sources:
    ///
    /// * Commits
    /// * Committed files
    ///     - All files must come from the same commit
    /// * A branch
    ///     - Branches can only be moved one at a time
    ///
    /// Mixing sources in a single command is not allowed.
    ///
    /// The order of the sources does not matter.
    ///
    /// Providing any of the sources as an argument for a target such as `--above` or `--below` is
    /// an error.
    #[clap(required = true)]
    pub sources: Vec<CliIdArg>,

    #[clap(flatten)]
    #[allow(missing_docs)]
    pub allow_merged: AllowMergedArg,
}
