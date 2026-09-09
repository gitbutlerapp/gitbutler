//! Arguments for `squash`.

#![deny(missing_docs)]

use crate::args::atoms::{AllowMergedArg, CliIdArg};

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
/// - Uncommit branches
///
/// If no message-related flag is passed when squashing commits or branches, an editor may be
/// opened where the new message can be composed; other squashes keep the target's message.
///
/// For more details about CLI IDs, see `but help cli-ids`.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// The message to use for the new commit.
    ///
    /// Can be supplied any number of times, each value being appended to the preceding ones with a
    /// blank line in between. Without a message flag, squashing commits or branches opens the
    /// editor in a terminal; a non-interactive run skips the editor.
    ///
    /// This cannot be used when `TARGET` is the uncommitted area (`@`).
    #[clap(short, long, group = "commit_message")]
    pub message: Option<Vec<String>>,

    /// Create the commit without a message.
    ///
    /// This cannot be used when `TARGET` is the uncommitted area (`@`).
    #[clap(long, group = "commit_message")]
    pub no_message: bool,

    /// Use the message of the target.
    ///
    /// The message of the source(s) will be discarded.
    ///
    /// This cannot be used when `TARGET` is the uncommitted area (`@`).
    #[clap(long, short = 'u', group = "commit_message")]
    pub use_target_message: bool,

    /// Use the message of the source(s).
    ///
    /// The message of the target will be discarded.
    ///
    /// Cannot be used if `<SOURCES>` are not committed, if `TARGET` is the uncommitted area
    /// (`@`), or if moving committed changes between commits.
    #[clap(long, group = "commit_message")]
    pub use_source_message: bool,

    /// The target to squash into.
    ///
    /// If `TARGET` is a commit the sources will be added to the commit.
    ///
    /// If `TARGET` is a branch the sources will be added to that branch's newest commit (its tip).
    ///
    /// If `TARGET` is the uncommitted area (`@`) the sources will be uncommitted. A commit owned by a
    /// worktree uncommits into that worktree's area instead, named by its ID (`<id>:@`).
    #[clap(long, short)]
    pub target: Option<CliIdArg>,

    /// The sources to squash, all of one kind.
    ///
    /// Commits: squashed into the target. Branches: every commit on them is squashed into the
    /// target and the branches are removed; with no target and exactly one branch, that branch is
    /// squashed into a single commit. Uncommitted files or hunks, or `@` for all of them: squashed
    /// into the target. Committed files and hunks from one commit: moved into the target.
    ///
    /// A target of `@` uncommits the sources instead. With `--target` and no sources, `@` is used.
    #[clap(required_unless_present = "target")]
    pub sources: Vec<CliIdArg>,

    #[clap(flatten)]
    #[allow(missing_docs)]
    pub allow_merged: AllowMergedArg,
}

/// Example invocations appended to a `but squash` parse error.
pub(crate) const ERROR_EXAMPLES: &str = "\
Examples:
  but squash <commit>... -t <other-commit> -m \"message\"   # squash commits into another commit
  but squash <branch> -m \"message\"                # squash a branch into a single commit
  but squash <file> -t <commit>                    # move an uncommitted file into a commit
  but squash <commit>:<file>:<hunk> -t <other-commit> # move a committed hunk to another commit
";
