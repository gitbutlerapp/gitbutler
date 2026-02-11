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
    ///
    /// Repeat up to 4 times for increasingly verbose output.
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
    /// After a mutation command completes, also output workspace status.
    ///
    /// In human mode, prints status after the command output.
    /// In JSON mode, wraps both in {"result": ..., "status": ...} on success, or
    /// {"result": ..., "status_error": ...} if the status query fails (in which case "status" is absent).
    #[clap(long = "status-after", global = true)]
    pub status_after: bool,
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

#[derive(Debug, clap::Subcommand, strum::VariantNames, strum::AsRefStr)]
#[strum(serialize_all = "kebab-case")]
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
    #[clap(verbatim_doc_comment)]
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
        /// Disable hints about available commands at the end of output.
        #[clap(long = "no-hint", default_value_t = false)]
        no_hint: bool,
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
    /// ## Operations Matrix
    ///
    /// Each cell shows what happens when you rub SOURCE → TARGET:
    ///
    /// ```text
    /// SOURCE ↓ / TARGET →  │ zz (unassigned) │ Commit     │ Branch      │ Stack
    /// ─────────────────────┼─────────────────┼────────────┼─────────────┼────────────
    /// File/Hunk            │ Unstage         │ Amend      │ Stage       │ Stage
    /// Commit               │ Undo            │ Squash     │ Move        │ -
    /// Branch (all changes) │ Unstage all     │ Amend all  │ Reassign    │ Reassign
    /// Stack (all changes)  │ Unstage all     │ -          │ Reassign    │ Reassign
    /// Unassigned (zz)      │ -               │ Amend all  │ Stage all   │ Stage all
    /// File-in-Commit       │ Uncommit        │ Move       │ Uncommit to │ -
    /// ```
    ///
    /// Legend:
    /// - `zz` is a special target meaning "unassigned" (no branch)
    /// - `-` means the operation is not supported
    /// - "all changes" / "all" refers to all uncommitted changes from that source
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
    #[clap(verbatim_doc_comment)]
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
    #[clap(verbatim_doc_comment)]
    Diff {
        /// The CLI ID of the entity to show the diff for
        target: Option<String>,
        /// Open an interactive TUI diff viewer
        #[clap(long = "tui", conflicts_with = "no_tui")]
        tui: bool,
        /// Disable the interactive TUI diff viewer (overrides but.ui.tui config)
        #[clap(long = "no-tui", conflicts_with = "tui")]
        no_tui: bool,
    },

    /// Open a file in the built-in text editor.
    ///
    /// A simple terminal-based text editor for quick edits. This is the same
    /// editor used as the fallback when no external editor is configured.
    ///
    /// ## Examples
    ///
    /// Edit a file:
    ///
    /// ```text
    /// but edit README.md
    /// ```
    ///
    #[clap(verbatim_doc_comment, hide = true)]
    Edit {
        /// Path to the file to edit (created if it doesn't exist)
        file: String,
    },

    /// Shows detailed information about a commit or branch.
    ///
    /// When given a commit ID, displays the full commit message, author information,
    /// committer information (if different from author), and the list of files modified.
    ///
    /// When given a branch name, displays the branch name and a list of all commits
    /// on that branch. Use --verbose to show full commit messages and files changed.
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
    /// Show branch commits by branch name:
    ///
    /// ```text
    /// but show my-feature-branch
    /// ```
    ///
    /// Show branch with full commit details:
    ///
    /// ```text
    /// but show my-feature-branch --verbose
    /// ```
    ///
    #[cfg(feature = "legacy")]
    #[clap(verbatim_doc_comment)]
    Show {
        /// The commit ID (short or full SHA), branch name, or CLI ID to show details for
        commit: String,
        /// Show full commit messages and files changed for each commit
        #[clap(short = 'v', long = "verbose")]
        verbose: bool,
    },

    /// Sets up a GitButler project from a git repository in the current directory.
    ///
    /// This command will:
    /// - Add the repository to the global GitButler project registry
    /// - Switch to the gitbutler/workspace branch (if not already on it)
    /// - Set up a default target branch (the remote's HEAD)
    /// - Add a gb-local remote if no push remote exists
    ///
    /// If you have an existing Git repository and want to start using GitButler
    /// with it, you can run this command to set up the necessary configuration
    /// and data structures.
    ///
    /// ## Examples
    ///
    /// Initialize a new git repository and set up GitButler:
    ///
    /// ```text
    /// but setup --init
    /// ```
    ///
    #[cfg(feature = "legacy")]
    #[clap(verbatim_doc_comment)]
    Setup {
        /// Initialize a new git repository with an empty commit if one doesn't exist.
        ///
        /// This is useful when running in non-interactive environments (like CI/CD)
        /// where you want to ensure a git repository exists before setting up GitButler.
        #[clap(long)]
        #[clap(verbatim_doc_comment)]
        init: bool,
    },

    /// Exit GitButler mode and return to normal Git workflow.
    ///
    /// This command:
    /// - Creates an oplog snapshot of the current state
    /// - Finds the first active branch and checks it out
    /// - Cherry-picks any dangling commits from gitbutler/workspace
    /// - Provides instructions on how to return to GitButler mode
    ///
    /// This is useful when you want to temporarily or permanently leave GitButler
    /// management and work with standard Git commands.
    ///
    /// ## Examples
    ///
    /// Exit GitButler mode:
    ///
    /// ```text
    /// but teardown
    /// ```
    ///
    #[cfg(feature = "legacy")]
    #[clap(verbatim_doc_comment)]
    Teardown,

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
    #[clap(verbatim_doc_comment)]
    Pull {
        /// Only check the status without updating (equivalent to the old `but base check`)
        #[clap(long, short = 'c')]
        check: bool,
    },

    /// Commands for managing branches.
    ///
    /// This includes creating, deleting, listing, and showing details about branches.
    ///
    /// By default without a subcommand, it will list the branches.
    ///
    /// To apply or unapply branches, use `but apply` and `but unapply`.
    ///
    #[clap(verbatim_doc_comment)]
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
    #[clap(verbatim_doc_comment)]
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
    #[clap(verbatim_doc_comment)]
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
    #[clap(verbatim_doc_comment)]
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
    #[clap(verbatim_doc_comment)]
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
    /// Use `but commit empty --before <target>` or `but commit empty --after <target>`
    /// to insert a blank commit. This is useful for creating a placeholder
    /// commit that you can amend changes into later using `but mark`, `but rub` or `but absorb`.
    ///
    #[cfg(feature = "legacy")]
    #[clap(verbatim_doc_comment)]
    Commit(commit::Platform),

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
    #[clap(verbatim_doc_comment)]
    Push(push::Command),

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
    #[clap(verbatim_doc_comment)]
    Reword {
        /// Commit ID to edit the message for, or branch ID to rename
        target: String,
        /// The new commit message or branch name. If not provided, opens an editor.
        #[clap(short = 'm', long = "message", conflicts_with = "format")]
        message: Option<String>,
        /// Format the existing commit message to 72-char line wrapping without opening an editor
        #[clap(short = 'f', long = "format", conflicts_with = "message")]
        format: bool,
    },

    /// Commands for viewing and managing operation history.
    ///
    /// Displays a list of past operations performed in the repository,
    /// including their timestamps and descriptions.
    ///
    /// This allows you to restore to any previous point in the history of the
    /// project. All state is preserved in operations, including uncommitted changes.
    ///
    /// You can use `but oplog restore <oplog-sha>` to restore to a specific state.
    ///
    /// By default, shows the last 20 oplog entries (same as `but oplog list`).
    ///
    #[cfg(feature = "legacy")]
    #[clap(verbatim_doc_comment)]
    Oplog(oplog::Platform),

    /// Undo the last operation by reverting to the previous snapshot.
    ///
    /// This is a shorthand for restoring to the last oplog entry before the
    /// current one. It allows you to quickly undo the most recent operation.
    ///
    #[cfg(feature = "legacy")]
    #[clap(verbatim_doc_comment)]
    Undo,

    /// Amends changes into the appropriate commits where they belong.
    ///
    /// The semantic for finding "the appropriate commit" is as follows:
    ///
    /// - If a change has a dependency to a particular commit, it will be amended into that particular commit
    /// - If a change is staged to a particular lane (branch), it will be amended into a commit there
    /// - If there are no commits in this branch, a new commit is created
    /// - Changes are amended into the topmost commit of the leftmost (first) lane (branch)
    ///
    /// Optionally an identifier to an Uncommitted File or a Branch (stack) may be provided.
    ///
    /// - If an Uncommitted File id is provided, absorb will be performed for just that file
    /// - If a Branch (stack) id is provided, absorb will be performed for all changes staged to that stack
    /// - If no source is provided, absorb is performed for all uncommitted changes
    ///
    /// If `--dry-run` is specified, no changes will be made; instead, the absorption plan
    /// (what changes would be absorbed by which commits) will be shown.
    ///
    /// If `--new` is specified, new commits will be created for absorbed changes
    /// instead of amending existing commits.
    #[cfg(feature = "legacy")]
    #[clap(verbatim_doc_comment)]
    Absorb {
        /// If the Source is an uncommitted change - the change will be absorbed.
        /// If the Source is a stack - anything staged to the stack will be absorbed accordingly.
        /// If not provided, everything that is uncommitted will be absorbed.
        source: Option<String>,
        /// Show the absorption plan without making any changes.
        #[clap(long = "dry-run")]
        dry_run: bool,
        /// Create new commits, instead of amending existing ones.
        /// This is useful when you want to preserve existing commits and add new ones for the absorbed changes.
        #[clap(long, short = 'n')]
        new: bool,
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
    #[clap(verbatim_doc_comment)]
    Discard {
        /// The ID of the file or hunk to discard (as shown in `but status`)
        id: String,
    },

    /// Commands for creating and managing reviews on a forge, e.g. GitHub PRs or GitLab MRs.
    ///
    /// If you are authenticated with a forge using `but config forge auth`, you can use
    /// the `but pr` or `but mr` commands to create pull requests (or merge requests) on
    /// the remote repository for your branches.
    ///
    /// Running `but pr` without a subcommand defaults to `but pr new`, which
    /// will prompt you to select a branch to create a PR for.
    ///
    #[cfg(feature = "legacy")]
    #[clap(visible_alias = "review")]
    #[clap(visible_alias = "mr")]
    Pr(forge::pr::Platform),

    /// Trigger a refresh of remote data fetching from the remote, Pull Requests, and CI status.
    ///
    /// This is a hidden command primarily used for background sync operations.
    #[cfg(feature = "legacy")]
    #[clap(hide = true)]
    #[clap(verbatim_doc_comment)]
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
        /// Whether to also check for application updates.
        #[clap(long, default_value_t = false)]
        updates: bool,
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
    #[clap(verbatim_doc_comment)]
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
    #[clap(verbatim_doc_comment)]
    Claude(claude::Platform),

    /// AI: Cursor hooks
    ///
    /// Provides lifecycle hooks handlers for the Cursor hooks feature.
    ///
    /// See: <https://docs.gitbutler.com/features/ai-integration/cursor-hooks>
    ///
    #[cfg(feature = "legacy")]
    #[clap(hide = true)]
    #[clap(verbatim_doc_comment)]
    Cursor(cursor::Platform),

    /// INTERNAL: GitButler Actions are automated tasks (like macros) that can be performed on a repository.
    #[cfg(feature = "legacy")]
    #[clap(hide = true)]
    #[clap(verbatim_doc_comment)]
    Actions(actions::Platform),

    /// INTERNAL: If metrics are permitted, this subcommand handles posthog event creation.
    #[clap(hide = true)]
    #[clap(verbatim_doc_comment)]
    Metrics {
        #[clap(long, value_enum)]
        command_name: metrics::CommandName,
        #[clap(long)]
        props: String,
    },

    /// UTILITY: Generate shell completion scripts for the specified or inferred shell.
    #[clap(hide = true)]
    #[clap(verbatim_doc_comment)]
    Completions {
        /// The shell to generate completions for, or the one extracted from the `SHELL` environment variable.
        #[clap(value_enum)]
        shell: Option<clap_complete::Shell>,
    },

    /// Manage GitButler CLI and app updates.
    ///
    /// Check for new versions, install updates, or suppress update notifications.
    #[clap(verbatim_doc_comment)]
    Update(update::Platform),

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
    #[clap(verbatim_doc_comment)]
    Alias(alias::Platform),

    /// View and manage GitButler configuration.
    ///
    /// Without a subcommand, displays an overview of important settings including
    /// user information, target branch, forge configuration, and AI setup.
    ///
    /// ## Examples
    ///
    /// View configuration overview:
    ///
    /// ```text
    /// but config
    /// ```
    ///
    /// View/set user configuration:
    ///
    /// ```text
    /// but config user
    /// but config user set name "John Doe"
    /// but config user set email john@example.com
    /// ```
    ///
    /// View/set forge configuration:
    ///
    /// ```text
    /// but config forge
    /// ```
    ///
    /// View/set target branch:
    ///
    /// ```text
    /// but config target
    /// ```
    ///
    /// View/set metrics:
    ///
    /// ```text
    /// but config metrics
    /// ```
    ///
    #[clap(verbatim_doc_comment)]
    Config(config::Platform),

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
    #[clap(verbatim_doc_comment)]
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
    #[clap(verbatim_doc_comment)]
    Fetch,

    /// Squash commits together.
    ///
    /// Can be invoked in three ways:
    /// 1. Using commit identifiers: `but squash <commit1> <commit2>` or `but squash <commit1> <commit2> <commit3>...`
    ///    - Squashes all commits except the last into the last commit
    /// 2. Using a commit range: `but squash <commit1>..<commit4>`
    ///    - Squashes all commits in the range into the last commit in the range
    /// 3. Using a branch name: `but squash <branch>`
    ///    - Squashes all commits in the branch into the bottom-most commit
    #[cfg(feature = "legacy")]
    #[clap(verbatim_doc_comment)]
    Squash {
        /// Commit identifiers, a range (commit1..commit2), or a branch name
        commits: Vec<String>,
        /// Drop source commit messages and keep only the target commit's message
        #[clap(long, short = 'd', group = "message_opts")]
        drop_message: bool,
        /// Provide a new commit message for the resulting commit
        #[clap(long, short = 'm', group = "message_opts")]
        message: Option<String>,
        /// Generate commit message using AI with optional user summary or instructions.
        /// Use --ai by itself or --ai="your instructions" (equals sign required for value)
        #[clap(long, short = 'i', group = "message_opts", num_args = 0..=1, require_equals = true)]
        ai: Option<Option<String>>,
    },

    /// Uncommit changes from a commit or file-in-commit to the unstaged area.
    ///
    /// Wrapper for `but rub <source> zz`.
    #[cfg(feature = "legacy")]
    #[clap(verbatim_doc_comment)]
    Uncommit {
        /// Commit ID or file-in-commit ID to uncommit
        source: String,
    },

    /// Amend a file change into a specific commit and rebases any dependent commits.
    ///
    /// Wrapper for `but rub <file> <commit>`.
    #[cfg(feature = "legacy")]
    #[clap(verbatim_doc_comment)]
    Amend {
        /// File ID to amend
        file: String,
        /// Commit ID to amend into
        commit: String,
    },

    /// Merge a branch into your local target branch.
    ///
    /// If the target branch is local (`gb-local`), finds the local branch that the target
    /// references (e.g., `gb-local/master` becomes `master`) and merges the specified
    /// branch into that local branch. After merging, runs the equivalent of `but pull`
    /// to update all branches.
    ///
    /// ## Examples
    ///
    /// Merge a branch by its CLI ID:
    ///
    /// ```text
    /// but merge bu
    /// ```
    ///
    /// Merge a branch by name:
    ///
    /// ```text
    /// but merge my-feature-branch
    /// ```
    #[cfg(feature = "legacy")]
    #[clap(verbatim_doc_comment)]
    Merge {
        /// Branch ID or name to merge
        branch: String,
    },

    /// Move a commit to a different location in the stack.
    ///
    /// By default, commits are moved to be before (below) the target.
    /// Use `--after` to move the commit after (above) the target instead.
    ///
    /// When moving to a branch, the commit is placed at the top of that branch's stack.
    ///
    /// ## Examples
    ///
    /// Move a commit before another commit:
    ///
    /// ```text
    /// but move abc123 def456
    /// ```
    ///
    /// Move a commit after another commit:
    ///
    /// ```text
    /// but move abc123 def456 --after
    /// ```
    ///
    /// Move a commit to a different branch (places at top):
    ///
    /// ```text
    /// but move abc123 my-feature-branch
    /// ```
    #[cfg(feature = "legacy")]
    #[clap(verbatim_doc_comment)]
    Move {
        /// Commit ID to move
        source_commit: String,
        /// Target commit ID or branch name
        target: String,
        /// Move the commit after (above) the target instead of before (below)
        #[clap(short = 'a', long = "after")]
        after: bool,
    },

    /// Cherry-pick a commit from an unapplied branch into an applied virtual branch.
    ///
    /// This command allows you to pick individual commits from unapplied branches
    /// and apply them to your current workspace branches.
    ///
    /// The source can be:
    /// - A commit SHA (full or short)
    /// - A CLI ID (e.g., "c5" from `but status`)
    /// - An unapplied branch name (shows interactive commit selection)
    ///
    /// If no target branch is specified:
    /// - In interactive mode: prompts you to select a target branch
    /// - If only one branch exists: automatically uses that branch
    /// - In non-interactive mode: fails with an error
    ///
    /// ## Examples
    ///
    /// Pick a specific commit into a branch:
    ///
    /// ```text
    /// but pick abc1234 my-feature
    /// ```
    ///
    /// Pick using a CLI ID:
    ///
    /// ```text
    /// but pick c5 my-feature
    /// ```
    ///
    /// Interactively select commits from an unapplied branch:
    ///
    /// ```text
    /// but pick feature-branch
    /// ```
    ///
    #[cfg(feature = "legacy")]
    #[clap(verbatim_doc_comment)]
    Pick {
        /// The commit SHA, CLI ID, or unapplied branch name to cherry-pick from
        source: String,
        /// The target virtual branch to apply the commit(s) to
        #[clap(value_name = "TARGET_BRANCH")]
        target_branch: Option<String>,
    },

    /// Unapply a branch from the workspace.
    ///
    /// If you want to unapply an applied branch from your workspace
    /// (effectively stashing it) so you can work on other branches,
    /// you can run `but unapply <branch-name>`.
    ///
    /// This will remove the changes in that branch from your working
    /// directory and you can re-apply it later when needed. You will then
    /// see the branch as unapplied in `but branch list`.
    ///
    /// The identifier can be:
    /// - A CLI ID pointing to a stack or branch (e.g., "bu" from `but status`)
    /// - A branch name
    ///
    /// If a branch name (or an identifier pointing to a branch) is provided,
    /// the entire stack containing that branch will be unapplied.
    ///
    /// ## Examples
    ///
    /// Unapply by branch name:
    ///
    /// ```text
    /// but unapply my-feature-branch
    /// ```
    ///
    /// Unapply by CLI ID:
    ///
    /// ```text
    /// but unapply bu
    /// ```
    ///
    #[cfg(feature = "legacy")]
    #[clap(verbatim_doc_comment)]
    Unapply {
        /// CLI ID or name of the branch/stack to unapply
        identifier: String,
        /// Force unapply without confirmation
        #[clap(long, short = 'f')]
        force: bool,
    },

    /// Apply a branch to the workspace.
    ///
    /// If you want to apply an unapplied branch to your workspace so you
    /// can work on it, you can run `but apply <branch-name>`.
    ///
    /// This will apply the changes in that branch into your working directory
    /// as a parallel applied branch.
    ///
    /// ## Examples
    ///
    /// Apply by branch name:
    ///
    /// ```text
    /// but apply my-feature-branch
    /// ```
    ///
    #[cfg(feature = "legacy")]
    #[clap(verbatim_doc_comment)]
    Apply {
        /// Name of the branch to apply
        branch_name: String,
    },

    /// Stages a file or hunk to a specific branch.
    ///
    /// Without arguments, opens an interactive TUI for selecting files and hunks to stage.
    /// With arguments, stages the specified file or hunk to the given branch.
    ///
    /// Usage:
    ///   `but stage`                             (interactive TUI selector)
    ///   `but stage --branch <branch>`           (interactive, specific branch)
    ///   `but stage <file-or-hunk> <branch>`     (direct staging)
    #[cfg(feature = "legacy")]
    #[clap(verbatim_doc_comment)]
    Stage {
        /// File or hunk ID to stage
        file_or_hunk: Option<String>,
        /// Branch to stage to (positional)
        branch_pos: Option<String>,
        /// Branch to stage to (for interactive mode)
        #[clap(long = "branch", short = 'b')]
        branch: Option<String>,
    },

    /// Unstages a file or hunk from a branch.
    ///
    /// Wrapper for `but rub <file-or-hunk> zz`.
    #[cfg(feature = "legacy")]
    #[clap(verbatim_doc_comment)]
    #[clap(hide = true)]
    Unstage {
        /// File or hunk ID to unstage
        file_or_hunk: String,
        /// Branch ID to unstage from (optional, for validation)
        #[clap(required = false)]
        branch: Option<String>,
    },

    /// Manage Claude AI skills for GitButler.
    ///
    /// Skills provide enhanced AI capabilities for working with GitButler through
    /// Claude Code and other AI assistants.
    ///
    /// Use `but skill install` to install the GitButler skill files. By default,
    /// it prompts for scope (repository or global home directory) and then format.
    /// When run outside a git repository, local scope is unavailable and the
    /// default install location is global (home directory). You can still
    /// install to a custom location with `--path` using an absolute or `~` path.
    ///
    /// ## Examples
    ///
    /// Install interactively (prompts for scope and format):
    ///
    /// ```text
    /// but skill install
    /// ```
    ///
    /// Install the skill globally:
    ///
    /// ```text
    /// but skill install --global
    /// ```
    #[clap(verbatim_doc_comment)]
    Skill(skill::Platform),

    /// Show help information grouped by category.
    ///
    /// Displays all available commands organized into functional categories
    /// such as Inspection, Branching and Committing, Server Interactions, etc.
    ///
    /// This is equivalent to running `but -h` to see the command overview.
    #[clap(hide = true)]
    #[clap(verbatim_doc_comment)]
    Help,

    /// INTERNAL: First-run onboarding that shows metrics info and marks onboarding complete.
    ///
    /// This command is silent if onboarding has already been completed.
    /// It is designed to be called by the installer after installation.
    #[clap(hide = true)]
    Onboarding,

    /// AI: Claude Code hook for workspace awareness and skill activation.
    ///
    /// Outputs workspace status as JSON and a skill-loading nudge.
    /// Intended to fire on the Stop hook.
    #[clap(hide = true)]
    #[clap(verbatim_doc_comment)]
    EvalHook,
}

pub mod alias;
pub mod commit;
pub mod config;
pub mod skill;
pub mod update;

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
