/// Arguments for the `but config` command and subcommands.

#[derive(Debug, clap::Parser)]
pub struct Platform {
    #[clap(subcommand)]
    pub cmd: Option<Subcommands>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// View and configure user information (name and email).
    ///
    /// Without arguments, displays current user.name and user.email.
    /// With a key=value argument, sets that configuration value.
    ///
    /// ## Examples
    ///
    /// View user configuration:
    ///
    /// ```text
    /// but config user
    /// ```
    ///
    /// Set user name:
    ///
    /// ```text
    /// but config user user.name "John Doe"
    /// ```
    ///
    /// Set user email:
    ///
    /// ```text
    /// but config user user.email john@example.com
    /// ```
    User {
        /// Configuration key to set (e.g., "user.name" or "user.email")
        key: Option<String>,
        /// Value to set for the key
        value: Option<String>,
        /// Set the configuration globally instead of locally
        #[clap(long, short = 'g')]
        global: bool,
    },

    /// View forge configuration.
    ///
    /// Shows configured forge accounts (GitHub, GitLab, etc.) and authentication status.
    Forge,

    /// View or set the target branch.
    ///
    /// Without arguments, displays the current target branch.
    /// With a branch name, sets the target branch.
    ///
    /// ## Examples
    ///
    /// View current target:
    ///
    /// ```text
    /// but config target
    /// ```
    ///
    /// Set target branch:
    ///
    /// ```text
    /// but config target origin/main
    /// ```
    Target {
        /// New target branch to set (e.g., "origin/main")
        branch: Option<String>,
    },
}
