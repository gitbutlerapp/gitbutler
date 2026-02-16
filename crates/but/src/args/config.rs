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

    /// View or set metrics collection.
    ///
    /// GitButler uses metrics to help us know what is useful and improve it.
    /// Privacy policy: <https://gitbutler.com/privacy>
    ///
    /// Without arguments, displays the current setting.
    ///
    /// ## Examples
    ///
    /// View metrics configuration:
    ///
    /// ```text
    /// but config metrics
    /// ```
    ///
    /// Enable metrics:
    ///
    /// ```text
    /// but config metrics enable
    /// ```
    ///
    /// Disable metrics:
    ///
    /// ```text
    /// but config metrics disable
    /// ```
    Metrics {
        /// Whether metrics are enabled.
        #[clap(value_enum)]
        status: Option<MetricsStatus>,
    },

    /// View and configure UI preferences.
    ///
    /// Without arguments, displays current UI settings.
    /// Use subcommands to set or unset configuration values.
    ///
    /// ## Examples
    ///
    /// View UI configuration:
    ///
    /// ```text
    /// but config ui
    /// ```
    ///
    /// Enable TUI mode for diff by default:
    ///
    /// ```text
    /// but config ui set tui true
    /// ```
    ///
    /// Disable TUI mode:
    ///
    /// ```text
    /// but config ui set tui false
    /// ```
    Ui {
        #[clap(subcommand)]
        cmd: Option<UiSubcommand>,
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

/// Subcommands for `but config ui`
#[derive(Debug, clap::Subcommand)]
pub enum UiSubcommand {
    /// Set a UI configuration value.
    ///
    /// ## Examples
    ///
    /// ```text
    /// but config ui set tui true
    /// but config ui set --global tui true
    /// ```
    Set {
        /// The configuration key to set
        key: UiConfigKey,
        /// The value to set (true/false or 1/0)
        value: String,
        /// Set the configuration globally instead of locally
        #[clap(long, short = 'g')]
        global: bool,
    },

    /// Unset (remove) a UI configuration value.
    ///
    /// ## Examples
    ///
    /// ```text
    /// but config ui unset tui
    /// ```
    Unset {
        /// The configuration key to unset
        key: UiConfigKey,
        /// Unset the global configuration instead of local
        #[clap(long, short = 'g')]
        global: bool,
    },
}

/// UI configuration keys that can be set or unset
#[derive(Debug, Clone, clap::ValueEnum)]
pub enum UiConfigKey {
    /// Use the interactive TUI for diff by default (but.ui.tui)
    Tui,
}

impl UiConfigKey {
    /// Convert to the corresponding git config key
    pub fn to_git_key(&self) -> &'static str {
        match self {
            UiConfigKey::Tui => "but.ui.tui",
        }
    }
}

/// Values for `but config metrics`
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum MetricsStatus {
    Enable,
    Disable,
}

impl MetricsStatus {
    pub fn enabled(self) -> bool {
        matches!(self, MetricsStatus::Enable)
    }
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
#[derive(Debug, Clone, clap::Subcommand)]
pub enum ForgeSubcommand {
    /// Authenticate with your forge provider (currently only GitHub is supported).
    ///
    /// This will guide you through the authentication process using either:
    /// GitHub
    ///  - Device flow (OAuth)
    ///  - Personal Access Token (PAT)
    ///  - GitHub Enterprise
    ///
    /// GitLab
    ///  - Personal Access Token (PAT)
    ///  - Self-Hosted
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
    /// If there are multiple accounts with the same username, you'll be interactively prompted to select which one(s) to forget.
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
