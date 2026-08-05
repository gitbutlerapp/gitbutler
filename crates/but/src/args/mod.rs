//! Command-line argument parsing for GitButler CLI.
//!
//! Uses `clap` for defining commands and options.
//!
//! This module defines the main `Args` struct which represents the top-level
//! command-line interface, along with its subcommands and options.
//!
//! Nearly all documentation for the CLI is defined here using `clap` attributes,
//! which are then used to generate help messages and online documentation.

use std::ffi::OsString;
use std::path::PathBuf;

use crate::args::atoms::CliIdArg;
use crate::utils::envs;

pub mod atoms;

#[derive(Debug, clap::Parser)]
#[clap(
    name = "but",
    about = "A GitButler CLI tool",
    version = option_env!("VERSION").unwrap_or("dev"),
    disable_help_subcommand = true,
)]
pub struct Args {
    /// Enable tracing for debug and performance information printed to stderr.
    ///
    /// Repeat up to 4 times for increasingly verbose output.
    #[clap(short = 't', long, action = clap::ArgAction::Count, hide = true, env = "BUT_TRACE")]
    pub trace: u8,
    /// Log to this file instead of stderr.
    ///
    /// If the file does not exist it will be created. If it does exist it will be truncated.
    #[clap(long, hide = true)]
    pub log_file: Option<PathBuf>,
    /// Run as if but was started in PATH instead of the current working directory.
    #[clap(short = 'C', long, default_value = ".", value_name = "PATH")]
    pub current_dir: PathBuf,
    #[clap(flatten)]
    pub format: OutputFormatArg,
    /// Append workspace status when supported by a mutation command.
    ///
    /// Use when the next step needs IDs or details from the resulting workspace.
    #[clap(long, global = true)]
    pub status_after: bool,
    /// Subcommand to run (`but <COMMAND>`).
    ///
    /// On UNIX, if `<COMMAND>` is not built in and `but-<COMMAND>` exists on the PATH, that program
    /// is executed with the remaining arguments (`but <COMMAND> [ARGS]`).
    #[clap(subcommand)]
    pub cmd: Option<Subcommands>,
}

#[derive(Debug, clap::Args)]
pub struct OutputFormatArg {
    /// Output detailed information as JSON for tool consumption.
    #[clap(long, global = true)]
    pub json: bool,
}

/// How we should format anything written to [`std::io::stdout()`].
#[derive(Debug, Copy, Clone)]
pub enum OutputFormat {
    /// The output to write is supposed to be for human consumption, and can be more verbose.
    Human { agent: bool },
    /// Output detailed information as JSON for tool consumption.
    Json,
}

impl OutputFormat {
    /// Reads the output format from `BUT_OUTPUT_FORMAT`.
    pub fn try_from_env(agent_detected: bool) -> Option<Self> {
        std::env::var(envs::BUT_OUTPUT_FORMAT)
            .ok()
            .and_then(|env| match &*env {
                "json" => Some(OutputFormat::Json),
                "human" => Some(OutputFormat::Human {
                    agent: agent_detected,
                }),
                _ => None,
            })
    }

    /// Whether this format renders human-readable text.
    pub fn is_human_text(self) -> bool {
        matches!(self, Self::Human { .. })
    }

    pub fn is_agent(self) -> bool {
        matches!(self, Self::Human { agent: true })
    }

    /// Whether this format renders plain text.
    pub fn is_text(self) -> bool {
        self.is_human_text()
    }

    /// Whether this format may truncate long text when output is not paged.
    pub fn allows_truncation(self) -> bool {
        matches!(self, Self::Human { agent: false })
    }

    /// Whether this format may use human terminal UI affordances like pagers, progress, prompts, or ambient messages.
    pub fn allows_human_ui(self) -> bool {
        matches!(self, Self::Human { agent: false })
    }

    /// Whether this format writes JSON values.
    pub fn is_json(self) -> bool {
        matches!(self, Self::Json)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::Subcommand)]
pub enum HelpTopic {
    /// Smart IDs to reference commits, branches and more in `but`.
    ///
    /// CLI IDs are used by `but` to reference things such as commits, branches and files. While
    /// obvious identifiers like full commit hashes and entire branch names are viable CLI IDs,
    /// there are also various shorter identifiers that can be used in place of the full names.
    ///
    /// In general, `but status` will show all currently available CLI IDs in front of the "thing".
    ///
    /// Typical CLI IDs include:
    ///
    /// * **Commit:**
    ///     - The entire commit ID
    ///     - The entire change ID
    ///     - Any prefix of the commit ID or change ID that is unique in the current context. `but status`
    ///       highlights the shortest possible prefix.
    /// * **Branch:**
    ///     - The entire branch name
    ///     - An exact short ID for the branch name, as shown by `but status`
    /// * **Uncommitted file:** A path-derived ID that is typically 1-3 characters
    /// * **Uncommitted hunk:** `<uncommitted_file_cli_id>:<hunk_cli_id>`
    ///     - Run `but diff` to show all current uncommitted hunks and their IDs
    /// * **Uncommitted area:** Always `zz`
    /// * **Committed file:** `<commit_cli_id>:<file_cli_id>`
    ///     - Run `but status -f` to show committed files
    ///
    /// Many CLI IDs depend on the context and may change if the context changes, such as when new
    /// data is written to files, commits are made or rearranged and branches are created or
    /// deleted.
    ///
    /// Some CLI IDs are more stable than others. For example, a commit's change ID is stable even
    /// when commits are made and moved around, but the minimum prefix may increase as other IDs are
    /// introduced.
    ///
    /// Most but not all commands accept CLI IDs to perform various actions. See the documentation
    /// for the individual command (`but <command> --help`) for details on how to use CLI IDs for
    /// that command in particular.
    #[clap(name = "cli-ids")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    CliIds,
}

impl HelpTopic {
    pub fn name(self) -> &'static str {
        match self {
            HelpTopic::CliIds => "cli-ids",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            HelpTopic::CliIds => "CLI IDs",
        }
    }
}

#[derive(
    Debug, clap::Subcommand, strum::VariantNames, strum::AsRefStr, strum::EnumDiscriminants,
)]
#[strum(serialize_all = "kebab-case")]
#[strum_discriminants(
    derive(strum::EnumIter, strum::AsRefStr, Hash),
    name(SubcommandDiscriminant)
)]
pub enum Subcommands {
    /// Overview of the project workspace state.
    ///
    /// This shows uncommitted files, all applied branches (stacked or
    /// parallel), commits on each of those branches,
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
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
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
        // Hidden no-op compatibility flag because some agents have a habit of running
        // `but status --short`.
        #[clap(long = "short", default_value_t = false, hide = true)]
        short: bool,
    },

    /// Displays the diff of changes in the repo.
    ///
    /// Without any arguments, it shows the diff of all uncommitted changes.
    /// Optionally, provide one CLI ID to show the diff specific to:
    /// - an uncommitted file
    /// - a branch
    /// - an entire stack
    /// - a commit
    /// - a file change within a commit
    ///
    /// `TARGET` accepts at most one entity. To show several entities, run this command once per
    /// entity.
    #[cfg(feature = "legacy")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Diff {
        /// The CLI ID of one entity to show the diff for
        target: Option<String>,
        /// Open an interactive TUI diff viewer
        #[clap(long = "tui", conflicts_with = "no_tui")]
        tui: bool,
        /// Disable the interactive TUI diff viewer (overrides but.ui.tui config)
        #[clap(long = "no-tui", conflicts_with = "tui")]
        no_tui: bool,
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
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Show {
        /// The commit ID (short or full SHA), branch name, or CLI ID to show details for
        commit: String,
        /// Show full commit messages and files changed for each commit
        #[clap(short = 'v', long = "verbose")]
        verbose: bool,
    },

    #[cfg(feature = "legacy")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Commit(commit::Platform),

    #[cfg(feature = "legacy")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Squash(squash::Platform),

    #[cfg(feature = "legacy")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Move(r#move::Platform),

    #[cfg(feature = "legacy")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    #[clap(hide = true, name = "_diff2")]
    _Diff2(diff2::Platform),

    #[cfg(feature = "legacy")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    #[clap(hide = true, name = "_reword2")]
    _Reword2(reword2::Platform),

    /// Debug command for expanding a CLI ID into any matching resources.
    ///
    /// This is not considered a feature of the CLI and should not be relied upon. It is only for
    /// testing and debugging.
    #[clap(hide = true, name = "_expand")]
    _Expand {
        /// CLI ID to parse.
        cli_id: CliIdArg,
    },

    /// Commands for managing branches.
    ///
    /// This includes creating, deleting, listing, and showing details about branches.
    ///
    /// By default without a subcommand, it will list the branches.
    ///
    /// To apply or unapply branches, use `but apply` and `but unapply`.
    ///
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Branch(branch::Platform),

    /// Land a branch directly onto the target branch.
    ///
    /// Lands the branch onto the configured target (for example `origin/master`) without going
    /// through a pull request — the "just push to the target" workflow. By default the target is
    /// fast-forwarded to the branch tip when possible (no merge commit); otherwise a merge commit
    /// is created. For a local (`gb-local`) target the refs are moved locally; otherwise the result
    /// is pushed to the remote. After landing, the remaining applied branches are reconciled onto
    /// the moved target, just like `but pull`.
    ///
    /// Requires an active GitButler workspace. Updating the target is direct and not easily
    /// reversible, so a confirmation is required (use `--yes` to skip it in scripts).
    ///
    /// When NOT to use this: if your project lands changes through pull requests / code review,
    /// use `but push` and open a PR (`but pr new`) instead — `but land` deliberately bypasses that
    /// process. On a real remote, a branch protected against direct pushes will reject the land.
    ///
    /// Landing a segment with other segments below it is refused unless `--whole-stack` is
    /// passed with the stack's top segment, which lands the entire stack.
    ///
    /// ## Examples
    ///
    /// Land a branch by its CLI ID:
    ///
    /// ```text
    /// but land bu
    /// ```
    ///
    /// Land a branch by name, forcing a merge commit:
    ///
    /// ```text
    /// but land my-feature-branch --no-ff
    /// ```
    ///
    /// Land an entire stack by naming its top segment:
    ///
    /// ```text
    /// but land top-branch --whole-stack
    /// ```
    #[cfg(feature = "legacy")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Land {
        /// Branch ID or name to land onto the target branch.
        branch: String,
        /// Skip the confirmation prompt.
        #[clap(long)]
        yes: bool,
        /// Always create a merge commit, even when the branch can be fast-forwarded.
        #[clap(long)]
        no_ff: bool,
        /// Land the entire stack: BRANCH must be the top segment, and the segments below it are
        /// published to the target along with it.
        #[clap(long)]
        whole_stack: bool,
    },

    #[cfg(feature = "legacy")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Discard(discard::Platform),

    // Hidden and underscore-prefixed to stay out of human-facing help and shell completions:
    // comments are an experiment, read and archived by agents that are pointed at the command
    // explicitly.
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    #[clap(hide = true, name = "_comment")]
    _Comment(comment::Platform),

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
    /// Alternatively, resolve with AI in one step: `but resolve <commit-id> --ai`,
    /// or `but resolve --ai` to resolve all conflicted commits, oldest first.
    ///
    /// When in resolution mode, `but status` will also show that you're resolving conflicts.
    ///
    #[cfg(feature = "legacy")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Resolve {
        /// Subcommand to run (defaults to entering resolution mode)
        #[clap(subcommand)]
        cmd: Option<resolve::Subcommands>,
        /// Commit ID to enter resolution mode for (when no subcommand is provided)
        commit: Option<String>,
        /// Resolve the conflicts with the configured AI model and apply the result.
        ///
        /// With a commit ID this resolves only that commit; without one it
        /// resolves all conflicted commits in the workspace, oldest first.
        /// Undo the result with `but undo`.
        #[clap(long)]
        ai: bool,
    },

    #[cfg(feature = "legacy")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Unapply(unapply::Platform),

    #[cfg(feature = "legacy")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Apply(apply::Platform),

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
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Push(push::Command),

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
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Pull {
        /// Only check the status without updating (equivalent to the old `but base check`)
        #[clap(long, short = 'c')]
        check: bool,
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

    /// Amends changes into the appropriate commits where they belong.
    ///
    /// The semantic for finding "the appropriate commit" is as follows:
    ///
    /// - If a change has a dependency to a particular commit, it will be amended into that particular commit
    /// - If a change is assigned to a branch in the GitButler app, it will be amended into a commit there
    /// - If there are no commits on that branch, a new commit is created there
    /// - Changes are amended into the topmost commit of the leftmost (first) branch
    ///
    /// Optionally an identifier to an Uncommitted File may be provided.
    ///
    /// - If an Uncommitted File id is provided, absorb will be performed for just that file
    /// - If no source is provided, absorb is performed for all uncommitted changes
    ///
    /// If `--dry-run` is specified, no changes will be made; instead, the absorption plan
    /// (what changes would be absorbed by which commits) will be shown.
    ///
    #[cfg(feature = "legacy")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Absorb {
        /// If the Source is an uncommitted change - the change will be absorbed.
        /// If not provided, everything that is uncommitted will be absorbed.
        source: Option<String>,
        /// Show the absorption plan without making any changes.
        #[clap(long = "dry-run")]
        dry_run: bool,
        #[clap(flatten)]
        allow_merged: atoms::AllowMergedArg,
    },

    /// Edit the commit message of the specified commit.
    ///
    /// You can easily change the commit message of any of your commits by
    /// running `but reword <commit-id>` and providing a new message in the
    /// editor.
    ///
    /// This will recreate the commit with the new message and then rebase any
    /// dependent commits on top of it.
    ///
    /// You can also use `but reword <branch-id>` to rename the branch.
    ///
    #[cfg(feature = "legacy")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Reword {
        /// Commit ID to edit the message for, or branch ID to rename
        target: CliIdArg,
        /// The new commit message or branch name. If not provided, opens an editor.
        #[clap(short = 'm', long = "message", conflicts_with = "fix_formatting")]
        message: Option<String>,
        /// Format the existing commit message to 72-char line wrapping without opening an editor
        #[clap(
            id = "fix_formatting",
            short = 'f',
            long = "fix-formatting",
            conflicts_with = "message"
        )]
        format: bool,
        /// Always show diff inside the editor.
        ///
        /// By default the diff will be shown unless it's large. The diff will always be shown if
        /// `--diff` is passed, regardless of the size of the diff.
        #[clap(long = "diff", default_value_t, conflicts_with_all = &["no_diff", "fix_formatting"])]
        diff: bool,
        /// Never show the diff inside the editor.
        #[clap(long = "no-diff", default_value_t, conflicts_with_all = &["diff", "fix_formatting"])]
        no_diff: bool,
        #[clap(flatten)]
        allow_merged: atoms::AllowMergedArg,
    },

    #[cfg(feature = "legacy")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Uncommit(uncommit::Platform),

    #[cfg(feature = "legacy")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Amend(amend::Platform),

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
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Oplog(oplog::Platform),

    #[cfg(feature = "legacy")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Undo(undo::Platform),

    #[cfg(feature = "legacy")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Redo(redo::Platform),

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
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Setup {
        /// Initialize a new git repository with an empty commit if one doesn't exist.
        ///
        /// This is useful when running in non-interactive environments (like CI/CD)
        /// where you want to ensure a git repository exists before setting up GitButler.
        #[clap(long)]
        #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
        init: bool,
    },

    /// Exit GitButler mode and return to normal Git workflow.
    ///
    /// This command:
    /// - Creates an oplog snapshot of the current state
    /// - Finds the first active branch and checks it out
    ///     - Alternatively, use `--checkout-to <branch>` to override this default
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
    /// ```text
    /// but teardown --checkout-to my-feature-branch
    /// ```
    ///
    #[cfg(feature = "legacy")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Teardown {
        /// Explicit override for which local branch to checkout to.
        #[clap(long, short = 'c', value_name = "LOCAL_BRANCH")]
        checkout_to: Option<String>,
    },

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
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Gui {
        /// Open the project in a new application window.
        #[clap(long, short = 'n', default_value_t = false)]
        new_window: bool,
        /// Path to the directory to open as a GitButler project. Defaults to the current directory.
        path: Option<PathBuf>,
    },

    /// Open files in the workspace using any defined program.
    #[clap(hide = true, name = "_open")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    _Open {
        /// One or more uncommitted files or hunks to open.
        #[clap(num_args = 1..)]
        sources: Vec<CliIdArg>,
        /// The program to use for opening.
        #[clap(long, short = 'p')]
        program_id: Option<String>,
    },

    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    #[cfg(feature = "legacy")]
    Tui(tui::Platform),

    /// Remove empty branches from the workspace.
    ///
    /// A branch is considered empty if it has no local commits and (by default)
    /// no upstream-only commits. Stacks with uncommitted changes assigned to them
    /// in the GitButler app are skipped entirely.
    ///
    /// The entire operation is recorded as a single oplog entry, so it can
    /// be undone with `but undo`.
    ///
    /// ## Examples
    ///
    /// Remove all empty branches:
    ///
    /// ```text
    /// but clean
    /// ```
    ///
    /// Preview which branches would be removed:
    ///
    /// ```text
    /// but clean --dry-run
    /// ```
    ///
    /// Pull latest changes first, then clean:
    ///
    /// ```text
    /// but clean --pull
    /// ```
    ///
    /// Also remove branches that only have upstream commits:
    ///
    /// ```text
    /// but clean --include-upstream
    /// ```
    ///
    #[cfg(feature = "legacy")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Clean {
        /// Preview which branches would be removed without actually deleting them.
        #[clap(long)]
        dry_run: bool,
        /// Pull latest changes from the remote before cleaning.
        #[clap(long)]
        pull: bool,
        /// Also remove branches that have upstream-only commits but no local commits or changes.
        #[clap(long)]
        include_upstream: bool,
    },

    /// Manage GitButler CLI and app updates.
    ///
    /// Check for new versions, install updates, or suppress update notifications.
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
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
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
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
    /// but config push-remote
    /// ```
    ///
    /// View/set metrics:
    ///
    /// ```text
    /// but config metrics
    /// ```
    ///
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Config(config::Platform),

    #[cfg(feature = "legacy")]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Pick(pick::Platform),

    /// Switch to a local branch, workspace branch ID, or the GitButler workspace.
    ///
    /// ## Examples
    ///
    /// Switch to a branch:
    ///
    /// ```text
    /// but switch my-feature
    /// ```
    ///
    /// Switch back to the GitButler workspace:
    ///
    /// ```text
    /// but switch --workspace
    /// ```
    ///
    /// Create a new branch at the project target and switch to it:
    ///
    /// ```text
    /// but switch --new
    /// ```
    ///
    /// Create a named branch at the project target and switch to it:
    ///
    /// ```text
    /// but switch --new my-feature
    /// ```
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    #[clap(hide = true, group(
        clap::ArgGroup::new("switch_target")
            .args(["target", "workspace", "new"])
            .required(true)
            .multiple(true)
    ))]
    Switch {
        /// Branch name, full local branch ref, workspace CLI branch ID, or new branch name with --new
        target: Option<CliIdArg>,
        /// Switch back to gitbutler/workspace
        #[clap(long, short = 'w', conflicts_with_all = &["target", "new"])]
        workspace: bool,
        /// Create a branch at the project target and switch to it
        #[clap(long = "new", short = 'n')]
        new: bool,
    },

    /// Manage AI agent skills for GitButler.
    ///
    /// Skills provide enhanced AI capabilities for working with GitButler through
    /// Claude Code, Codex, and other AI assistants.
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
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Skill(skill::Platform),

    /// Set up GitButler for AI coding agents.
    ///
    /// Runs a guided setup wizard for installing GitButler agent skills and
    /// writing workflow steering instructions into supported agent instruction
    /// files.
    ///
    /// ## Examples
    ///
    /// Start the interactive setup wizard (`but agent setup` is equivalent):
    ///
    /// ```text
    /// but agent
    /// ```
    ///
    /// Print the default generated steering text:
    ///
    /// ```text
    /// but agent setup --print
    /// ```
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Agent(agent::Platform),

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
    #[clap(hide = true)]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Edit {
        /// Path to the file to edit (created if it doesn't exist)
        file: String,
    },

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
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Worktree(worktree::Platform),

    /// Trigger a refresh of remote data fetching from the remote, Pull Requests, and CI status.
    ///
    /// This is a hidden command primarily used for background sync operations.
    #[cfg(feature = "legacy")]
    #[clap(hide = true)]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
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

    /// Run GitButler as a Model Context Protocol server.
    ///
    /// The server exposes workspace and review tools with MCP App views to
    /// clients such as Codex, Claude Code, and ChatGPT.
    #[clap(hide = true)]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Mcp(mcp::Platform),

    /// INTERNAL: GitButler Actions are automated tasks (like macros) that can be performed on a repository.
    #[cfg(feature = "legacy")]
    #[clap(hide = true)]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Actions(actions::Platform),

    /// INTERNAL: If metrics are permitted, this subcommand handles posthog event creation.
    #[clap(hide = true)]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Metrics {
        #[clap(long, value_enum)]
        command_name: metrics::CommandName,
        #[clap(long)]
        props: String,
    },

    /// Generate `but` shell completions.
    ///
    /// ## Examples
    ///
    /// ```bash
    /// # bash, put in .bashrc or .bash_profile depending on system setup
    /// eval "$(but completions bash)"
    ///
    /// # zsh, put in .zshrc
    /// eval "$(but completions zsh)"
    ///
    /// # fish, put in config.fish
    /// but completions fish | source
    /// ```
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Completions {
        /// The shell to generate completions for, or the one extracted from the `SHELL` environment variable.
        #[clap(value_enum)]
        shell: Option<clap_complete::Shell>,
    },

    /// Hidden command that redirects to `but pull --check`
    #[cfg(feature = "legacy")]
    #[clap(hide = true)]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Fetch,

    /// AI: capture agent logs into GitMeta.
    #[clap(name = "agentlog", hide = true)]
    AgentLog {
        #[clap(subcommand)]
        cmd: but_agentlog::Command,
    },

    /// Show help information.
    ///
    /// Without a topic, displays all available commands organized into functional categories
    /// such as Inspection, Branching and Committing, Server Interactions, etc.
    ///
    /// With a topic, displays help for that topic, such as `but help cli-ids`.
    #[clap(hide = true)]
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Help {
        /// Help topic to show.
        #[clap(subcommand)]
        topic: Option<HelpTopic>,
    },

    /// INTERNAL: First-run onboarding that shows metrics info and marks onboarding complete.
    ///
    /// This command is silent if onboarding has already been completed.
    /// It is designed to be called by the installer after installation.
    #[clap(hide = true)]
    Onboarding,

    /// External commands and aliases are resolved to this variant.
    ///
    /// External commands are currently only supported on UNIX platforms, but this is available on
    /// Windows for alias resolution.
    #[strum(disabled)]
    #[clap(hide = true, external_subcommand)]
    External(Vec<OsString>),
}

pub mod agent;
pub mod alias;
#[cfg(feature = "legacy")]
pub mod amend;
#[cfg(feature = "legacy")]
pub mod apply;
pub mod comment;
#[cfg(feature = "legacy")]
pub mod commit;
pub mod config;
#[cfg(feature = "legacy")]
pub mod diff2;
#[cfg(feature = "legacy")]
pub mod discard;
pub mod mcp;
#[cfg(feature = "legacy")]
pub mod r#move;
#[cfg(feature = "legacy")]
pub mod pick;
#[cfg(feature = "legacy")]
pub mod redo;
#[cfg(feature = "legacy")]
pub mod reword2;
pub mod skill;
#[cfg(feature = "legacy")]
pub mod squash;
#[cfg(feature = "legacy")]
pub mod tui;
#[cfg(feature = "legacy")]
pub mod unapply;
#[cfg(feature = "legacy")]
pub mod uncommit;
#[cfg(feature = "legacy")]
pub mod undo;
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

/// Find the subcommand token and its index in raw argv, skipping root options.
pub(crate) fn find_subcommand(args: &[std::ffi::OsString]) -> Option<(usize, &std::ffi::OsString)> {
    let mut ix = 1;
    while ix < args.len() {
        let token = &args[ix].as_encoded_bytes();
        // Root options that consume a separate value token. `-C` is the only
        // value-taking short option, so a short cluster ending in it (e.g.
        // `-tC`) consumes the next token too; with an attached value
        // (`-C.`, `-tC.`) the cluster ends in the value instead.
        let cluster_ending_in_c =
            token.starts_with(b"-") && !token.starts_with(b"--") && token.ends_with(b"C");
        if cluster_ending_in_c || *token == b"--current-dir" || *token == b"--log-file" {
            ix += 2;
            continue;
        }
        if token.starts_with(b"-") {
            ix += 1;
            continue;
        }
        return Some((ix, &args[ix]));
    }
    None
}

/// Example invocations appended to a subcommand's parse error, for the
/// commands whose grammar most often trips people up.
pub(crate) fn error_examples(subcommand: &str) -> Option<&'static str> {
    #[cfg(feature = "legacy")]
    {
        Some(match subcommand {
            "commit" => commit::ERROR_EXAMPLES,
            "amend" => amend::ERROR_EXAMPLES,
            "move" => r#move::ERROR_EXAMPLES,
            "squash" => squash::ERROR_EXAMPLES,
            _ => return None,
        })
    }
    #[cfg(not(feature = "legacy"))]
    {
        let _ = subcommand;
        None
    }
}

#[cfg(test)]
mod tests;
