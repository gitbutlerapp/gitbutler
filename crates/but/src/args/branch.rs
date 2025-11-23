#[derive(Debug, clap::Parser)]
pub struct Platform {
    #[clap(subcommand)]
    pub cmd: Option<Subcommands>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Creates a new branch in the workspace
    ///
    /// If no branch name is provided, a new parallel branch with a generated
    /// name will be created.
    ///
    /// You can also specify an anchor point using the `--anchor` option,
    /// which can be either a commit ID or an existing branch name to create
    /// the new branch from. This allows you to create stacked branches.
    ///
    New {
        /// Name of the new branch
        branch_name: Option<String>,
        /// Anchor point - either a commit ID or branch name to create the new branch from
        #[clap(long, short = 'a')]
        anchor: Option<String>,
    },

    /// Deletes a branch from the workspace
    ///
    /// This will remove the branch and all its commits from the workspace.
    /// If the branch has unpushed commits, you will be prompted for confirmation
    /// unless the `--force` flag is used.
    ///
    #[clap(short_flag = 'd')]
    Delete {
        /// Name of the branch to delete
        branch_name: String,
        /// Force deletion without confirmation
        #[clap(long, short = 'f')]
        force: bool,
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
    Show {
        /// CLI ID or name of the branch to show
        branch_id: String,
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

    /// Apply a branch to the workspace
    ///
    /// If you want to apply an unapplied branch to your workspace so you
    /// can work on it, you can run `but branch apply <branch-name>`.`
    ///
    /// This will apply the changes in that branch into your working directory
    /// as a parallel applied branch.
    ///
    Apply {
        /// Name of the branch to apply
        branch_name: String,
    },

    /// Unapply a branch from the workspace
    ///
    /// If you want to unapply an applied branch from your workspace
    /// (effectively stashing it) so you can work on other branches,
    /// you can run `but branch unapply <branch-name>`.
    ///
    /// This will remove the changes in that branch from your working
    /// directory and you can re-apply it later when needed. You will then
    /// see the branch as unapplied in `but branch list`.
    ///
    Unapply {
        /// Name of the branch to unapply
        branch_name: String,
        /// Force unapply without confirmation
        #[clap(long, short = 'f')]
        force: bool,
    },
}
