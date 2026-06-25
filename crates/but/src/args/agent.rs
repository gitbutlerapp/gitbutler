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
    },
}
