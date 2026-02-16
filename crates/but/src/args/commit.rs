#[derive(Debug, clap::Parser)]
pub struct Platform {
    /// Commit message
    #[clap(short = 'm', long = "message", conflicts_with = "message_file")]
    pub message: Option<String>,
    /// Read commit message from file
    #[clap(long = "message-file", value_name = "FILE", conflicts_with = "message")]
    pub message_file: Option<std::path::PathBuf>,
    /// Branch CLI ID or name to derive the stack to commit to
    pub branch: Option<String>,
    /// Whether to create a new branch for this commit.
    /// If the branch name given matches an existing branch, that branch will be used instead.
    /// If no branch name is given, a new branch with a generated name will be created.
    #[clap(short = 'c', long = "create")]
    pub create: bool,
    /// Only commit staged files, not unstaged files
    #[clap(short = 'o', long = "only")]
    pub only: bool,
    /// Bypass pre-commit hooks
    #[clap(short = 'n', long = "no-hooks", alias = "no-verify")]
    pub no_hooks: bool,
    /// Generate commit message using AI with optional user summary.
    /// Use --ai by itself or --ai="your instructions" (equals sign required for value)
    #[clap(
        short = 'i',
        long = "ai",
        conflicts_with = "message",
        conflicts_with = "message_file",
        num_args = 0..=1,
        require_equals = true
    )]
    pub ai: Option<Option<String>>,
    /// Uncommitted file or hunk CLI IDs to include in the commit.
    /// Can be specified multiple times or as comma-separated values.
    /// If not specified, all uncommitted changes (or changes staged to the target branch) are committed.
    #[clap(long = "changes", short = 'p', value_delimiter = ',', conflicts_with = "only")]
    pub changes: Vec<String>,
    #[clap(subcommand)]
    pub cmd: Option<Subcommands>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Insert a blank commit before or after the specified commit.
    ///
    /// This is useful for creating a placeholder commit that you can
    /// then amend changes into later using `but mark`, `but rub` or `but absorb`.
    ///
    /// You can modify the empty commit message at any time using `but reword`.
    ///
    /// This allows for a more Jujutsu style workflow where you create commits
    /// first and then fill them in as you work. Create an empty commit, mark it
    /// for auto-commit, and then just work on your changes. Write the commit
    /// message whenever you prefer.
    ///
    /// ## Examples
    ///
    /// Insert at the top of the first branch (no arguments):
    ///
    /// ```text
    /// but commit empty
    /// ```
    ///
    /// Insert before a commit:
    ///
    /// ```text
    /// but commit empty ab
    /// ```
    ///
    /// Explicitly insert before a commit:
    ///
    /// ```text
    /// but commit empty --before ab
    /// ```
    ///
    /// Insert after a commit (at the top of the stack if target is a branch):
    ///
    /// ```text
    /// but commit empty --after ab
    /// ```
    ///
    #[cfg(feature = "legacy")]
    #[clap(verbatim_doc_comment)]
    #[command(group = clap::ArgGroup::new("position"))]
    Empty {
        /// The target commit or branch to insert relative to.
        ///
        /// If a target is provided without --before or --after, defaults to --before behavior.
        /// If no arguments are provided at all, inserts at the top of the first branch.
        #[arg(group = "position")]
        target: Option<String>,
        /// Insert the blank commit before this commit or branch
        #[arg(long, group = "position")]
        before: Option<String>,
        /// Insert the blank commit after this commit or branch
        #[arg(long, group = "position")]
        after: Option<String>,
    },
}
