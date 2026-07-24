//! Command-line argument parsing for `but-debug`.

use std::path::PathBuf;

/// Top-level CLI arguments for `but-debug`.
#[derive(Debug, clap::Parser)]
#[command(
    name = "but-debug",
    about = "Debugging utilities for GitButler repositories",
    version = option_env!("GIX_VERSION")
)]
pub struct Args {
    /// Enable tracing for debug and performance information printed to stderr.
    #[arg(short = 't', long, action = clap::ArgAction::Count)]
    pub trace: u8,
    /// Run as if `but-debug` was started in `PATH` instead of the current working directory.
    #[arg(
        short = 'C',
        long,
        default_value = ".",
        value_name = "PATH",
        global = true
    )]
    pub current_dir: PathBuf,
    /// The debugging command to run.
    #[command(subcommand)]
    pub cmd: Subcommands,
}

/// The debugging subcommands supported by `but-debug`.
#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Call selected but-api entry points directly.
    Api(ApiArgs),
    /// Archive a repository for debugging.
    Dump(DumpArgs),
    /// Return a segmented graph starting from `HEAD`.
    Graph(GraphArgs),
    /// Apply a local branch using the new but-workspace apply path.
    Apply(ApplyArgs),
    /// Unapply a local branch using the new but-workspace unapply path.
    Unapply(UnapplyArgs),
    /// Debug revision graph operations.
    #[clap(visible_alias = "rev")]
    Revision(RevisionArgs),
}

/// Arguments for direct `but-api` calls.
#[derive(Debug, clap::Args)]
pub struct ApiArgs {
    /// The API command to run.
    #[command(subcommand)]
    pub cmd: ApiSubcommands,
}

/// The `but-api` entry points exposed by `but-debug`.
#[derive(Debug, clap::Subcommand)]
pub enum ApiSubcommands {
    /// List branches through `but_api::branch::branch_list`.
    BranchList,
    /// Unapply a stack through `but_api::legacy::virtual_branches::unapply_stack`.
    #[command(name = "unapply-stack", visible_alias = "unapply")]
    UnapplyStack(ApiUnapplyStackArgs),
}

/// Arguments for `but-debug api unapply-stack`.
#[derive(Debug, clap::Args)]
pub struct ApiUnapplyStackArgs {
    /// Shared workspace debug output options.
    #[command(flatten)]
    pub debug: DebugWorkspaceArgs,
    /// Stack UUID, stack tip branch name, or any branch name in the stack.
    pub stack: String,
}

/// Arguments for the `dump` debugging subcommand.
#[derive(Debug, clap::Args)]
pub struct DumpArgs {
    /// The kind of dump archive to create.
    #[command(subcommand)]
    pub cmd: DumpSubcommands,
}

/// The archive kinds supported by `but-debug dump`.
#[derive(Debug, clap::Subcommand)]
pub enum DumpSubcommands {
    /// Archive a repository for debugging.
    Repo(RepoDumpArgs),
    /// Archive graph and workspace diagnostics.
    #[clap(visible_alias = "diag")]
    Diagnostics(DiagnosticsDumpArgs),
}

/// Arguments for the `dump repo` debugging subcommand.
#[derive(Debug, clap::Args)]
pub struct RepoDumpArgs {
    /// Shared archive output options.
    #[command(flatten)]
    pub archive: ArchiveOutputArgs,
    /// Diagnostics capture options.
    #[command(flatten)]
    pub diagnostics: DiagnosticsCaptureArgs,
    /// Include only Git directory state and skip worktree files.
    #[arg(long)]
    pub git_only: bool,
    /// Do not include graph and workspace diagnostics in the archive root.
    #[arg(long)]
    pub no_diagnostics: bool,
}

/// Archive output options shared by dump subcommands.
#[derive(Debug, clap::Args)]
pub struct ArchiveOutputArgs {
    /// Where to write the zip archive.
    #[arg(long, value_name = "PATH")]
    pub output: Option<PathBuf>,
    /// Do not open the directory containing the archive after it is created.
    #[arg(long = "no-open-archive-directory", visible_alias = "no-open")]
    pub no_open_archive_directory: bool,
}

/// Arguments for the `dump diagnostics` debugging subcommand.
#[derive(Debug, clap::Args)]
pub struct DiagnosticsDumpArgs {
    /// Shared archive output options.
    #[command(flatten)]
    pub archive: ArchiveOutputArgs,
    /// Diagnostics capture options.
    #[command(flatten)]
    pub diagnostics: DiagnosticsCaptureArgs,
}

/// Options for graph and workspace diagnostics capture.
#[derive(Debug, clap::Args)]
pub struct DiagnosticsCaptureArgs {
    /// Maximum seconds to wait for `dot -Tsvg`; 0 disables the timeout.
    #[arg(long = "dot-timeout", value_name = "SECONDS", default_value_t = 30)]
    pub dot_timeout_seconds: u32,
}

/// Arguments for the `graph` debugging subcommand.
#[derive(Debug, clap::Args)]
pub struct GraphArgs {
    /// Debug-print the whole graph and ignore all other dot-related flags.
    #[arg(long, short = 'd')]
    pub debug: bool,
    /// Print graph statistics first to get a grasp of huge graphs.
    #[arg(long, short = 's')]
    pub stats: bool,
    /// The rev-spec of the extra target to provide for traversal.
    #[arg(long)]
    pub extra_target: Option<String>,
    /// Disable post-processing of the graph, useful if that's failing.
    #[arg(long)]
    pub no_post: bool,
    /// Do not debug-print the workspace.
    ///
    /// If too large, it takes a long time or runs out of memory.
    #[arg(long)]
    pub no_debug_workspace: bool,
    /// Output the dot-file to stdout.
    #[arg(long, conflicts_with = "dot_show")]
    pub dot: bool,
    /// The maximum number of commits to traverse.
    ///
    /// Use only as safety net to prevent runaways.
    #[arg(long)]
    pub hard_limit: Option<usize>,
    /// The hint of the number of commits to traverse.
    ///
    /// Specifying no limit with `--limit` removes all limits.
    #[arg(long, short = 'l', default_value = "300")]
    pub limit: Option<Option<usize>>,
    /// Refill the limit when running over these hashes, provided as short or long hash.
    #[arg(long, short = 'e')]
    pub limit_extension: Vec<String>,
    /// Open the dot-file as SVG instead of writing it to stdout.
    #[arg(long)]
    pub dot_show: bool,
    /// The name of the ref to start the graph traversal at.
    pub ref_name: Option<String>,
}

/// Arguments for direct workspace mutation commands.
#[derive(Debug, clap::Args)]
pub struct ApplyArgs {
    /// Shared workspace debug output options.
    #[command(flatten)]
    pub debug: DebugWorkspaceArgs,
    /// Branch or full ref name to apply or unapply.
    pub ref_name: String,
}

/// Arguments for direct workspace unapply commands.
#[derive(Debug, clap::Args)]
pub struct UnapplyArgs {
    /// Shared workspace debug output options.
    #[command(flatten)]
    pub debug: DebugWorkspaceArgs,
    /// How the workspace should be represented after unapplying.
    #[arg(long, value_enum, default_value_t = WorkspaceDispositionArg::KeepWorkspaceReference)]
    pub disposition: WorkspaceDispositionArg,
    /// Branch or full ref name to unapply.
    pub ref_name: String,
}

/// Workspace disposition choices exposed by `but-debug unapply`.
#[derive(Debug, Copy, Clone, clap::ValueEnum)]
pub enum WorkspaceDispositionArg {
    /// Preserve the managed workspace commit even if it is no longer necessary.
    KeepWorkspaceCommit,
    /// Collapse unnecessary workspace commits, but keep the workspace reference checked out.
    KeepWorkspaceReference,
    /// Delete the workspace reference after switching away when it is no longer necessary.
    PreventUnnecessaryWorkspaceReferences,
    /// Delete the workspace reference when possible, otherwise keep the workspace merge commit for compatibility.
    PreventUnnecessaryWorkspaceReferencesKeepWorkspaceCommit,
}

impl From<WorkspaceDispositionArg> for but_workspace::branch::unapply::WorkspaceDisposition {
    fn from(value: WorkspaceDispositionArg) -> Self {
        use but_workspace::branch::unapply::WorkspaceDisposition as T;
        match value {
            WorkspaceDispositionArg::KeepWorkspaceCommit => T::KeepWorkspaceCommit,
            WorkspaceDispositionArg::KeepWorkspaceReference => T::KeepWorkspaceReference,
            WorkspaceDispositionArg::PreventUnnecessaryWorkspaceReferences => {
                T::PreventUnnecessaryWorkspaceReferences
            }
            WorkspaceDispositionArg::PreventUnnecessaryWorkspaceReferencesKeepWorkspaceCommit => {
                T::PreventUnnecessaryWorkspaceReferencesKeepWorkspaceCommit
            }
        }
    }
}

/// Shared option for commands that can debug-print the post-operation workspace.
#[derive(Debug, clap::Args)]
pub struct DebugWorkspaceArgs {
    /// Debug-print the workspace after the mutation.
    #[arg(long, visible_alias = "ws")]
    pub debug_workspace: bool,
    /// Re-read the workspace from repository state before emitting post-operation workspace output.
    #[arg(long, visible_alias = "invalidate")]
    pub invalidate_workspace: bool,
}

impl DebugWorkspaceArgs {
    /// Emit the workspace details controlled only by these debug options.
    pub fn emit_workspace(
        &self,
        workspace: &but_graph::Workspace,
        err: &mut dyn std::io::Write,
    ) -> anyhow::Result<()> {
        if self.debug_workspace {
            writeln!(err, "{workspace:#?}")?;
        }
        Ok(())
    }
}

/// Arguments for the `revision` subcommand.
#[derive(Debug, clap::Args)]
pub struct RevisionArgs {
    /// The revision debugging command to run.
    #[command(subcommand)]
    pub cmd: RevisionSubcommands,
}

/// The debugging subcommands supported by `but-debug revision`.
#[derive(Debug, clap::Subcommand)]
pub enum RevisionSubcommands {
    /// Print commits reachable by a rev-spec.
    Log(LogArgs),
    /// Compute the octopus merge-base for two or more revisions.
    #[command(name = "merge-base")]
    MergeBase(MergeBaseArgs),
}

/// Graph construction options shared by revision debugging subcommands.
#[derive(Debug, clap::Args)]
pub struct RevisionGraphArgs {
    /// The named reference to use as the workspace target during graph traversal.
    #[arg(long)]
    pub target_ref: Option<String>,
    /// The rev-spec of the extra target to provide for graph traversal.
    #[arg(long)]
    pub extra_target: Option<String>,
}

/// Arguments for the `revision log` debugging subcommand.
#[derive(Debug, clap::Args)]
pub struct LogArgs {
    /// Shared graph construction options.
    #[command(flatten)]
    pub graph: RevisionGraphArgs,
    /// Follow only the first parent when traversing merge commits.
    #[arg(long)]
    pub first_parent: bool,
    /// The rev-spec to log. Exclusive ranges like `main..branch` are supported.
    pub rev_spec: String,
}

/// Arguments for the `revision merge-base` debugging subcommand.
#[derive(Debug, clap::Args)]
pub struct MergeBaseArgs {
    /// Shared graph construction options.
    #[command(flatten)]
    pub graph: RevisionGraphArgs,
    /// The rev-specs whose octopus merge-base should be computed.
    #[arg(required = true, num_args = 2.., value_name = "REV")]
    pub revisions: Vec<String>,
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::Args;

    #[test]
    fn clap_configuration_is_valid() {
        Args::command().debug_assert();
    }
}
