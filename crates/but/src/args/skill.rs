/// Arguments for skill management commands
#[derive(Debug, clap::Parser)]
#[clap(args_conflicts_with_subcommands = true)]
pub struct Platform {
    /// Running `but skill` with no subcommand prints the core skill guide.
    #[clap(subcommand)]
    pub cmd: Option<Subcommands>,
    /// Also print every reference document after the core guide.
    #[clap(long)]
    pub full: bool,
}

impl Platform {
    /// The embedded document a read-only invocation prints, if this is one.
    pub fn doc(&self) -> Option<&'static str> {
        match self.cmd {
            None => Some("core"),
            Some(Subcommands::Reference) => Some("reference"),
            Some(Subcommands::Concepts) => Some("concepts"),
            Some(Subcommands::Examples) => Some("examples"),
            Some(Subcommands::Install { .. } | Subcommands::Check { .. }) => None,
        }
    }
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Print the command reference: syntax and flags for every `but` command
    Reference,
    /// Print the concepts guide: the workspace model behind `but`
    Concepts,
    /// Print the workflow examples
    Examples,
    /// Install the GitButler CLI skill files for Coding agents
    ///
    /// By default, the command prompts you to choose installation scope first
    /// (current repository or global home directory), then prompts you to
    /// select a skill folder format (Agent Skills / .agents, Claude Code,
    /// OpenCode, Codex, GitHub Copilot, Cursor, Windsurf, Poolside) unless you
    /// specify a custom path with --path.
    /// When run outside a git repository, local scope is unavailable and the
    /// default install location is global (home directory). You can still
    /// install to a custom location with `--path` using an absolute or `~` path.
    ///
    /// Use --global to install the skill in a global location instead of the
    /// current repository.
    ///
    /// In non-interactive mode, a detected agent uses its global skill directory;
    /// otherwise specify --path or --detect.
    ///
    /// ## Examples
    ///
    /// Install interactively (prompts for scope and format):
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
    /// but skill install --path .agents/skills/gitbutler
    /// ```
    ///
    /// Auto-detect installation location (update existing installations):
    ///
    /// ```text
    /// but skill install --detect
    /// ```
    ///
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Install {
        /// Install the skill globally instead of in the current repository
        #[clap(long, short = 'g')]
        global: bool,
        /// Custom path where to install the skill (relative to repository root or absolute).
        /// Outside a repository, relative paths require --global.
        #[clap(long, short = 'p')]
        path: Option<String>,
        /// Refresh existing installations in place, updating every GitButler skill
        /// found in the current scope (local before global)
        #[clap(long, short = 'd')]
        detect: bool,
        /// Install a discovery stub that points agents at `but skill` instead of
        /// the full skill files. Team-internal while the approach is evaluated.
        #[clap(long, hide = true)]
        stub: bool,
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
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
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
