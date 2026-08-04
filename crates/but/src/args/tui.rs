//! Arguments for `tui`.

#![deny(missing_docs)]

use crate::args::atoms::CliIdArg;

/// Open a live terminal workspace for branches, commits, changes, and diffs.
///
/// The GitButler TUI provides a visual experience similar to the GitButler GUI - right in your
/// terminal. For the full workflow and key bindings, see <https://docs.gitbutler.com/gitbutler-tui>
///
/// **Environment variables:**
///
/// **BUT_THEME**  Sets the theme for but. Options: dark, light. [default: detected from the terminal, falling back to dark]
///
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// When the TUI quits save the selection and restore it when re-opening.
    ///
    /// If the saved selection cannot be restore the TUI launch normally as if
    /// `--remember-selection` wasn't passed.
    #[clap(long, default_value_t = false)]
    pub remember_selection: bool,

    /// Automatically show the diff when opening the TUI.
    #[clap(long)]
    pub diff: bool,

    /// The commit, branch, committed file, or uncommitted file or hunk to select.
    #[clap(conflicts_with = "remember_selection")]
    pub target: Option<CliIdArg>,

    #[clap(flatten)]
    #[allow(missing_docs)]
    pub dev_flags: DevFlags,
}

/// Dev only flags.
///
/// Enabled with `--features tui-profiling`.
#[derive(Debug, Clone, Copy, Default, clap::Args)]
pub struct DevFlags {
    /// Show debug pane with selected-line metadata.
    ///
    /// Requires `tui-profiling` feature.
    #[cfg(feature = "tui-profiling")]
    #[clap(long, default_value_t = false)]
    pub debug: bool,

    /// Quit after rendering this many frames.
    ///
    /// Requires `tui-profiling` feature.
    #[cfg(feature = "tui-profiling")]
    #[clap(long)]
    pub quit_after: Option<u64>,

    /// Run the TUI with an in-memory terminal and no terminal event polling.
    ///
    /// Requires `tui-profiling` feature.
    #[cfg(feature = "tui-profiling")]
    #[clap(long)]
    pub headless: bool,

    /// Do not print status when the TUI exits.
    ///
    /// Requires `tui-profiling` feature.
    #[cfg(feature = "tui-profiling")]
    #[clap(long)]
    pub skip_status_after: bool,
}
