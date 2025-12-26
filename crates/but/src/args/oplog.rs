#[derive(Debug, clap::Parser)]
pub struct Platform {
    /// Show all operations (uses a pager)
    #[clap(long, short = 'a', global = true)]
    pub all: bool,
    /// Only show named snapshots
    #[clap(long, short = 's', global = true)]
    pub snapshots: bool,
    #[clap(subcommand)]
    pub cmd: Option<Subcommands>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// List operation history.
    ///
    /// Displays a list of past operations performed in the repository,
    /// including their timestamps and descriptions.
    ///
    /// This allows you to restore to any previous point in the history of the
    /// project. All state is preserved in operations, including uncommitted changes.
    ///
    /// You can use `but restore <oplog-sha>` to restore to a specific state.
    ///
    List {
        /// Start from this oplog SHA instead of the head
        #[clap(long)]
        since: Option<String>,
        /// Show all operations (uses a pager)
        #[clap(long, short = 'a')]
        all: bool,
        /// Only show named snapshots
        #[clap(long, short = 's')]
        snapshots: bool,
    },

    /// Create an on-demand snapshot with optional message.
    ///
    /// This allows you to create a named snapshot of the current state, which
    /// can be helpful to always be able to return to a known good state.
    ///
    /// You can provide an optional message to describe the snapshot.
    ///
    Snapshot {
        /// Message to include with the snapshot
        #[clap(short = 'm', long = "message")]
        message: Option<String>,
    },
}
