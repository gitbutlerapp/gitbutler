#[cfg(feature = "legacy")]
use crate::args::atoms::{AllowMergedArg, BranchArg, CliIdArg};

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, Default)]
pub enum IntegrationStrategy {
    /// Rebuilds the branch picking first the commits on the remote, and then the commits on the local branch.
    #[default]
    PullRebase,
    /// Tries to fold matching remote work into related local commits.
    /// This is done through matching Change IDs, and falling back to pull-rebase ordering otherwise.
    SmartSquash,
    /// Keeps your local history and merges the remote tip into it.
    Merge,
    /// Rebuilds the branch picking only the commits on the remote.
    PickRemote,
}

#[derive(Debug, clap::Parser)]
#[clap(after_help = "To rename an applied branch, use `but reword <branch> -m <new-name>`.")]
pub struct Platform {
    #[clap(subcommand)]
    pub cmd: Option<Subcommands>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    #[cfg(feature = "legacy")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    New(NewPlatform),

    /// Delete branchs from the workspace
    ///
    #[cfg(feature = "legacy")]
    #[clap(short_flag = 'd')]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Delete {
        /// One or more branches to delete.
        #[clap(required = true)]
        branches: Vec<CliIdArg>,
    },

    /// List the branches in the repository
    ///
    /// By default, shows the active branch and the 20 most recently updated branches.
    ///
    /// You can use the `--all` flag to show all branches, `--local` to show only
    /// local branches, or `--remote` to show only remote branches.
    ///
    /// You can also filter branch names by specifying a substring, such as
    /// `but branch list feature` to show only branches with "feature" in the name.
    ///
    /// If you want to check for review status, you can add `--review` to fetch
    /// and display pull request or merge request information for each branch.
    /// This will make the command slower as it needs to query the forge.
    ///
    /// By default, the command checks if each branch merges cleanly into
    /// the *upstream base target branch* (not your workspace).
    /// You can disable this check with `--no-check` to make the command faster.
    ///
    /// By default it also calculates the number of commits each branch is ahead
    /// of the base branch. You can disable this with `--no-ahead` to
    /// make the command faster.
    ///
    #[cfg(feature = "legacy")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    List {
        /// Filter branches by name (case-insensitive substring match)
        filter: Option<String>,
        /// Show only local branches
        #[clap(long, short = 'l', conflicts_with = "remote")]
        local: bool,
        /// Show only remote branches
        #[clap(long, short = 'r', conflicts_with = "local")]
        remote: bool,
        /// Show all branches (not just active + 20 most recent)
        #[clap(long, short = 'a')]
        all: bool,
        /// Don't calculate and show number of commits ahead of base (faster)
        #[clap(long)]
        no_ahead: bool,
        /// Fetch and display review information (PRs, MRs, etc.)
        #[clap(long)]
        review: bool,
        /// Don't check if each branch merges cleanly into upstream
        #[clap(long)]
        no_check: bool,
        /// Include branches with no commits on them (hidden by default)
        #[clap(long)]
        empty: bool,
    },

    /// Show commits ahead of base for a specific branch
    ///
    /// This shows the list of commits that are on the specified branch
    /// but not yet integrated into the base target branch.
    ///
    /// You can also choose to fetch and display review information,
    /// show files modified in each commit with line counts, generate
    /// an AI summary of the branch changes, and check if the branch
    /// merges cleanly into upstream.
    ///
    #[cfg(feature = "legacy")]
    Show {
        /// CLI ID or name of the branch to show
        branch: CliIdArg,
        /// Fetch and display review information
        #[clap(short, long)]
        review: bool,
        /// Show files modified in each commit with line counts
        #[clap(short, long)]
        files: bool,
        /// Generate AI summary of the branch changes
        #[clap(long)]
        ai: bool,
        /// Check if the branch merges cleanly into upstream and identify conflicting commits
        #[clap(long)]
        check: bool,
    },

    /// Deprecated: use `but move` instead
    #[clap(hide = true)]
    Move {
        #[clap(trailing_var_arg = true, allow_hyphen_values = true)]
        _args: Vec<String>,
    },

    /// Update your local branch with the content of its remote counterpart.
    ///
    /// This allows you to resolve the divergence between your local branch and its
    /// tracked remote in different ways.
    #[clap(short_flag = 'u')]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Update {
        /// Name of the local branch to integrate
        branch: String,
        /// Strategy to use for the integration. If no strategy is specified, we default
        /// to pull-rebase.
        #[clap(long, short = 's', value_enum, default_value_t)]
        strategy: IntegrationStrategy,
        /// Preview the resulting branch state without persisting changes
        #[clap(long)]
        dry_run: bool,
        /// Show additional dry-run details like the current divergence
        #[clap(long, short = 'v')]
        verbose: bool,
        /// Open the generated integration script in an editor
        #[clap(long, short = 'i')]
        interactive: bool,
    },
}

/// Create a new branch.
///
/// Use `--above` or `--below` to created stacked branches. Omitting these create a new unstacked
/// branch.
///
/// For more details about CLI IDs, see `but help cli-ids`.
#[cfg(feature = "legacy")]
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct NewPlatform {
    /// Place the branch above `BRANCH_OR_COMMIT`, which must be an applied branch or commit.
    ///
    /// If `BRANCH_OR_COMMIT` is a commit, the new branch is created above the commit.
    ///
    /// If `BRANCH_OR_COMMIT` is a branch, the new branch is created above the targeted branch.
    #[clap(
        short = 'A',
        long,
        value_name = "BRANCH_OR_COMMIT",
        group = "targeting"
    )]
    pub above: Option<CliIdArg>,

    /// Deprecated flag that will be removed in a future release. Use `--above` instead.
    #[clap(
        short,
        long,
        value_name = "BRANCH_OR_COMMIT",
        group = "targeting",
        hide = true
    )]
    pub anchor: Option<CliIdArg>,

    /// Place the branch below `BRANCH_OR_COMMIT`, which must be an applied branch or commit.
    ///
    /// If `BRANCH_OR_COMMIT` is a commit, the new branch is created below the commit.
    ///
    /// If `BRANCH_OR_COMMIT` is a branch, the new branch is created below the targeted branch.
    #[clap(
        short = 'B',
        long,
        value_name = "BRANCH_OR_COMMIT",
        group = "targeting"
    )]
    pub below: Option<CliIdArg>,

    /// Name of the new branch.
    ///
    /// If omitted the new branch will get a generated name.
    pub name: Option<BranchArg>,

    #[clap(flatten)]
    #[allow(missing_docs)]
    pub allow_merged: AllowMergedArg,
}
