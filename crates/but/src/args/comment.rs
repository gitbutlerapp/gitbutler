use crate::args::atoms::CliIdArg;

/// The kind of identity a comment client is acting as.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum AuthorKind {
    /// A person using the client.
    Human,
    /// An automated agent using the client.
    Agent,
}

/// Work with ephemeral comments anchored to lines in diffs.
///
/// Comments are typically created in the GUI on a line of a diff — of an uncommitted file, or of
/// a commit — and picked up here by agents, which act on them, reply, and archive when done.
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
        /// The display name of the waiting client. Required with `--wait`.
        #[clap(long, value_name = "NAME", requires = "wait")]
        author: Option<String>,
        /// Stable identity of the waiting agent workstream. Required with `--wait`.
        #[clap(long, value_name = "ID", requires = "wait")]
        client_id: Option<String>,
        /// Friendly title of the waiting agent workstream.
        #[clap(long, value_name = "TITLE", requires = "wait")]
        title: Option<String>,
        /// Whether the waiting client is a human or an agent. Required with `--wait`.
        #[clap(long, value_enum, requires = "wait")]
        author_kind: Option<AuthorKind>,
    },
    /// Archive a comment, hiding it from all future listings.
    Archive {
        /// The id of the comment to archive. A unique prefix is enough.
        id: String,
    },
    /// Acknowledge messages through a specific message in a thread.
    Ack {
        /// The id of the comment to acknowledge. A unique prefix is enough.
        id: String,
        /// The id of the last handled message. A unique prefix within the thread is enough.
        #[clap(long, value_name = "MESSAGE_ID")]
        message: String,
        /// Stable identity of the acknowledging agent workstream.
        #[clap(long, value_name = "ID")]
        client_id: String,
    },
    /// Reply to a comment without archiving or resolving it.
    Reply {
        /// The id of the comment to reply to. A unique prefix is enough.
        id: String,
        /// The reply text.
        #[clap(short, long)]
        message: String,
        /// The display name of the replying client.
        #[clap(long, value_name = "NAME")]
        author: String,
        /// Whether the replying client is a human or an agent.
        #[clap(long, value_enum)]
        author_kind: AuthorKind,
        /// Stable workstream identity. Required when `--author-kind agent`.
        #[clap(long, value_name = "ID")]
        client_id: Option<String>,
        /// Agent workstream ids to invite into the thread with this reply.
        #[clap(long, value_name = "CLIENT_ID")]
        mention: Vec<String>,
        /// Last message this reply has handled. A unique prefix within the thread is enough.
        #[clap(long, value_name = "MESSAGE_ID", requires = "client_id")]
        ack_through: Option<String>,
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
        /// The display name of the client starting the thread.
        #[clap(long, value_name = "NAME")]
        author: String,
        /// Whether the client starting the thread is a human or an agent.
        #[clap(long, value_enum)]
        author_kind: AuthorKind,
        /// Stable workstream identity. Required when `--author-kind agent`.
        #[clap(long, value_name = "ID")]
        client_id: Option<String>,
        /// Agent workstream ids to invite into the new thread.
        #[clap(long, value_name = "CLIENT_ID")]
        mention: Vec<String>,
        /// Anchor to the diff of this commit instead of the uncommitted changes.
        #[clap(long, value_name = "COMMIT")]
        commit: Option<CliIdArg>,
        /// Count the line in the old side of the diff (removed lines and context) instead of the
        /// new side.
        #[clap(long)]
        old: bool,
    },
}
