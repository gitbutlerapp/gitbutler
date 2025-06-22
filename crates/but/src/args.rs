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
    /// Whether tracing should occur
    #[clap(long, short = 't')]
    pub trace: bool,
    /// Subcommand to run.
    #[clap(subcommand)]
    pub cmd: Subcommands,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Provides an overview of the Workspace commit graph.
    Log,
    /// Overview of the oncommitted changes in the repository.
    Status,

    /// Combines two entities together to perform an operation.
    #[clap(about = "Combines two entities together to perform an operation.
Non-exhaustive list of operations 
      │Source     │Target
──────┼───────────┼──────
Amend │File,Branch│Commit
Squash│Commit     │Commit
Assign│File,Branch│Branch
Move  │Commit     │Branch

For examples `but rub --help`.")]
    Rub {
        /// The source entity to combine
        source: String,
        /// The target entity to combine with the source
        target: String,
    },
    /// Starts up the MCP server.
    Mcp {
        /// Starts the internal MCP server which has more granular tools.
        #[clap(long, short = 'i')]
        internal: bool,
    },
    /// GitButler Actions are automated tasks (like macros) that can be peformed on a repository.
    Actions(actions::Platform),
    /// The family of but inspection commands
    BetaInspect(Inspect),
}

pub mod actions {
    #[derive(Debug, clap::Parser)]
    pub struct Platform {
        #[clap(subcommand)]
        pub cmd: Option<Subcommands>,
    }
    #[derive(Debug, clap::Subcommand)]
    pub enum Subcommands {
        /// Automatically handles the changes in the repository, creating a commit with the provided context.
        HandleChanges {
            /// A context describing the changes that are currently uncommitted
            #[clap(long, short = 'd', alias = "desc", visible_alias = "description")]
            description: String,
            /// Which handler is to be used for the operation. Different handles would have different behavior.
            #[clap(long, value_enum, default_value = "simple")]
            handler: Handler,
        },
    }

    #[derive(Debug, Clone, Copy, clap::ValueEnum)]
    pub enum Handler {
        /// Handles changes in a simple way.
        Simple,
    }
}

#[derive(Debug, clap::Parser)]
pub struct Inspect {
    #[clap(subcommand)]
    pub cmd: InspectSubcommands,
}

#[derive(Debug, clap::Subcommand)]
pub enum InspectSubcommands {
    /// Get the current index status
    Status,
    /// Generate embeddings
    Generate,
}
