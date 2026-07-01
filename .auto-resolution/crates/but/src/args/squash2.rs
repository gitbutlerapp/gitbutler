//! Arguments for `squash2`.

#![deny(missing_docs)]

use crate::args::atoms::CliIdArg;

/// Squash commits, branches, or changes.
///
/// Squash is flexible in the ways it can move changes around. It can
///
/// - Squash commits into other commits
/// - Squash branches into commits
/// - Move changes between commits
/// - Amend uncommitted changes into a commit
/// - Uncommit commits
/// - Uncommit changes in commits
///
/// If no message-related flag is passed an editor will be opened where the new message can be
/// composed.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// The message to use for the new commit.
    ///
    /// Can be supplied any number of times, each value being appended to the preceding ones with a
    /// blank line in between.
    ///
    /// This cannot be used when `TARGET` is the uncommitted area (`zz`).
    #[clap(short, long, group = "commit_message")]
    pub message: Option<Vec<String>>,

    /// Creates the commit without a commit message.
    ///
    /// This cannot be used when `TARGET` is the uncommitted area (`zz`).
    #[clap(long, group = "commit_message")]
    pub no_message: bool,

    /// Use the message of the target.
    ///
    /// The message of the source(s) will be discarded.
    ///
    /// This cannot be used when `TARGET` is the uncommitted area (`zz`).
    #[clap(long, short = 'u', group = "commit_message")]
    pub use_target_message: bool,

    /// Use the message of the source(s).
    ///
    /// The message of the target will be discarded.
    ///
    /// Cannot be used if `<SOURCES>` are not committed, if `TARGET` is the uncommitted area
    /// (`zz`), or if moving committed changes between commits.
    #[clap(long, group = "commit_message")]
    pub use_source_message: bool,

    /// The target to squash into.
    ///
    /// If `TARGET` is a commit the sources will be added to the commit.
    ///
    /// If `TARGET` is a branch the sources will be added to the first commit on the branch.
    ///
    /// If `TARGET` is the uncommitted area (`zz`) the sources will be uncommitted.
    #[clap(long, short)]
    pub target: Option<CliIdArg>,

    /// The sources to squash.
    ///
    /// If `<SOURCES>` is one or more commits they will be squashed into the target.
    ///
    /// If `<SOURCES>` is one or more branches all the commits on the branches will be squashed
    /// into the target and the branches will be removed.
    ///
    /// If `TARGET` is omitted and `<SOURCES>` is exactly one branch all commits on the branch will
    /// be squashed.
    ///
    /// If `<SOURCES>` is one or more uncommitted files or hunks they will be squashed into the
    /// target.
    ///
    /// If `<SOURCES>` is the uncommitted area (`zz`) all uncommitted changes will be squashed into
    /// the target.
    ///
    /// If `<SOURCES>` is a committed file those changes will be moved into the target. All changes
    /// must come from the same commit. It is not possible to move changes from multiple source
    /// commits into a single target.
    ///
    /// It is not possible to mix sources of different types, i.e., all sources must either be
    /// commits, branches, uncommitted files, `zz`, or committed files.
    #[clap(required = true)]
    pub sources: Vec<CliIdArg>,
}
