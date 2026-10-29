/// Run GitButler as a Model Context Protocol server.
#[derive(Debug, clap::Parser)]
pub struct Platform {
    #[clap(subcommand)]
    pub cmd: Subcommands,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Serve GitButler tools and MCP App resources over standard input/output.
    ///
    /// Workspace tools prefer filesystem roots supplied by the MCP client. A
    /// repository path can also be supplied to individual tool calls when the
    /// client does not expose the desired repository as a root.
    ///
    /// ## Examples
    ///
    /// ```text
    /// but mcp serve
    /// ```
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Serve,
}
