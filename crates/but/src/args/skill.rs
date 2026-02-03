/// Arguments for skill management commands
#[derive(Debug, clap::Parser)]
pub struct Platform {
    #[clap(subcommand)]
    pub cmd: Subcommands,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Install the GitButler CLI skill files for Coding agents
    ///
    /// By default, installs the skill into the current repository. The command
    /// will prompt you to select a skill folder format (Claude Code, OpenCode, GitHub Copilot,
    /// Cursor, Windsurf) unless you specify a custom path with --path.
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
    /// but skill install --detect
    /// ```
    Install {
        /// Install the skill globally instead of in the current repository
        #[clap(long, short = 'g')]
        global: bool,
        /// Custom path where to install the skill (relative to repository root or absolute)
        #[clap(long, short = 'p')]
        path: Option<String>,
        /// Automatically detect where to install by finding existing installation
        #[clap(long, short = 'd')]
        detect: bool,
    },
    /// Check if installed GitButler skills are up to date with the CLI version
    ///
    /// Scans for installed skill files and compares their version with the current
    /// CLI version. By default, checks both local (repository) and global installations.
    ///
    /// ## Examples
    ///
    /// Check all installed skills:
    ///
    /// ```text
    /// but skill check
    /// ```
    ///
    /// Check and automatically update outdated skills:
    ///
    /// ```text
    /// but skill check --update
    /// ```
    ///
    /// Check only global installations:
    ///
    /// ```text
    /// but skill check --global
    /// ```
    Check {
        /// Only check global installations (in home directory)
        #[clap(long, short = 'g', conflicts_with = "local")]
        global: bool,
        /// Only check local installations (in current repository)
        #[clap(long, short = 'l', conflicts_with = "global")]
        local: bool,
        /// Automatically update any outdated skills found
        #[clap(long, short = 'u')]
        update: bool,
    },
}
