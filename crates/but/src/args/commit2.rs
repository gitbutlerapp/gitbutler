//! Arguments for `commit2`.

#![deny(missing_docs)]

use crate::args::atoms::CliIdArg;

/// Create a commit.
///
/// By default, all uncommitted changes are included in the commit. This can be controlled with
/// change flags such as `--empty` and `--interactive`, or by providing `CHANGES` as positional
/// arguments.
///
/// If there are no branches applied, a new branch is created for the commit. If there is only one
/// branch applied, the commit is placed at the tip of that branch. Otherwise, the targeting flags
/// `--above`, `--below` and `--branch` must be used to control where the commit is placed. Note
/// that only one of the targeting flags can be provided at a time.
///
/// The commit is expected to have a commit message unless `--no-message` is provided. If neither of
/// `--no-message` nor `--message` is provided, the user's preferred editor is opened to input a
/// message.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// Creates the commit without a commit message.
    #[clap(long, group = "commit_message")]
    pub no_message: bool,

    /// The message to use for the commit.
    ///
    /// Can be supplied any amount of times, each value being appended to the preceding ones with a
    /// blank line in between.
    #[clap(short, long, group = "commit_message")]
    pub message: Option<Vec<String>>,

    /// Place the commit on the branch `BRANCH`.
    ///
    /// If `BRANCH` does not exist, it is created as an unstacked branch.
    ///
    /// If `BRANCH` is omitted, an unstacked branch with a generated name is created.
    ///
    /// Attempting to place a commit on a branch that exists but is not applied is an error.
    #[clap(short, long, value_name = "BRANCH", group = "targeting")]
    pub branch: Option<Option<CliIdArg>>,

    /// Place the commit above `BRANCH_OR_COMMIT`, which must be an applied branch or commit.
    ///
    /// If `BRANCH_OR_COMMIT` is a commit, the new commit is placed on the same branch as the
    /// targeted commit.
    ///
    /// If `BRANCH_OR_COMMIT` is a branch, the new commit is placed on a new branch above the
    /// targeted branch.
    #[clap(
        short = 'A',
        long,
        value_name = "BRANCH_OR_COMMIT",
        group = "targeting"
    )]
    pub above: Option<CliIdArg>,

    /// Place the commit below `BRANCH_OR_COMMIT`, which must be an applied branch or commit.
    ///
    /// If `BRANCH_OR_COMMIT` is a commit, the new commit is placed on the same branch as the targeted
    /// commit.
    ///
    /// If `BRANCH_OR_COMMIT` is a branch, the new commit is placed on a new branch below the
    /// targeted branch. Branches are treated as buckets, meaning that "below a branch" is treated
    /// as below the oldest ancestor on that branch.
    #[clap(
        short = 'B',
        long,
        value_name = "BRANCH_OR_COMMIT",
        group = "targeting"
    )]
    pub below: Option<CliIdArg>,

    /// Forces the commit to be empty regardless of repository state.
    #[clap(long, group = "changes_to_commit")]
    pub empty: bool,

    /// Open the TUI to interactively select what to commit.
    #[clap(short, long, group = "changes_to_commit")]
    pub interactive: bool,

    /// One or more changes to commit.
    ///
    /// A change can either be a file or a hunk.
    #[clap(group = "changes_to_commit")]
    pub changes: Vec<CliIdArg>,
}
