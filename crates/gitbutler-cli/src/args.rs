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

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Unapply the given ownership claim.
    UnapplyOwnership {
        /// The path to remove the claim from.
        filepath: PathBuf,
        /// The first line of hunks that should be removed.
        from_line: u32,
        /// The last line of hunks that should be removed.
        to_line: u32,
    },
    /// List and manipulate virtual branches.
    #[clap(visible_alias = "branches")]
    Branch(vbranch::Platform),
    /// List and manipulate projects.
    #[clap(visible_alias = "projects")]
    Project(project::Platform),
    /// List and restore snapshots.
    #[clap(visible_alias = "snapshots")]
    Snapshot(snapshot::Platform),
}

pub mod vbranch {
    use gitbutler_branch::BranchIdentity;

    #[derive(Debug, clap::Parser)]
    pub struct Platform {
        #[clap(subcommand)]
        pub cmd: Option<SubCommands>,
    }

    #[derive(Debug, clap::Subcommand)]
    pub enum SubCommands {
        /// List all local branches that aren't GitButler specific.
        ListLocal,
        /// Provide the current state of all applied virtual branches.
        Status,
        /// Switch to the GitButler workspace.
        SetBase {
            /// The name of the remote branch to integrate with, like `origin/main`.
            short_tracking_branch_name: String,
        },
        /// Make the named branch the default so all worktree or index changes are associated with it automatically.
        SetDefault {
            /// The name of the new default virtual branch.
            name: String,
        },
        /// Remove a branch from the workspace.
        Unapply {
            /// The name of the virtual branch to unapply.
            name: String,
        },
        /// Add a branch to the workspace.
        Apply {
            /// The name of the virtual branch to apply.
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
        /// Provide details about given branches.
        Details {
            /// The short-name/identity of branches to list.
            names: Vec<BranchIdentity>,
        },
        /// List all branches that can be relevant.
        ListAll,
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
        /// Switch back to the workspace branch for use of virtual branches.
        SwitchToWorkspace {
            /// The long name of the remote reference to track, like `refs/remotes/origin/main`.
            remote_ref_name: RemoteRefname,
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
        /// Restores the state of the working direcory as well as virtual branches to a given snapshot.
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
