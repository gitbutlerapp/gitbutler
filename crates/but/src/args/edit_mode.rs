/// Subcommands for the `but edit-mode` command.
#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Show the status of the edit session, listing changed files.
    Status,

    /// Save changes and return to workspace mode.
    ///
    /// This commits the edited changes, rebases any commits on top of the
    /// edited commit, and returns to the normal workspace.
    Finish,

    /// Cancel the edit session and return to workspace mode.
    ///
    /// This discards all changes made during editing and restores
    /// the workspace to its pre-edit state.
    Cancel {
        /// Forcibly remove any changes made
        #[clap(short = 'f', long)]
        force: bool,
    },
}
