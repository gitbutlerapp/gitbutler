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
    /// Shows configured forge accounts (GitHub, GitLab, Bitbucket) and authentication status.
    /// Use subcommands to manage accounts or native GitHub stacked pull requests.
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
    ///
    /// View or configure native GitHub stacked pull requests:
    ///
    /// ```text
    /// but config forge github-stacks
    /// but config forge github-stacks enable
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
    ///
    /// Set a target branch and push branches to a fork:
    ///
    /// ```text
    /// but config target upstream/main --push-remote origin
    /// ```
    Target {
        /// New target branch to set (e.g., "origin/main")
        branch: Option<String>,
        /// Remote to push branches to (e.g., "origin" for a fork).
        #[clap(long, value_name = "REMOTE", requires = "branch")]
        push_remote: Option<String>,
    },

    /// View or set the remote used to push branches.
    ///
    /// Without arguments, displays the effective push remote. With a remote name, updates the push
    /// remote without changing the target branch.
    ///
    /// ## Examples
    ///
    /// View the current push remote:
    ///
    /// ```text
    /// but config push-remote
    /// ```
    ///
    /// Push branches to a fork remote:
    ///
    /// ```text
    /// but config push-remote origin
    /// ```
    PushRemote {
        /// New remote to use when pushing branches (e.g., "origin").
        remote: Option<String>,
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

    /// View and configure feature flags.
    ///
    /// Without arguments, displays all feature flags that can be changed through the CLI.
    /// Specify a flag to view its current value, or add `enable` or `disable` to update it.
    ///
    /// ## Examples
    ///
    /// View all feature flags:
    ///
    /// ```text
    /// but config feature
    /// ```
    ///
    /// Enable single-branch mode:
    ///
    /// ```text
    /// but config feature single-branch enable
    /// ```
    Feature {
        /// Feature flag to view or update.
        #[clap(value_enum)]
        flag: Option<FeatureFlag>,
        /// Whether the feature flag is enabled.
        #[clap(value_enum, requires = "flag")]
        status: Option<FeatureStatus>,
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

    /// View and configure AI provider settings.
    ///
    /// Without subcommands, this starts an interactive setup flow.
    /// Use provider subcommands for non-interactive configuration.
    ///
    /// ## Examples
    ///
    /// Interactive setup:
    ///
    /// ```text
    /// but config ai
    /// ```
    ///
    /// View current AI configuration:
    ///
    /// ```text
    /// but config ai show
    /// ```
    ///
    /// Configure OpenAI non-interactively:
    ///
    /// ```text
    /// but config ai openai --key-option bring-your-own --api-key-env OPENAI_API_KEY --model gpt-5.4-nano
    /// ```
    ///
    /// Configure Ollama locally:
    ///
    /// ```text
    /// but config ai --local ollama --endpoint localhost:11434 --model llama3.1
    /// ```
    Ai {
        /// Configure local repository git config instead of global user config
        #[clap(long, conflicts_with = "global")]
        local: bool,
        /// Configure global user git config
        #[clap(long)]
        global: bool,
        #[clap(subcommand)]
        cmd: Option<AiSubcommand>,
    },
}

/// Subcommands for `but config ai`
#[derive(Debug, Clone, clap::Subcommand)]
pub enum AiSubcommand {
    /// Show current AI provider configuration.
    Show,

    /// Configure OpenAI as the active AI provider.
    Openai {
        /// Which credential source to use.
        #[clap(long, value_enum)]
        key_option: Option<AiKeyOption>,
        /// Preferred model name (for example, gpt-5.4-nano).
        #[clap(long)]
        model: Option<String>,
        /// Optional custom OpenAI-compatible endpoint URL.
        #[clap(long)]
        endpoint: Option<String>,
        /// OpenAI API key. Prefer --api-key-env to avoid shell history exposure.
        #[clap(long, hide_env_values = true)]
        api_key: Option<String>,
        /// Name of an environment variable holding the OpenAI API key.
        #[clap(long)]
        api_key_env: Option<String>,
    },

    /// Configure Anthropic as the active AI provider.
    Anthropic {
        /// Which credential source to use.
        #[clap(long, value_enum)]
        key_option: Option<AiKeyOption>,
        /// Preferred model name (for example, claude-3-5-haiku-latest).
        #[clap(long)]
        model: Option<String>,
        /// Anthropic API key. Prefer --api-key-env to avoid shell history exposure.
        #[clap(long, hide_env_values = true)]
        api_key: Option<String>,
        /// Name of an environment variable holding the Anthropic API key.
        #[clap(long)]
        api_key_env: Option<String>,
    },

    /// Configure Ollama as the active AI provider.
    Ollama {
        /// Ollama endpoint in host:port form (for example, localhost:11434).
        #[clap(long)]
        endpoint: Option<String>,
        /// Preferred model name.
        #[clap(long)]
        model: Option<String>,
    },

    /// Configure LM Studio as the active AI provider.
    Lmstudio {
        /// LM Studio API base endpoint (for example, http://localhost:1234/v1).
        #[clap(long)]
        endpoint: Option<String>,
        /// Preferred model name.
        #[clap(long)]
        model: Option<String>,
    },

    /// Configure OpenRouter as the active AI provider.
    Openrouter {
        /// Preferred model name (for example, openai/gpt-4.1-mini).
        #[clap(long)]
        model: Option<String>,
        /// OpenRouter API key. Prefer --api-key-env to avoid shell history exposure.
        #[clap(long, hide_env_values = true)]
        api_key: Option<String>,
        /// Name of an environment variable holding the OpenRouter API key.
        #[clap(long)]
        api_key_env: Option<String>,
    },
}

/// Credential source options for OpenAI/Anthropic.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum AiKeyOption {
    BringYourOwn,
    ButlerApi,
}

impl From<AiKeyOption> for but_llm::CredentialsKeyOption {
    fn from(value: AiKeyOption) -> Self {
        match value {
            AiKeyOption::BringYourOwn => Self::BringYourOwn,
            AiKeyOption::ButlerApi => Self::ButlerApi,
        }
    }
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

/// Feature flags that can be managed through `but config feature`.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum FeatureFlag {
    /// Use the V3 unapply compatibility mode.
    UnapplyV3Pgm,
    /// Enable single-branch mode.
    SingleBranch,
}

impl FeatureFlag {
    pub fn as_str(self) -> &'static str {
        match self {
            FeatureFlag::UnapplyV3Pgm => "unapply-v3-pgm",
            FeatureFlag::SingleBranch => "single-branch",
        }
    }

    pub fn as_json_key(self) -> &'static str {
        match self {
            FeatureFlag::UnapplyV3Pgm => "unapply_v3_pgm",
            FeatureFlag::SingleBranch => "single_branch",
        }
    }
}

/// Values for a feature flag.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum FeatureStatus {
    Enable,
    Disable,
}

impl FeatureStatus {
    pub fn enabled(self) -> bool {
        matches!(self, FeatureStatus::Enable)
    }
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
    /// Authenticate with your forge provider (GitHub, GitLab or Bitbucket).
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
    ///
    /// Bitbucket
    ///  - Atlassian API token with scopes (read:user:bitbucket,
    ///    read:repository:bitbucket, read:pullrequest:bitbucket,
    ///    write:pullrequest:bitbucket)
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

    /// View or configure native GitHub stacked pull requests for this repository.
    ///
    /// This is an opt-in GitHub private-preview feature. The setting is stored in the
    /// repository-local Git config and shared with the GitButler desktop application.
    #[cfg(feature = "legacy")]
    GithubStacks {
        /// Enable, disable, or auto-detect native GitHub stacked pull requests.
        #[clap(value_enum)]
        status: Option<GitHubStacksStatus>,
    },
}

/// Values for the native GitHub stacks project setting.
#[cfg(feature = "legacy")]
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum GitHubStacksStatus {
    /// Use native stacks when the repository supports them (the default).
    Auto,
    Enable,
    Disable,
}
