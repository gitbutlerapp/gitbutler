use std::path::PathBuf;

#[derive(Debug, clap::Parser)]
#[clap(name = "but", about = "A GitButler CLI tool", version = option_env!("GIX_VERSION"))]
pub struct Args {
    /// Run as if gitbutler-cli was started in PATH instead of the current working directory.
    #[clap(short = 'C', long, default_value = ".", value_name = "PATH")]
    pub current_dir: PathBuf,
    /// Whether to use JSON output format.
    #[clap(long, short = 'j')]
    pub json: bool,
    /// Subcommand to run.
    #[clap(subcommand)]
    pub cmd: Subcommands,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Starts up the MCP-internal server.
    McpInternal,
    /// Starts up the MCP server.
    Mcp,
    /// Automatically handle changes in the current repository, creating a commit with the provided context.
    HandleChanges {
        /// A context describing the changes that are currently uncommitted
        #[clap(long, short = 'd', alias = "desc", visible_alias = "description")]
        change_description: String,
        /// If true, this will perform simple, non-AI based handling.
        #[clap(long, short = 's', default_value_t = true)]
        simple: bool,
    },
    ListActions {
        /// The page number to list past actions from.
        #[clap(long, short = 'p', default_value_t = 1)]
        page: i64,
        /// The number of actions to list per page.
        #[clap(long, short = 's', default_value_t = 10)]
        page_size: i64,
    },
}
