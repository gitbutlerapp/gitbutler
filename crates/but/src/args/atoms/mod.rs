//! This module contains "CLI argument atoms".
//!
//! Instead of using `String` for CLI argument types we should use more specific types from this
//! modules. The types provides ways to be "resolved" that ensures consistent behavior, especially
//! around error messages.

#![warn(missing_docs)]

mod branch_arg;
pub use branch_arg::*;

mod commit_arg;
pub use commit_arg::*;

mod cli_id;
pub use cli_id::*;

mod message_file;
pub use message_file::*;

/// The `--json` flag, flattened into each command that can print its output as JSON.
///
/// The flag is global, so on a command with subcommands it also covers every subcommand, and is
/// accepted before or after the subcommand's name. A command with `args_conflicts_with_subcommands`,
/// like `skill`, accepts it only after the subcommand's name. Put it on a command with
/// subcommands only when the command and all of its subcommands print JSON, and otherwise on
/// each subcommand that does.
///
/// Commands without it reject `--json`. [`crate::args::Subcommands::json_arg`] finds it on the
/// parsed command.
#[derive(Debug, Clone, Copy, Default, clap::Args)]
pub struct JsonArg {
    /// Output detailed information as JSON for tool consumption.
    #[clap(long, global = true)]
    pub json: bool,
}

/// Escape hatch for the merged-upstream guard, shared by mutation commands.
#[derive(Debug, Clone, Copy, Default, clap::Args)]
pub struct AllowMergedArg {
    /// Allow targeting branches and commits that are already merged upstream.
    ///
    /// By default, mutations refuse to touch history that has landed in the
    /// target branch, since the results tend to conflict on the next `but pull`.
    #[clap(long)]
    pub allow_merged: bool,
}
