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

    #[clap(subcommand)]
    pub cmd: Subcommands,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum UpdateMode {
    Rebase,
    Merge,
    Unapply,
    Delete,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// List and manipulate virtual branches.
    #[clap(visible_alias = "branches")]
    Branch(vbranch::Platform),
    /// List and manipulate projects.
    #[clap(visible_alias = "projects")]
    Project(project::Platform),
}

pub mod vbranch {

    #[derive(Debug, clap::Parser)]
    pub struct Platform {
        #[clap(subcommand)]
        pub cmd: Option<SubCommands>,
    }

    #[derive(Debug, clap::Subcommand)]
    pub enum SubCommands {
        /// Add a branch to the workspace.
        Apply {
            /// Whether it's a branch that we're applying.
            ///
            /// If a stack create from the given branch is not found a new stack is created.
            #[clap(short = 'b', long, default_value_t = false)]
            branch: bool,
            /// The name of the stack to apply.
            ///
            /// If the flag `--branch` is set, this is the name of the branch to apply a stack from.
            name: String,
        },
        /// Create a new commit to named virtual branch with all changes currently in the worktree or staging area assigned to it.
        Commit {
            /// The commit message
            #[clap(short = 'm', long)]
            message: String,
            /// The name of the virtual to commit all staged and unstaged changes to.
            name: String,
        },
        /// Create a new series on top of the stack.
        Series {
            /// The name of the series to create on top of the stack.
            #[clap(short = 's', long)]
            series_name: String,
            /// The name of the stack to create new series for.
            name: String,
        },
        /// Create a new virtual branch
        Create {
            /// Also make this branch the default branch, so it is considered the owner of new edits.
            #[clap(short = 'd', long)]
            set_default: bool,
            /// The name of the virtual branch to create
            name: String,
        },
    }
}

pub mod project {
    use std::path::PathBuf;

    use gitbutler_reference::RemoteRefname;

    #[derive(Debug, clap::Parser)]
    pub struct Platform {
        /// The location of the directory to contain app data.
        ///
        /// Defaults to the standard location on this platform if unset.
        #[clap(short = 'd', long, env = "GITBUTLER_CLI_DATA_DIR")]
        pub app_data_dir: Option<PathBuf>,
        /// A suffix like `dev` to refer to projects of the development version of the application.
        ///
        /// The production version is used if unset.
        #[clap(short = 's', long)]
        pub app_suffix: Option<String>,
        #[clap(subcommand)]
        pub cmd: Option<SubCommands>,
    }

    #[derive(Debug, clap::Subcommand)]
    pub enum SubCommands {
        /// Add the given Git repository as project for use with GitButler.
        Add {
            /// The long name of the remote reference to track, like `refs/remotes/origin/main`,
            /// when switching to the workspace branch.
            #[clap(short = 's', long)]
            switch_to_workspace: Option<RemoteRefname>,
            /// The path at which the repository worktree is located.
            #[clap(default_value = ".", value_name = "REPOSITORY")]
            path: PathBuf,
        },
    }
}

pub mod snapshot {
    #[derive(Debug, clap::Parser)]
    pub struct Platform {
        #[clap(subcommand)]
        pub cmd: Option<SubCommands>,
    }

    #[derive(Debug, clap::Subcommand)]
    pub enum SubCommands {
        /// Restores the state of the working directory as well as virtual branches to a given snapshot.
        Restore {
            /// The snapshot to restore
            snapshot_id: String,
        },
        /// Show what is stored in a given snapshot.
        Diff {
            /// The hex-hash of the commit-id of the snapshot.
            snapshot_id: String,
        },
    }
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
