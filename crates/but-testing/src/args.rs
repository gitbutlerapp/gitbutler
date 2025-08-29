use gitbutler_reference::RemoteRefname;
use gitbutler_stack::StackId;
use std::path::PathBuf;

#[derive(Debug, clap::Parser)]
#[clap(name = "gitbutler-cli", about = "A CLI for GitButler", version = option_env!("GIX_VERSION"))]
pub struct Args {
    /// Enable tracing for debug and performance information printed to stderr.
    #[clap(short = 'd', long)]
    pub trace: bool,
    /// Run as if gitbutler-cli was started in PATH instead of the current working directory.
    #[clap(short = 'C', long, default_value = ".", value_name = "PATH")]
    pub current_dir: PathBuf,
    /// The location of the directory to contain app data.
    ///
    /// Defaults to the standard location on this platform if unset.
    #[clap(short = 'a', long, env = "GITBUTLER_CLI_DATA_DIR")]
    pub app_data_dir: Option<PathBuf>,
    /// A suffix like `dev` to refer to projects of the development version of the application.
    ///
    /// The production version is used if unset.
    #[clap(short = 's', long)]
    pub app_suffix: Option<String>,
    /// Turn on V3 mode for those subcommands that support it.
    ///
    /// This helps test the output of certain functions in V3 mode, and/or compare.
    #[clap(short = '3', long, env = "BUT3")]
    pub v3: bool,
    /// Whether to use JSON output format.
    #[clap(long, short = 'j')]
    pub json: bool,

    #[clap(subcommand)]
    pub cmd: Subcommands,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Add the given Git repository as project for use with GitButler.
    AddProject {
        /// The long name of the remote reference to track, like `refs/remotes/origin/main`,
        /// when switching to the workspace branch.
        #[clap(short = 's', long)]
        switch_to_workspace: Option<RemoteRefname>,
        /// The path at which the repository worktree is located.
        #[clap(default_value = ".", value_name = "REPOSITORY")]
        path: PathBuf,
    },
    /// Commit or amend all worktree changes to a new commit.
    Commit {
        /// The repo-relative path to the changed file to commit.
        #[clap(requires_if(clap::builder::ArgPredicate::IsPresent, "hunk_headers"))]
        current_path: Option<PathBuf>,
        /// If the change is a rename, identify the repo-relative path of the source.
        previous_path: Option<PathBuf>,
        /// The 1-based pairs of 4 numbers equivalent to '(old_start,old_lines,new_start,new_lines)'
        #[clap(
            long,
            requires_if(clap::builder::ArgPredicate::IsPresent, "current_path"),
            num_args = 4,
            value_names = ["old-start", "old-lines", "new-start", "new-lines"])
        ]
        hunk_headers: Vec<u32>,
        /// The message of the new commit.
        #[clap(long, short = 'm')]
        message: Option<String>,
        /// The name of the reference that the commit should be in.
        ///
        /// If there is ambiguity, this is what makes it ambiguous.
        #[clap(long, short = 's')]
        stack_segment_ref: Option<String>,
        /// Amend to the current or given commit.
        #[clap(long)]
        amend: bool,
        /// The rev-spec of the tip of the workspace.
        // TODO: this should be replaced with head-info discovery once available.
        #[clap(long)]
        workspace_tip: Option<String>,
        /// The revspec to create the commit on top of, or the commit to amend to.
        #[clap(long)]
        parent: Option<String>,
        /// A JSON specification of the changes to commit.
        #[clap(long)]
        diff_spec: Option<String>,
    },
    /// List all uncommitted working tree changes.
    Status {
        /// Also compute unified diffs for each tree-change.
        #[clap(long, short = 'c', default_value_t = crate::command::UI_CONTEXT_LINES)]
        context_lines: u32,
        /// Also compute unified diffs for each tree-change.
        #[clap(long, short = 'd')]
        unified_diff: bool,
    },
    /// Discard the specified worktree change.
    DiscardChange {
        /// The zero-based indices of all hunks to discard.
        #[clap(long)]
        hunk_indices: Vec<usize>,
        /// The 1-based pairs of 4 numbers equivalent to '(old_start,old_lines,new_start,new_lines)'
        #[clap(long, num_args = 4, conflicts_with = "hunk_indices", value_names = ["old-start", "old-lines", "new-start", "new-lines"])]
        hunk_headers: Vec<u32>,
        /// The repo-relative path to the changed file to discard.
        current_path: PathBuf,
        /// If the change is a rename, identify the repo-relative path of the source.
        previous_path: Option<PathBuf>,
    },
    /// Calculate the changes between two commits.
    CommitChanges {
        /// Also compute unified diffs for each tree-change.
        #[clap(long, short = 'd')]
        unified_diff: bool,
        /// The revspec to the commit that the returned changes turn the previous commit into.
        current_commit: String,
        /// The revspec to the previous commit that the returned changes transform into current commit.
        previous_commit: Option<String>,
    },
    /// Return the dependencies of worktree changes with the commits that last changed them.
    #[clap(visible_alias = "dep")]
    HunkDependency {
        /// Whether to show the dependencies in the ui format.
        #[clap(long, default_value_t = false)]
        simple: bool,
    },
    Watch,
    WatchDb,
    #[clap(visible_alias = "operating-mode", alias = "opmode")]
    OpMode,
    /// Returns the current hunk assignments
    HunkAssignments,
    AssignHunk {
        path: String,
        stack_id: StackId,
        old_start: u32,
        old_lines: u32,
        new_start: u32,
        new_lines: u32,
    },
    /// Returns the list of stacks that are currently part of the GitButler workspace.
    Stacks {
        /// Whether to list only the stacks in the workspace.
        #[clap(long, short = 'w')]
        workspace_only: bool,
    },
    /// Returns details about a stack, as identified by StackID.
    ///
    /// As StackIDs are going away, BranchDetails would be the part that remains.
    StackDetails {
        /// The ID of the stack to list details for.
        id: StackId,
    },
    /// Returns detailed
    BranchDetails {
        /// A resolvable reference name.
        ref_name: String,
    },
    /// Returns everything we know about the given ref, or `HEAD`.
    RefInfo {
        /// Perform all possible computations.
        #[clap(long, short = 'e')]
        expensive: bool,
        /// The name of the ref to get workspace information for.
        ref_name: Option<String>,
    },
    /// Returns a segmented graph starting from `HEAD`.
    Graph {
        /// Debug-print the whole graph.
        #[clap(long, short = 'd')]
        debug: bool,
        /// The rev-spec of the extra target to provide for traversal.
        #[clap(long)]
        extra_target: Option<String>,
        /// Do not debug-print the workspace.
        ///
        /// If too large, it takes a long time or runs out of memory.
        #[clap(long)]
        no_debug_workspace: bool,
        /// Do not output the dot-file to stdout.
        #[clap(long, conflicts_with = "no_open")]
        no_dot: bool,
        /// The maximum number of commits to traverse.
        ///
        /// Use only as safety net to prevent runaways.
        #[clap(long)]
        hard_limit: Option<usize>,
        /// The hint of the number of commits to traverse.
        ///
        /// Specifying no limit with `--limit` removes all limits.
        #[clap(long, short = 'l', default_value = "300")]
        limit: Option<Option<usize>>,
        /// Refill the limit when running over these hashes, provided as short or long hash.
        #[clap(long, short = 'e')]
        limit_extension: Vec<String>,
        /// Avoid opening the resulting dot-file and instead write it to standard output.
        #[clap(long)]
        no_open: bool,
        /// The name of the ref to start the graph traversal at.
        ref_name: Option<String>,
    },
    /// Return all stack branches related to the given `id`.
    StackBranches {
        /// The ID of the stack to list branches from.
        ///
        /// If creating a branch, this is optionally the stack to which the branch will be added.
        /// If no ID is present while creating a branch, a new stack will be created that will
        /// contain the brand new branch.
        id: Option<StackId>,
        /// Optional. The name of the branch to create.
        ///
        /// If this is set, a branch will be created with the given name.
        #[clap(long, short = 'b')]
        branch_name: Option<String>,
        /// Optional. The description of the branch to create.
        ///
        /// This is the place where some metadata about the branch can be stored.
        #[clap(long, short = 'd')]
        description: Option<String>,
    },
    /// Create a reference at the given position (dependent and independent)
    CreateReference {
        /// Create the branch above the given commit or branch short name.
        #[arg(long, short = 'a', conflicts_with = "below")]
        above: Option<String>,
        /// Create the branch below the given commit or branch short name.
        #[arg(long, short = 'b', conflicts_with = "above")]
        below: Option<String>,
        /// the short-name of the new branch.
        short_name: String,
    },
    /// Delete the given workspace reference.
    RemoveReference {
        /// Allow stacks to be empty.
        #[arg(long)]
        permit_empty_stacks: bool,
        /// Do not delete the branch metadata associated with the deleted reference.
        ///
        /// Useful if it might be recreated, afterwards.
        #[arg(long)]
        keep_metadata: bool,
        /// the short-name of the reference to delete.
        short_name: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clap() {
        use clap::CommandFactory;
        Args::command().debug_assert();
    }
}
