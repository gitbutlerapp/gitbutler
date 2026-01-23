#[derive(Debug, clap::Parser)]
pub struct Platform {
    /// Commit message
    #[clap(short = 'm', long = "message", conflicts_with = "file")]
    pub message: Option<String>,
    /// Read commit message from file
    #[clap(
        short = 'f',
        long = "file",
        value_name = "FILE",
        conflicts_with = "message"
    )]
    pub file: Option<std::path::PathBuf>,
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
    /// Insert before a commit:
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
    #[command(group = clap::ArgGroup::new("position").required(true))]
    Empty {
        /// Insert the blank commit before this commit or branch
        #[arg(long, group = "position")]
        before: Option<String>,
        /// Insert the blank commit after this commit or branch
        #[arg(long, group = "position")]
        after: Option<String>,
    },
}
