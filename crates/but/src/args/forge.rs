pub mod review {
    #[derive(Debug, clap::Parser)]
    pub struct Platform {
        #[clap(subcommand)]
        pub cmd: Subcommands,
    }
    #[derive(Debug, clap::Subcommand)]
    pub enum Subcommands {
        /// Publish review requests for active branches in your workspace.
        /// By default, publishes reviews for all active branches.
        Publish {
            /// Publish reviews only for the specified branch.
            #[clap(long, short = 'b')]
            branch: Option<String>,
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
            /// If the review contains only a single commit, the commit message will be used for the review title and description.
            #[clap(long, short = 't', default_value_t = false)]
            default: bool,
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
