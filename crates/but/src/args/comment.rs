use crate::args::atoms::CliIdArg;

/// Work with ephemeral comments anchored to lines in diffs.
///
/// Comments are typically created in the GUI on a line of a diff — of an uncommitted file, or of
/// a commit — and picked up here by agents, which act on them and archive them when done.
/// Anchors follow the diff as it changes; comments whose anchored line disappears from the diff
/// (for example because the change was committed or discarded) are archived automatically.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
#[deny(missing_docs)]
pub struct Platform {
    /// The subcommand to run.
    #[clap(subcommand)]
    pub cmd: Subcommands,
}

/// The `but _comment` subcommands.
#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// List all comments, with their anchors refreshed against the current diffs.
    ///
    /// Every listed comment points at a line that exists in the current diff of its file and
    /// includes an excerpt of the surrounding diff for context. Comments whose anchor no longer
    /// exists are archived automatically and not listed.
    List {
        /// Block until a comment exists instead of returning an empty listing.
        ///
        /// Returns immediately when comments already exist; prints a notice when the
        /// timeout elapses without any. Run again to keep waiting.
        #[clap(long)]
        wait: bool,
        /// How many seconds `--wait` blocks before giving up for this invocation.
        #[clap(long, value_name = "SECONDS", default_value_t = 60, requires = "wait")]
        timeout: u64,
    },
    /// Archive a comment, hiding it from all future listings.
    Archive {
        /// The id of the comment to archive. A unique prefix is enough.
        id: String,
    },
    /// Add a comment anchored to a line in a diff.
    Add {
        /// Where to anchor the comment, as `<path>:<line>`. The line number is 1-based and counts
        /// in the new version of the file (or in the old version with `--old`), and must exist in
        /// the diff being commented on.
        anchor: String,
        /// The comment text.
        #[clap(short, long)]
        message: String,
        /// Anchor to the diff of this commit instead of the uncommitted changes.
        #[clap(long, value_name = "COMMIT")]
        commit: Option<CliIdArg>,
        /// Count the line in the old side of the diff (removed lines and context) instead of the
        /// new side.
        #[clap(long)]
        old: bool,
    },
}
