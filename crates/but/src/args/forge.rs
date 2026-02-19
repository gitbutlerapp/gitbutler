pub mod pr {
    #[derive(Debug, clap::Parser)]
    pub struct Platform {
        #[clap(subcommand)]
        pub cmd: Option<Subcommands>,
        /// Whether to create reviews as a draft.
        #[clap(long, short = 'd', default_value_t = false)]
        pub draft: bool,
    }
    #[derive(Debug, clap::Subcommand)]
    pub enum Subcommands {
        /// Create a new review for a branch.
        /// If no branch is specified, you will be prompted to select one.
        /// If there is only one branch without a review, you will be asked to confirm.
        New {
            /// The branch to create a review for.
            #[clap(value_name = "BRANCH")]
            branch: Option<String>,
            /// review title and description. The first line is the title, the rest is the description.
            #[clap(short = 'm', long = "message", conflicts_with_all = &["file", "default"])]
            message: Option<String>,
            /// Read review title and description from file. The first line is the title, the rest is the description.
            #[clap(short = 'F', long = "file", value_name = "FILE", conflicts_with_all = &["message", "default"])]
            file: Option<std::path::PathBuf>,
            /// Force push even if it's not fast-forward (defaults to true).
            #[clap(long, short = 'f', default_value_t = true)]
            with_force: bool,
            /// Skip force push protection checks
            #[clap(long, short = 's')]
            skip_force_push_protection: bool,
            /// Run pre-push hooks (defaults to true).
            #[clap(long, short = 'r', default_value_t = true)]
            run_hooks: bool,
            /// Use the default content for the review title and description, skipping any prompts.
            /// If the branch contains only a single commit, the commit message will be used.
            #[clap(long, short = 't', default_value_t = false)]
            default: bool,
            /// Whether to create reviews as a draft.
            #[clap(long, short = 'd', default_value_t = false)]
            draft: bool,
        },
        /// Configure the template to use for review descriptions.
        /// This will list all available templates found in the repository and allow you to select one.
        Template {
            /// Path to the review template file within the repository.
            template_path: Option<String>,
        },
    }
}

pub mod integration {
    #[derive(Debug, clap::Parser)]
    pub struct Platform {
        #[clap(subcommand)]
        pub cmd: Subcommands,
    }
    #[derive(Debug, clap::Subcommand)]
    pub enum Subcommands {
        /// Authenticate with your forge provider (at the moment, only GitHub is supported)
        Auth,
        /// List authenticated forge accounts known to GitButler
        ListUsers,
        /// Forget a previously authenticated forge account
        Forget {
            /// The username of the forge account to forget
            /// If not provided, you'll be prompted to select which account(s) to forget. If only one account exists, it will be forgotten automatically.
            username: Option<String>,
        },
    }
}

pub mod ci {
    #[derive(Debug, clap::Parser)]
    pub struct Platform {
        #[clap(subcommand)]
        pub cmd: Subcommands,
    }
    #[derive(Debug, clap::Subcommand)]
    pub enum Subcommands {
        /// Warm up the CI checks cache for all applied branches with PRs.
        /// This command is hidden because it's spawned automatically during initialization
        /// for background CI cache refresh. It also performs cleanup of stale cache entries.
        #[clap(hide = true)]
        Warm,
    }
}
