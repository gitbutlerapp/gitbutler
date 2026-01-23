/// Arguments for the `but config` command and subcommands.

#[derive(Debug, clap::Parser)]
pub struct Platform {
    #[clap(subcommand)]
    pub cmd: Option<Subcommands>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// View and configure user information (name, email, editor).
    ///
    /// Without arguments, displays current user.name, user.email, and core.editor.
    /// Use subcommands to set or unset configuration values.
    ///
    /// ## Examples
    ///
    /// View user configuration:
    ///
    /// ```text
    /// but config user
    /// ```
    ///
    /// Set user name (locally):
    ///
    /// ```text
    /// but config user set name "John Doe"
    /// ```
    ///
    /// Set user email globally:
    ///
    /// ```text
    /// but config user set --global email john@example.com
    /// ```
    ///
    /// Unset a local value:
    ///
    /// ```text
    /// but config user unset name
    /// ```
    User {
        #[clap(subcommand)]
        cmd: Option<UserSubcommand>,
    },

    /// View and manage forge configuration.
    ///
    /// Shows configured forge accounts (GitHub, GitLab, etc.) and authentication status.
    /// Use subcommands to authenticate or forget accounts.
    ///
    /// ## Examples
    ///
    /// View configured forge accounts:
    ///
    /// ```text
    /// but config forge
    /// ```
    ///
    /// Authenticate with a forge:
    ///
    /// ```text
    /// but config forge auth
    /// ```
    ///
    /// List authenticated accounts:
    ///
    /// ```text
    /// but config forge list-users
    /// ```
    ///
    /// Forget an account:
    ///
    /// ```text
    /// but config forge forget username
    /// ```
    Forge {
        #[clap(subcommand)]
        cmd: Option<ForgeSubcommand>,
    },

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

/// Subcommands for `but config user`
#[derive(Debug, clap::Subcommand)]
pub enum UserSubcommand {
    /// Set a user configuration value.
    ///
    /// ## Examples
    ///
    /// ```text
    /// but config user set name "John Doe"
    /// but config user set --global email john@example.com
    /// ```
    Set {
        /// The configuration key to set
        key: UserConfigKey,
        /// The value to set
        value: String,
        /// Set the configuration globally instead of locally
        #[clap(long, short = 'g')]
        global: bool,
    },

    /// Unset (remove) a user configuration value.
    ///
    /// ## Examples
    ///
    /// ```text
    /// but config user unset name
    /// but config user unset --global email
    /// ```
    Unset {
        /// The configuration key to unset
        key: UserConfigKey,
        /// Unset the global configuration instead of local
        #[clap(long, short = 'g')]
        global: bool,
    },
}

/// User configuration keys that can be set or unset
#[derive(Debug, Clone, clap::ValueEnum)]
pub enum UserConfigKey {
    /// Git user name (user.name)
    Name,
    /// Git user email (user.email)
    Email,
    /// Git editor (core.editor)
    Editor,
}

impl UserConfigKey {
    /// Convert to the corresponding git config key
    pub fn to_git_key(&self) -> &'static str {
        match self {
            UserConfigKey::Name => "user.name",
            UserConfigKey::Email => "user.email",
            UserConfigKey::Editor => "core.editor",
        }
    }
}

/// Subcommands for `but config forge`
#[derive(Debug, clap::Subcommand)]
pub enum ForgeSubcommand {
    /// Authenticate with your forge provider (currently only GitHub is supported).
    ///
    /// This will guide you through the authentication process using either:
    /// - Device flow (OAuth)
    /// - Personal Access Token (PAT)
    /// - GitHub Enterprise
    Auth,

    /// List authenticated forge accounts known to GitButler.
    ///
    /// Shows all configured accounts and their authentication status.
    ListUsers,

    /// Forget a previously authenticated forge account.
    ///
    /// ## Examples
    ///
    /// Forget a specific account:
    ///
    /// ```text
    /// but config forge forget username
    /// ```
    ///
    /// Interactively select which account(s) to forget:
    ///
    /// ```text
    /// but config forge forget
    /// ```
    Forget {
        /// The username of the forge account to forget.
        /// If not provided, you'll be prompted to select which account(s) to forget.
        username: Option<String>,
    },
}
