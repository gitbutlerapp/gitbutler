#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Show the status of conflict resolution, listing remaining conflicted files.
    Status,

    /// Finalize conflict resolution and return to workspace mode.
    ///
    /// This commits the resolved changes, rebases any commits on top of the
    /// resolved commit, and returns to the normal workspace.
    Finish,

    /// Cancel conflict resolution and return to workspace mode.
    ///
    /// This discards all changes made during resolution and restores
    /// the workspace to its pre-resolution state.
    Cancel,
}
