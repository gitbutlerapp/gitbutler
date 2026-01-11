/// Command-line argument parsing for GitButler CLI.
/// Uses `clap` for defining commands and options.
///
/// This module defines the main `Args` struct which represents the top-level
/// command-line interface, along with its subcommands and options.
///
/// Nearly all documentation for the CLI is defined here using `clap` attributes,
/// which are then used to generate help messages and online documentation.
use std::path::PathBuf;

#[derive(Debug, clap::Parser)]
#[clap(
    name = "but",
    about = "A GitButler CLI tool",
    version = option_env!("VERSION").unwrap_or("dev"),
    disable_help_subcommand = true
)]
pub struct Args {
    /// Enable tracing for debug and performance information printed to stderr.
    #[clap(short = 't', long, action = clap::ArgAction::Count, hide = true, env = "BUT_TRACE")]
    pub trace: u8,
    /// Run as if gitbutler-cli was started in PATH instead of the current working directory.
    #[clap(short = 'C', long, default_value = ".", value_name = "PATH")]
    pub current_dir: PathBuf,
    /// Explicitly control how output should be formatted.
    ///
    /// If unset and from a terminal, it defaults to human output, when redirected it's for shells.
    #[clap(
        long,
        short = 'f',
        env = "BUT_OUTPUT_FORMAT",
        conflicts_with = "json",
        default_value = "human"
    )]
    pub format: OutputFormat,
    /// Whether to use JSON output format.
    #[clap(long, short = 'j', global = true)]
    pub json: bool,
    /// Source entity for rub operation (when no subcommand is specified).
    /// If no target is specified, this is treated as a path to open on the GUI.
    #[clap(value_name = "SOURCE")]
    pub source_or_path: Option<String>,
    /// Target entity for rub operation (when no subcommand is specified).
    #[clap(value_name = "TARGET", requires = "source_or_path")]
    pub target: Option<String>,
    /// Subcommand to run.
    #[clap(subcommand)]
    pub cmd: Option<Subcommands>,
}

/// How we should format anything written to [`std::io::stdout()`].
#[derive(Debug, Copy, Clone, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// The output to write is supposed to be for human consumption, and can be more verbose.
    #[default]
    Human,
    /// The output should be suitable for shells, and assigning the major result to variables so that it can be reused
    /// in subsequent CLI invocations.
    Shell,
    /// Output detailed information as JSON for tool consumption.
    Json,
    /// Do not output anything, like redirecting to `/dev/null`.
    None,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Overview of the project workspace state.
    ///
    /// This shows unstaged files, files staged to stacks, all applied
    /// branches (stacked or parallel), commits on each of those branches,
    /// upstream commits that are unintegrated, commit status (pushed or local),
    /// and base branch information.
    ///
    /// ## Examples
    ///
    /// Normal usage:
    ///
    /// ```text
    /// but status
    /// ```
    ///
    /// Shorthand with listing files modified
    ///
    /// ```text
    /// but status -f
    /// ```
    ///
    #[cfg(feature = "legacy")]
    Status {
        /// Determines whether the committed files should be shown as well.
        #[clap(short = 'f', alias = "files", default_value_t = false)]
        show_files: bool,
        /// Show verbose output with commit author and timestamp.
        #[clap(short = 'v', long = "verbose", default_value_t = false)]
        verbose: bool,
        /// Forces a sync of pull requests from the forge before showing status.
        #[clap(short = 'r', long = "refresh-prs", default_value_t = false)]
        refresh_prs: bool,
        /// Show detailed list of upstream commits that haven't been integrated yet.
        #[clap(short = 'u', long = "upstream", default_value_t = false)]
        upstream: bool,
        /// Show a hint about available commands at the end of output.
        #[clap(long = "hint", default_value_t = false)]
        hint: bool,
    },

    /// Combines two entities together to perform an operation like amend, squash, stage, or move.
    ///
    /// The `rub` command is a simple verb that helps you do a number of editing
    /// operations by doing combinations of two things.
    ///
    /// For example, you can "rub" a file onto a branch to stage that file to
    /// the branch. You can also "rub" a commit onto another commit to squash
    /// them together. You can rub a commit onto a branch to move that commit.
    /// You can rub a file from one commit to another.
    ///
    /// Non-exhaustive list of operations:
    ///
    /// ```text
    ///       │Source     │Target
    /// ──────┼───────────┼──────
    /// Amend │File,Branch│Commit
    /// Squash│Commit     │Commit
    /// Stage │File,Branch│Branch
    /// Move  │Commit     │Branch
    /// ```
    ///
    /// ## Examples
    ///
    /// Squashing two commits into one (combining the commit messages):
    ///
    /// ```text
    /// but rub 3868155 abe3f53f
    /// ```
    ///
    /// Amending a commit with the contents of a modified file:
    ///
    /// ```text
    /// but rub README.md abe3f53f
    /// ```
    ///
    /// Moving a commit from one branch to another:
    ///
    /// ```text
    /// but rub 3868155 feature-branch
    /// ```
    ///
    #[cfg(feature = "legacy")]
    Rub {
        /// The source entity to combine
        source: String,
        /// The target entity to combine with the source
        target: String,
    },

    /// Displays the diff of changes in the repo.
    ///
    /// Without any arguments, it shows the diff of all uncommitted changes.
    /// Optionally, a CLI ID argument can be provided, which chan show the diff specific to
    /// - an uncommitted file
    /// - a branch
    /// - an entire stack
    /// - a commit
    /// - a file change within a commit
    #[cfg(feature = "legacy")]
    Diff {
        /// The CLI ID of the entity to show the diff for
        target: Option<String>,
    },

    /// Shows detailed information about a commit.
    ///
    /// Displays the full commit message, author information, committer information
    /// (if different from author), and the list of files modified in the commit.
    ///
    /// ## Examples
    ///
    /// Show commit details by short commit ID:
    ///
    /// ```text
    /// but show a1b2c3d
    /// ```
    ///
    /// Show commit details by CLI ID:
    ///
    /// ```text
    /// but show c5
    /// ```
    ///
    #[cfg(feature = "legacy")]
    Show {
        /// The commit ID (short or full SHA) or CLI ID to show details for
        commit: String,
    },

    /// Initializes a GitButler project from a git repository in the current directory.
    ///
    /// If you have an existing Git repository and want to start using GitButler
    /// with it, you can run this command to set up the necessary configuration
    /// and data structures.
    ///
    /// This is automatically run when you run any other `but` command in
    /// a git repository that is not yet initialized with GitButler.
    ///
    /// Note: Currently, if there is no Git repository already, you will need to
    /// initialize it with `git init` and add a remote first, as GitButler needs
    /// a remote to base the branches on.
    ///
    /// We are working on removing this limitation, but for now this is something
    /// to be aware of.
    #[cfg(feature = "legacy")]
    Init {
        /// Also initializes a git repository in the current directory if one does not exist.
        #[clap(long, short = 'r')]
        repo: bool,
    },

    /// Updates all applied branches to be up to date with the target branch.
    ///
    /// This fetches the latest changes from the remote and rebases all applied branches
    /// on top of the updated target branch.
    ///
    /// You should run this regularly to keep your branches up to date with the latest
    /// changes from the main development line.
    ///
    /// You can run `but pull --check` first to see if your branches can be cleanly
    /// merged into the target branch before running the update.
    ///
    #[cfg(feature = "legacy")]
    Pull {
        /// Only check the status without updating (equivalent to the old `but base check`)
        #[clap(long, short = 'c')]
        check: bool,
    },

    /// Commands for managing branches.
    ///
    /// This includes creating, deleting, listing, showing details about, and
    /// applying and unapplying branches.
    ///
    /// By default without a subcommand, it will list the branches.
    ///
    Branch(branch::Platform),

    /// Commands for managing worktrees.
    ///
    /// GitButler worktrees allow you to have multiple working directories
    /// associated with a single Git repository, each tied to a specific
    /// GitButler branch.
    ///
    /// This can be useful for working on multiple versions of a branch at
    /// the same time, or for isolating changes in different workspaces.
    ///
    #[cfg(feature = "legacy")]
    #[clap(hide = true)]
    Worktree(worktree::Platform),

    /// Mark a commit or branch for auto-stage or auto-commit.
    ///
    /// Creates or removes a rule for auto-staging or auto-committing changes
    /// to the specified target entity.
    ///
    /// If you mark a branch, new unstaged changes that GitButler sees when
    /// you run any command will be automatically staged to that branch.
    ///
    /// If you mark a commit, new uncommitted changes will automatically be
    /// amended into the marked commit.
    ///
    #[cfg(feature = "legacy")]
    Mark {
        /// The target entity that will be marked
        target: String,
        /// Deletes a mark
        #[clap(long, short = 'd')]
        delete: bool,
    },

    /// Removes any marks from the workspace
    ///
    /// This will unmark anything that has been marked by the `but mark` command.
    ///
    #[cfg(feature = "legacy")]
    Unmark,

    /// Open the GitButler GUI for the current project.
    ///
    /// Running `but gui` will launch the GitButler graphical user interface
    /// in the current directory's GitButler project.
    ///
    /// This provides a visual way to manage branches, commits, and uncommitted
    /// changes, complementing the command-line interface.
    ///
    /// You can also just run `but .` as a shorthand to open the GUI.
    ///
    #[clap(visible_alias = ".")]
    Gui,

    /// Commit changes to a stack.
    ///
    /// The `but commit` command allows you to create a new commit
    /// on a specified branch (stack) with the current uncommitted changes.
    ///
    /// If there is only one branch applied, it will commit to that branch by default.
    ///
    /// If there are multiple branches applied, you must specify which branch to
    /// commit to, or if in interactive mode, you will be prompted to select one.
    ///
    /// By default, all uncommitted changes and all changes already staged to that
    /// branch will be included in the commit. If you only want to commit the changes
    /// that are already staged to that branch, you can use the `--only` flag.
    ///
    /// It will not commit changes staged to other branches.
    ///
    #[cfg(feature = "legacy")]
    Commit {
        /// Commit message
        #[clap(short = 'm', long = "message", conflicts_with = "file")]
        message: Option<String>,
        /// Read commit message from file
        #[clap(
            short = 'f',
            long = "file",
            value_name = "FILE",
            conflicts_with = "message"
        )]
        file: Option<std::path::PathBuf>,
        /// Branch CLI ID or name to derive the stack to commit to
        branch: Option<String>,
        /// Whether to create a new branch for this commit.
        /// If the branch name given matches an existing branch, that branch will be used instead.
        /// If no branch name is given, a new branch with a generated name will be created.
        #[clap(short = 'c', long = "create")]
        create: bool,
        /// Only commit staged files, not unstaged files
        #[clap(short = 'o', long = "only")]
        only: bool,
    },

    /// Push changes in a branch to remote.
    ///
    /// `but push` will update the remote with the latest commits from the
    /// applied branch(es).
    ///
    /// Without a branch ID:
    /// - Interactive mode: Lists all branches with unpushed commits and prompts for selection
    /// - Non-interactive mode: Automatically pushes all branches with unpushed commits
    ///
    /// With a branch ID:
    /// - `but push bu` - push the branch with CLI ID "bu"
    /// - `but push feature-branch` - push the branch named "feature-branch"
    ///
    #[cfg(feature = "legacy")]
    Push(push::Command),

    /// Insert a blank commit before the specified commit, or at the top of a stack.
    ///
    /// This is useful for creating a placeholder commit that you can
    /// then amend changes into later using `but mark`, `but rub` or `but absorb`.
    ///
    /// You can modify the empty commit message at any time using `but describe`.
    ///
    /// This allows for a more Jujutsu style workflow where you create commits
    /// first and then fill them in as you work. Create an empty commit, mark it
    /// for auto-commit, and then just work on your changes. Write the commit
    /// message whenever you prefer.
    ///
    #[cfg(feature = "legacy")]
    New {
        /// Commit ID to insert before, or branch ID to insert at top of stack
        target: String,
    },

    /// Edit the commit message of the specified commit.
    ///
    /// You can easily change the commit message of any of your commits by
    /// running `but reword <commit-id>` and providing a new message in the
    /// editor.
    ///
    /// This will rewrite the commit with the new message and then rebase any
    /// dependent commits on top of it.
    ///
    /// You can also use `but reword <branch-id>` to rename the branch.
    ///
    #[cfg(feature = "legacy")]
    Reword {
        /// Commit ID to edit the message for, or branch ID to rename
        target: String,
        /// The new commit message or branch name. If not provided, opens an editor.
        #[clap(short = 'm', long = "message")]
        message: Option<String>,
    },

    /// Commands for viewing and managing operation history.
    ///
    /// Displays a list of past operations performed in the repository,
    /// including their timestamps and descriptions.
    ///
    /// This allows you to restore to any previous point in the history of the
    /// project. All state is preserved in operations, including uncommitted changes.
    ///
    /// You can use `but restore <oplog-sha>` to restore to a specific state.
    ///
    /// By default, shows the last 20 oplog entries (same as `but oplog list`).
    ///
    #[cfg(feature = "legacy")]
    Oplog(oplog::Platform),

    /// Restore to a specific oplog snapshot.
    ///
    /// This command allows you to revert the repository to a previous state
    /// captured in an oplog snapshot.
    ///
    /// You need to provide the SHA of the oplog entry you want to restore to,
    /// which you can find by running `but oplog`.
    ///
    #[cfg(feature = "legacy")]
    Restore {
        /// Oplog SHA to restore to
        oplog_sha: String,
        /// Skip confirmation prompt
        #[clap(short = 'f', long = "force")]
        force: bool,
    },

    /// Undo the last operation by reverting to the previous snapshot.
    ///
    /// This is a shorthand for restoring to the last oplog entry before the
    /// current one. It allows you to quickly undo the most recent operation.
    ///
    #[cfg(feature = "legacy")]
    Undo,

    /// Amends changes into the appropriate commits where they belong.
    ///
    /// The semantic for finding "the appropriate commit" is as follows:
    ///
    /// - Changes are amended into the topmost commit of the leftmost (first) lane (branch)
    /// - If a change is staged to a particular lane (branch), it will be amended into a commit there
    /// - If there are no commits in this branch, a new commit is created
    /// - If a change has a dependency to a particular commit, it will be amended into that particular commit
    ///
    /// Optionally an identifier to an Uncommitted File or a Branch (stack) may be provided.
    ///
    /// - If an Uncommitted File id is provided, absorb will be performed for just that file
    /// - If a Branch (stack) id is provided, absorb will be performed for all changes staged to that stack
    /// - If no source is provided, absorb is performed for all uncommitted changes
    ///
    #[cfg(feature = "legacy")]
    Absorb {
        /// If the Source is an uncommitted change - the change will be absorbed.
        /// If the Source is a stack - anything staged to the stack will be absorbed accordingly.
        /// If not provided, everything that is uncommitted will be absorbed.
        source: Option<String>,
    },

    /// Discard uncommitted changes from the worktree.
    ///
    /// This command permanently discards changes to files, restoring them to their
    /// state in the HEAD commit. Use this to undo unwanted modifications.
    ///
    /// The ID parameter should be a file ID as shown in `but status`. You can
    /// discard a whole file or specific hunks within a file.
    ///
    /// ## Examples
    ///
    /// Discard all changes to a file:
    ///
    /// ```text
    /// but discard a1
    /// ```
    #[cfg(feature = "legacy")]
    Discard {
        /// The ID of the file or hunk to discard (as shown in `but status`)
        id: String,
    },

    /// Commands for interacting with forges like GitHub, GitLab (coming soon), etc.
    ///
    /// The `but forge` tools allow you to authenticate with a forge from the CLI,
    /// which then enables features like creating pull requests with the `but pr`
    /// commands.
    ///
    /// Start by running `but forge auth` to authenticate with your forge.
    ///
    /// You can also authenticate several different users on a forge and see them
    /// listed with `but forge list-users` or forget a user with `but forge forget`.
    ///
    /// Currently only GitHub is supported, but more forges will be added in the
    /// near future.
    ///
    Forge(forge::integration::Platform),

    /// Commands for creating and managing pull requests on a forge.
    ///
    /// If you are authenticated with a forge using `but forge auth`, you can use
    /// the `but pr` commands to create pull requests (or merge requests) on
    /// the remote repository for your branches.
    ///
    /// Running `but pr` without a subcommand defaults to `but pr new`, which
    /// will prompt you to select a branch to create a PR for.
    ///
    #[cfg(feature = "legacy")]
    #[clap(visible_alias = "review")]
    Pr(forge::pr::Platform),

    /// Trigger a refresh of remote data fetching from the remote, Pull Requests, and CI status.
    ///
    /// This is a hidden command primarily used for background sync operations.
    #[cfg(feature = "legacy")]
    #[clap(hide = true)]
    RefreshRemoteData {
        /// Whether to also refresh git fetch from the remote.
        #[clap(long, default_value_t = false)]
        fetch: bool,
        /// Whether to also refresh Pull Requests from the forge.
        #[clap(long, default_value_t = false)]
        pr: bool,
        /// Whether to also refresh CI status from the forge.
        #[clap(long, default_value_t = false)]
        ci: bool,
    },

    /// AI: Starts up the MCP server.
    ///
    /// This is the local MCP server that can be used by coding agents to invoke
    /// automatic GitButler commits after code generation or edits.
    ///
    /// By default, there is only one tool exposed, which is to simply commit changes
    /// and generate a commit message based on the provided prompt.
    ///
    /// If you invoke with `--internal`, it starts the internal MCP server with
    /// more granular tools, allowing you to ask your agent to do more specific
    /// tasks.
    ///
    #[cfg(feature = "legacy")]
    #[clap(hide = true)]
    Mcp {
        /// Starts the internal MCP server which has more granular tools.
        #[clap(long, short = 'i', hide = true)]
        internal: bool,
    },

    /// AI: Claude hooks
    ///
    /// Provides lifecycle hooks handlers for the Claude Code hooks feature.
    ///
    /// See: <https://docs.gitbutler.com/features/ai-integration/claude-code-hooks>
    ///
    #[cfg(feature = "legacy")]
    #[clap(hide = true)]
    Claude(claude::Platform),

    /// AI: Cursor hooks
    ///
    /// Provides lifecycle hooks handlers for the Cursor hooks feature.
    ///
    /// See: <https://docs.gitbutler.com/features/ai-integration/cursor-hooks>
    ///
    #[cfg(feature = "legacy")]
    #[clap(hide = true)]
    Cursor(cursor::Platform),

    /// INTERNAL: GitButler Actions are automated tasks (like macros) that can be performed on a repository.
    #[cfg(feature = "legacy")]
    #[clap(hide = true)]
    Actions(actions::Platform),

    /// INTERNAL: If metrics are permitted, this subcommand handles posthog event creation.
    #[clap(hide = true)]
    Metrics {
        #[clap(long, value_enum)]
        command_name: metrics::CommandName,
        #[clap(long)]
        props: String,
    },

    /// UTILITY: Generate shell completion scripts for the specified or inferred shell.
    #[clap(hide = true)]
    Completions {
        /// The shell to generate completions for, or the one extracted from the `SHELL` environment variable.
        #[clap(value_enum)]
        shell: Option<clap_complete::Shell>,
    },

    /// Manage command aliases.
    ///
    /// Aliases allow you to create shortcuts for commonly used commands.
    /// They are stored in git config under the `but.alias.*` namespace.
    ///
    /// ## Examples
    ///
    /// List all configured aliases:
    ///
    /// ```text
    /// but alias
    /// ```
    ///
    /// Create a new alias:
    ///
    /// ```text
    /// but alias add st status
    /// but alias add stv "status --verbose"
    /// ```
    ///
    /// Remove an alias:
    ///
    /// ```text
    /// but alias remove st
    /// ```
    ///
    Alias(alias::Platform),

    /// Resolve conflicts in a commit.
    ///
    /// When a commit is in a conflicted state (marked with conflicts during rebase),
    /// use this command to enter resolution mode, resolve the conflicts, and finalize.
    ///
    /// ## Workflow
    ///
    /// 1. Enter resolution mode: `but resolve <commit-id>`
    /// 2. Resolve conflicts in your editor (remove conflict markers)
    /// 3. Check remaining conflicts: `but resolve status`
    /// 4. Finalize resolution: `but resolve finish`
    ///    Or cancel: `but resolve cancel`
    ///
    /// When in resolution mode, `but status` will also show that you're resolving conflicts.
    ///
    #[cfg(feature = "legacy")]
    Resolve {
        /// Subcommand to run (defaults to entering resolution mode)
        #[clap(subcommand)]
        cmd: Option<resolve::Subcommands>,
        /// Commit ID to enter resolution mode for (when no subcommand is provided)
        commit: Option<String>,
    },

    /// Hidden command that redirects to `but pull --check`
    #[cfg(feature = "legacy")]
    #[clap(hide = true)]
    Fetch,

    /// Squash two commits together.
    ///
    /// Wrapper for `but rub <commit1> <commit2>`.
    #[cfg(feature = "legacy")]
    Squash {
        /// First commit ID (will be squashed into the second)
        commit1: String,
        /// Second commit ID (target commit)
        commit2: String,
        /// Drop the first commit's message and keep only the second commit's message
        #[clap(long, short = 'd')]
        drop_message: bool,
    },

    /// Uncommit changes from a commit or file-in-commit to the unstaged area.
    ///
    /// Wrapper for `but rub <source> zz`.
    #[cfg(feature = "legacy")]
    Uncommit {
        /// Commit ID or file-in-commit ID to uncommit
        source: String,
    },

    /// Amend a file change into a specific commit and rebases any dependent commits.
    ///  
    /// Wrapper for `but rub <file> <commit>`.
    #[cfg(feature = "legacy")]
    Amend {
        /// File ID to amend
        file: String,
        /// Commit ID to amend into
        commit: String,
    },

    /// Stages a file or hunk to a specific branch.
    ///
    /// Wrapper for `but rub <file-or-hunk> <branch>`.
    #[cfg(feature = "legacy")]
    Stage {
        /// File or hunk ID to stage
        file_or_hunk: String,
        /// Branch ID to stage to
        branch: String,
    },

    /// Unstages a file or hunk from a branch.
    ///
    /// Wrapper for `but rub <file-or-hunk> zz`.
    #[cfg(feature = "legacy")]
    #[clap(hide = true)]
    Unstage {
        /// File or hunk ID to unstage
        file_or_hunk: String,
        /// Branch ID to unstage from (optional, for validation)
        #[clap(required = false)]
        branch: Option<String>,
    },

    /// Show help information grouped by category.
    ///
    /// Displays all available commands organized into functional categories
    /// such as Inspection, Branching and Committing, Server Interactions, etc.
    ///
    /// This is equivalent to running `but -h` to see the command overview.
    Help,
}

pub mod alias;

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

pub mod forge;
pub mod metrics;
#[cfg(feature = "legacy")]
pub mod oplog;
pub mod push;
#[cfg(feature = "legacy")]
pub mod resolve;

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
        Stop,
        #[clap(alias = "pp")]
        PermissionPromptMcp {
            /// The Claude session ID for this MCP server instance
            #[clap(long)]
            session_id: String,
        },
        /// Get the last user message (for testing purposes)
        #[clap(hide = true)]
        Last {
            /// Offset to skip N most recent messages (positive integer)
            #[clap(long, short = 'o', default_value = "0")]
            offset: usize,
        },
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

pub mod worktree {
    #[derive(Debug, clap::Parser)]
    pub struct Platform {
        #[clap(subcommand)]
        pub cmd: Subcommands,
    }

    #[derive(Debug, clap::Subcommand)]
    pub enum Subcommands {
        /// Create a new worktree from a reference
        New {
            /// The reference (branch, commit, etc.) to create the worktree from
            reference: String,
        },
        /// List all worktrees
        List,
        /// Integrate a worktree
        Integrate {
            /// The path or name of the worktree to integrate
            path: String,
            /// The target reference to integrate into (defaults to the reference the worktree was created from)
            #[clap(long)]
            target: Option<String>,
            /// Perform a dry run without making changes
            #[clap(long)]
            dry: bool,
        },
        /// Destroy worktree(s)
        Destroy {
            /// The path to the worktree to destroy, or a reference to destroy all worktrees created from it
            target: String,
            /// Treat the target as a reference instead of a path
            #[clap(long)]
            reference: bool,
        },
    }
}

pub mod branch;

#[cfg(test)]
mod tests;
