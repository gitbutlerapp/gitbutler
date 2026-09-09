pub mod pr {
    #[derive(Debug, clap::Parser)]
    pub struct Platform {
        #[clap(subcommand)]
        pub cmd: Option<Subcommands>,
        /// Create the review as a draft.
        #[clap(long, short = 'd', default_value_t = false)]
        pub draft: bool,
    }
    #[derive(Debug, clap::Subcommand)]
    pub enum Subcommands {
        /// Create a new review for a branch, force-pushing it first.
        ///
        /// If the branch is part of a stack, GitButler pushes that branch and its ancestors
        /// and creates missing reviews from the bottom upward. It also updates stack metadata
        /// using native GitHub stacks when enabled and supported, or review descriptions
        /// otherwise.
        New {
            /// The branch to create a review for. Without it, a terminal prompts for one (or
            /// confirms when only one branch lacks a review); a non-interactive run needs it.
            #[clap(value_name = "BRANCH")]
            branch: Option<String>,
            /// Review title and description: the first line is the title, the rest is the
            /// description. A non-interactive run needs `-m`, `-F`, or `-t`.
            #[clap(short = 'm', long = "message", conflicts_with_all = &["file", "default"])]
            message: Option<String>,
            /// Read review title and description from file. The first line is the title, the rest is the description.
            #[clap(short = 'F', long = "file", value_name = "FILE", conflicts_with_all = &["message", "default"])]
            file: Option<std::path::PathBuf>,
            /// Force push even if it's not fast-forward. Always on; the flag is kept for
            /// compatibility.
            #[clap(long, short = 'f', default_value_t = true, hide = true)]
            with_force: bool,
            /// Skip force push protection checks
            #[clap(long, short = 's')]
            skip_force_push_protection: bool,
            /// Bypass pre-push hooks.
            #[clap(long = "no-hooks", visible_alias = "no-verify")]
            no_hooks: bool,
            /// Use the default content for the review title and description, skipping any prompts.
            /// If the branch contains only a single commit, the commit message will be used.
            #[clap(long, short = 't', default_value_t = false)]
            default: bool,
            /// Create the review as a draft.
            #[clap(long, short = 'd', default_value_t = false)]
            draft: bool,
        },
        /// Enable or disable the automatic merging of reviews.
        AutoMerge {
            /// One or more comma-separated branch names, branch IDs, stack IDs (every review on
            /// the stack), or review numbers (the PR or MR number without the symbol). Without
            /// it, a terminal prompts for reviews from the workspace's branches; a
            /// non-interactive run needs it.
            #[clap(value_name = "SELECTOR")]
            selector: Option<String>,
            /// Disable automatic merging instead of enabling it
            #[clap(long, short = 'd', default_value_t = false)]
            off: bool,
        },
        /// Mark existing reviews as draft.
        SetDraft {
            /// One or more comma-separated branch names, branch IDs, stack IDs (every review on
            /// the stack), or review numbers (the PR or MR number without the symbol). Without
            /// it, a terminal prompts for reviews from the workspace's branches; a
            /// non-interactive run needs it.
            #[clap(value_name = "SELECTOR")]
            selector: Option<String>,
        },
        /// Mark existing reviews as ready for review.
        SetReady {
            /// One or more comma-separated branch names, branch IDs, stack IDs (every review on
            /// the stack), or review numbers (the PR or MR number without the symbol). Without
            /// it, a terminal prompts for reviews from the workspace's branches; a
            /// non-interactive run needs it.
            #[clap(value_name = "SELECTOR")]
            selector: Option<String>,
        },
        /// Configure the template to use for review descriptions.
        Template {
            /// Path to the review template file within the repository. Without it, a terminal
            /// lists the templates found in the repository to pick from; a non-interactive run
            /// needs it.
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
