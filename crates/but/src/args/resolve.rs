#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// List the conflicts of a conflicted commit, without entering resolution mode.
    ///
    /// Each conflict is shown with its ours side (the new base the commit was
    /// rebased onto), the common ancestor, and its theirs side (the commit's
    /// own version), numbered per file for use with `but resolve apply`.
    Conflicts {
        /// A conflicted commit, or a branch (meaning its oldest conflicted commit).
        /// Defaults to the first conflicted branch's oldest conflicted commit.
        commit: Option<String>,
    },

    /// Resolve conflicts of a conflicted commit, without entering resolution mode.
    ///
    /// Targets one conflict (`<path>:<N>`, numbers from `but resolve conflicts`)
    /// or every conflict in a file (`<path>` with `--ours`/`--theirs`). The
    /// replacement content for mixed resolutions is read from `--file` or stdin.
    /// Resolving only some conflicts keeps the commit conflicted with the rest,
    /// so conflicts can be worked off incrementally; the commit id changes with
    /// every apply. Undo with `but undo`.
    Apply {
        /// The conflicted file, optionally with a 1-based conflict number (`<path>:<N>`).
        target: String,
        /// A conflicted commit, or a branch (meaning its oldest conflicted
        /// commit — branch names stay stable across applies, unlike commit ids).
        /// Defaults to the first conflicted branch's oldest conflicted commit.
        #[clap(long)]
        commit: Option<String>,
        /// Take the ours side: the new base the commit was rebased onto.
        #[clap(long, conflicts_with_all = ["theirs", "file", "ai"])]
        ours: bool,
        /// Take the theirs side: the commit's own version.
        #[clap(long, conflicts_with_all = ["file", "ai"])]
        theirs: bool,
        /// Let the configured AI model merge the targeted conflicts.
        #[clap(long, conflicts_with = "file")]
        ai: bool,
        /// Read the replacement content from this file (otherwise from stdin).
        #[clap(long, short = 'F')]
        file: Option<std::path::PathBuf>,
    },

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
    Cancel {
        /// Forcibly remove any changes made
        #[clap(short = 'f', long)]
        force: bool,
    },
}
