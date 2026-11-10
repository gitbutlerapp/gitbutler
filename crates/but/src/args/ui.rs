/// Drive the running GitButler GPUI app as the keyboard does.
///
/// `dispatch` runs any of the app's actions by name, as `tiling::SplitRight`, where the keys
/// are. An action that throws work away asks first in the app, as it does from a key.
///
/// The app listens at $GITBUTLER_GPUI_SOCKET when set, as in its own terminals.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
#[deny(missing_docs)]
pub struct Platform {
    /// The subcommand to run.
    #[clap(subcommand)]
    pub cmd: Subcommands,
}

/// The `but _ui` subcommands.
#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Print the selected row, the tiles and their panes, and where the keys are.
    State,
    /// List the actions that would act where the keys are, with their keys.
    Actions,
    /// Run an action where the keys are, or in a tile and pane given the keys first.
    Dispatch {
        /// The action, as `actions` names it.
        action: String,
        /// The action's argument, as JSON.
        #[clap(long, value_name = "JSON")]
        args: Option<String>,
        /// Give this tile the keys first, as `state` names it, or `navigator`.
        #[clap(long, value_name = "TILE")]
        tile: Option<String>,
        /// Give this pane the keys first, as `files` or `diff`.
        #[clap(long, value_name = "PANE")]
        pane: Option<String>,
    },
    /// Select a row, as Go to anything finds it typed in full.
    Select {
        /// A project, branch, commit, worktree or page, by name.
        query: String,
    },
    /// Give a tile, or a pane of its page, the keys.
    Focus {
        /// As `state` names it, or `navigator`. Defaults to the tile with the keys.
        #[clap(long, value_name = "TILE")]
        tile: Option<String>,
        /// The pane, as `files` or `diff`.
        #[clap(long, value_name = "PANE")]
        pane: Option<String>,
    },
    /// Type where the keys are, into a text field or a terminal. A newline presses Enter.
    Type {
        /// What to type.
        text: String,
    },
}
