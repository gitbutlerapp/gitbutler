/// Arguments for skill management commands
#[derive(Debug, clap::Parser)]
pub struct Platform {
    #[clap(subcommand)]
    pub cmd: Subcommands,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Install the Claude skill files into the repository
    ///
    /// By default, installs the skill into the current repository. The command
    /// will prompt you to select a skill folder format (Claude Code, GitHub Copilot, etc.)
    /// unless you specify a custom path with --path.
    ///
    /// Use --global to install the skill in a global location instead of the
    /// current repository.
    ///
    /// ## Examples
    ///
    /// Install in current repository (prompts for format):
    ///
    /// ```text
    /// but skill install
    /// ```
    ///
    /// Install globally (prompts for format):
    ///
    /// ```text
    /// but skill install --global
    /// ```
    ///
    /// Install to a custom path:
    ///
    /// ```text
    /// but skill install --path .claude/skills/gitbutler
    /// ```
    ///
    /// Auto-detect installation location (update existing installation):
    ///
    /// ```text
    /// but skill install --infer
    /// ```
    Install {
        /// Install the skill globally instead of in the current repository
        #[clap(long, short = 'g')]
        global: bool,
        /// Custom path where to install the skill (relative to repository root or absolute)
        #[clap(long, short = 'p')]
        path: Option<String>,
        /// Automatically infer where to install by detecting existing installation
        #[clap(long, short = 'i')]
        infer: bool,
    },
}
