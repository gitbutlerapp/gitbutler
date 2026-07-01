//! Command-line argument definitions for the `but alias` command.

#[derive(Debug, clap::Parser)]
pub struct Platform {
    #[clap(subcommand)]
    pub cmd: Option<Subcommands>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// List all configured aliases (default)
    List,

    /// Add a new alias
    ///
    /// Creates a new alias that expands to the given command.
    ///
    /// ## Examples
    ///
    /// ```text
    /// but alias add st status
    /// but alias add stv "status --verbose"
    /// but alias add co "commit --only"
    /// ```
    Add {
        /// The name of the alias to create
        name: String,

        /// The command and arguments that the alias should expand to
        ///
        /// If the value contains spaces or special characters, quote it:
        /// "status --verbose"
        value: String,

        /// Store the alias globally (in ~/.gitconfig) instead of locally
        #[clap(long, short = 'g')]
        global: bool,
    },

    /// Remove an existing alias
    ///
    /// ## Examples
    ///
    /// ```text
    /// but alias remove st
    /// but alias remove co --global
    /// ```
    #[clap(alias = "rm")]
    Remove {
        /// The name of the alias to remove
        name: String,

        /// Remove from global config (in ~/.gitconfig) instead of local
        #[clap(long, short = 'g')]
        global: bool,
    },
}
