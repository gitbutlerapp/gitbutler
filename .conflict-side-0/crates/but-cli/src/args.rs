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
    /// Whether to use json output format.
    #[clap(long, short = 'j')]
    pub json: bool,

    #[clap(subcommand)]
    pub cmd: Subcommands,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
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
    HunkDependency,
    /// Returns the list of stacks that are currently part of the GitButler workspace.
    Stacks,
    /// Return all stack branches related to the given `id`.
    StackBranches { id: String },
    /// Returns all commits for the branch with the given `name` in the stack with the given `id`.
    StackBranchCommits { id: String, name: String },
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
