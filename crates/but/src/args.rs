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
    /// Show commits on active branches in your workspace.
    Log,
    /// Overview of the uncommitted changes in the repository.
    #[clap(alias = "st")]
    Status {
        /// Determines whether the committed files should be shown as well.
        #[clap(short = 'f', alias = "files", default_value_t = false)]
        show_files: bool,
        /// Show verbose output with commit author and timestamp.
        #[clap(short = 'v', long = "verbose", default_value_t = false)]
        verbose: bool,
    },
    /// Overview of the uncommitted changes in the repository with files shown.
    /// Equivalent to `but status --files`.
    #[clap(hide = true)]
    Stf {
        /// Show verbose output with commit author and timestamp.
        #[clap(short = 'v', long = "verbose", default_value_t = false)]
        verbose: bool,
    },

    /// Combines two entities together to perform an operation.
    #[clap(
        about = "Combines two entities together to perform an operation",
        long_about = "Combines two entities together to perform an operation.

Non-exhaustive list of operations:
      │Source     │Target
──────┼───────────┼──────
Amend │File,Branch│Commit
Squash│Commit     │Commit
Assign│File,Branch│Branch
Move  │Commit     │Branch

For examples see `but rub --help`."
    )]
    Rub {
        /// The source entity to combine
        source: String,
        /// The target entity to combine with the source
        target: String,
    },
    /// Initializes a GitButler project from a git repository in the current directory.
    Init {
        /// Also initializes a git repository in the current directory if one does not exist.
        #[clap(long, short = 'r')]
        repo: bool,
    },
    /// Commands for managing the base.
    Base(crate::base::Platform),
    /// Commands for managing branches.
    Branch(crate::branch::Platform),
    /// Creates or removes a rule for auto-assigning or auto-comitting
    Mark {
        /// The target entity that will be marked
        target: String,
        /// Deletes a mark
        #[clap(long, short = 'd')]
        delete: bool,
    },
    /// Removes all marks from the workspace
    Unmark,
    /// Commit changes to a stack.
    Commit {
        /// Commit message
        #[clap(short = 'm', long = "message")]
        message: Option<String>,
        /// Branch CLI ID or name to derive the stack to commit to
        branch: Option<String>,
        /// Only commit assigned files, not unassigned files
        #[clap(short = 'o', long = "only")]
        only: bool,
    },
    /// Insert a blank commit before the specified commit, or at the top of a stack.
    New {
        /// Commit ID to insert before, or branch ID to insert at top of stack
        target: String,
    },
    /// Edit the commit message of the specified commit.
    #[clap(alias = "desc")]
    Describe {
        /// Commit ID to edit the message for
        commit: String,
    },
    /// Show operation history (last 20 entries).
    Oplog {
        /// Start from this oplog SHA instead of the head
        #[clap(long)]
        since: Option<String>,
    },
    /// Restore to a specific oplog snapshot.
    Restore {
        /// Oplog SHA to restore to
        oplog_sha: String,
        /// Skip confirmation prompt
        #[clap(short = 'f', long = "force")]
        force: bool,
    },
    /// Undo the last operation by reverting to the previous snapshot.
    Undo,
    /// Starts up the MCP server.
    Mcp {
        /// Starts the internal MCP server which has more granular tools.
        #[clap(long, short = 'i', hide = true)]
        internal: bool,
    },
    /// GitButler Actions are automated tasks (like macros) that can be peformed on a repository.
    #[clap(hide = true)]
    Actions(actions::Platform),
    // Claude hooks
    #[clap(hide = true)]
    Claude(claude::Platform),
    // Cursor hooks
    #[clap(hide = true)]
    Cursor(cursor::Platform),
    /// If metrics are permitted, this subcommand handles posthog event creation.
    #[clap(hide = true)]
    Metrics {
        #[clap(long, value_enum)]
        command_name: CommandName,
        #[clap(long)]
        props: String,
    },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum, Default)]
pub enum CommandName {
    #[clap(alias = "log")]
    Log,
    #[clap(alias = "st")]
    Status,
    #[clap(alias = "stf", hide = true)]
    Stf,
    #[clap(alias = "rub")]
    Rub,
    #[clap(alias = "commit")]
    Commit,
    #[clap(alias = "new")]
    New,
    #[clap(alias = "describe")]
    Describe,
    #[clap(alias = "oplog")]
    Oplog,
    #[clap(alias = "restore")]
    Restore,
    #[clap(alias = "undo")]
    Undo,
    BaseCheck,
    BaseUpdate,
    BranchNew,
    #[clap(
        alias = "claude-pre-tool",
        alias = "claudepretool",
        alias = "claudePreTool",
        alias = "ClaudePreTool"
    )]
    ClaudePreTool,
    #[clap(
        alias = "claude-post-tool",
        alias = "claudeposttool",
        alias = "claudePostTool",
        alias = "ClaudePostTool"
    )]
    ClaudePostTool,
    #[clap(
        alias = "claude-bash-post-tool",
        alias = "claudebashposttool",
        alias = "claudeBashPostTool",
        alias = "ClaudeBashPostTool"
    )]
    ClaudeBashPostTool,
    #[clap(
        alias = "claude-stop",
        alias = "claudestop",
        alias = "claudeStop",
        alias = "ClaudeStop"
    )]
    ClaudeStop,
    #[clap(
        alias = "cursor-after-edit",
        alias = "cursorafteredit",
        alias = "cursorAfterEdit",
        alias = "CursorAfterEdit"
    )]
    CursorAfterEdit,
    #[clap(
        alias = "cursor-stop",
        alias = "cursorstop",
        alias = "cursorStop",
        alias = "CursorStop"
    )]
    CursorStop,
    #[default]
    Unknown,
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

pub mod claude {
    #[derive(Debug, clap::Parser)]
    pub struct Platform {
        #[clap(subcommand)]
        pub cmd: Subcommands,
    }
    #[derive(Debug, clap::Subcommand)]
    pub enum Subcommands {
        #[clap(alias = "pre-tool-use")]
        PreTool,
        #[clap(alias = "post-tool-use")]
        PostTool,
        #[clap(alias = "bash-post-tool-use")]
        BashPostTool,
        Stop,
        #[clap(alias = "pp")]
        PermissionPromptMcp,
    }
}

pub mod cursor {
    #[derive(Debug, clap::Parser)]
    pub struct Platform {
        #[clap(subcommand)]
        pub cmd: Subcommands,
    }
    #[derive(Debug, clap::Subcommand)]
    pub enum Subcommands {
        AfterEdit,
        Stop {
            #[clap(long, default_value = "false")]
            nightly: bool,
        },
    }
}
