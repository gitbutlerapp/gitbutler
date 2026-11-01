/// Arguments for AI agent setup commands.
#[derive(Debug, clap::Parser)]
pub struct Platform {
    /// Running `but agent` with no subcommand runs the setup wizard.
    #[clap(subcommand)]
    pub cmd: Option<Subcommands>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Configure GitButler skills and workflow instructions for coding agents.
    ///
    /// Starts an interactive wizard that generates GitButler workflow steering,
    /// installs selected agent skills, and optionally writes the generated
    /// steering into agent instruction files.
    ///
    /// ## Examples
    ///
    /// Start the setup wizard:
    ///
    /// ```text
    /// but agent setup
    /// ```
    ///
    /// Print the default steering text without modifying files:
    ///
    /// ```text
    /// but agent setup --print
    /// ```
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Setup {
        /// Print the default generated steering text without prompting or modifying files.
        #[clap(long)]
        print: bool,
        /// Install discovery stubs that point agents at `but skill` instead of
        /// the full skill files. Team-internal while the approach is evaluated.
        #[clap(long, hide = true)]
        stub: bool,
    },
    /// Remove GitButler skills and workflow instructions from coding agents.
    ///
    /// Deletes every GitButler skill folder that `but agent setup` or
    /// `but skill install` created, in your home directory and in the current
    /// repository, and removes the GitButler section from agent instruction
    /// files such as `~/.codex/AGENTS.md`, `~/.claude/rules/gitbutler.md`, or a
    /// repository's `AGENTS.md`. Text outside the GitButler section is kept,
    /// and a file left empty is deleted. It lists everything it removed.
    ///
    /// ## Examples
    ///
    /// Remove GitButler from your coding agents:
    ///
    /// ```text
    /// but agent uninstall
    /// ```
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Uninstall,
}
