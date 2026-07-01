use super::WorkspaceState;
use std::collections::BTreeMap;

use but_core::{DryRun, RefMetadata};
use but_rebase::graph_rebase::{MaterializeOutcome, SuccessfulRebase};

use but_workspace::RefInfo;

impl WorkspaceState {
    /// Create a new workspace state from operation outputs.
    pub fn new(
        replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
        head_info: RefInfo,
    ) -> Self {
        Self {
            replaced_commits,
            head_info,
        }
    }

    /// Build a [`WorkspaceState`] from an already-prepared overlayed graph.
    ///
    /// Use this when the caller already has a graph describing the workspace after the
    /// intended operation, regardless of whether that graph came from a preview, a
    /// materialized rebase, or another graph-producing workflow. The caller is
    /// responsible for supplying the matching `replaced_commits` map for that graph.
    ///
    /// This is the most direct constructor in this module and is the right choice when
    /// there is no need to inspect or materialize a [`SuccessfulRebase`].
    pub(crate) fn from_workspace(
        workspace: &but_graph::Workspace,
        repo: &gix::Repository,
        replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
    ) -> anyhow::Result<WorkspaceState> {
        let head_info = but_workspace::graph_to_ref_info(
            workspace,
            repo,
            but_workspace::ref_info::Options {
                project_meta: workspace.graph.project_meta.clone(),
                traversal: but_graph::init::Options::limited(),
                expensive_commit_info: true,
                ..Default::default()
            },
        )?
        .pruned_to_entrypoint();

        Ok(WorkspaceState::new(replaced_commits, head_info))
    }

    /// Build a preview [`WorkspaceState`] from a successful rebase without materializing it.
    ///
    /// Use this when the caller needs to report the post-rebase workspace layout before
    /// writing the rebase result back to the repository, such as dry-run flows or
    /// operations that intentionally preview the outcome first and materialize later.
    ///
    /// The `replaced_commits` map should describe the commit rewrites visible in the
    /// preview graph, which typically comes from `rebase.history.commit_mappings()`.
    pub fn from_rebase_preview<M: RefMetadata>(
        rebase: &SuccessfulRebase<'_, '_, M>,
        replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
    ) -> anyhow::Result<WorkspaceState> {
        Self::from_workspace(
            &rebase.overlayed_graph()?.into_workspace()?,
            rebase.repo(),
            replaced_commits,
        )
    }

    /// Build a [`WorkspaceState`] from an already-materialized rebase.
    ///
    /// Use this when the caller needs to perform additional bookkeeping after materialization
    /// before constructing the final workspace state.
    pub fn from_materialized_rebase<M: RefMetadata>(
        materialized: MaterializeOutcome<'_, '_, M>,
        repo: &gix::Repository,
    ) -> anyhow::Result<WorkspaceState> {
        Self::from_workspace(
            materialized.workspace,
            repo,
            materialized.history.commit_mappings(),
        )
    }

    /// Build a [`WorkspaceState`] from a successful rebase, materializing it when needed.
    ///
    /// Use this as the default entry point when an operation ends with a [`SuccessfulRebase`] and
    /// the API should return the resulting workspace state. When `dry_run` is `true`, this
    /// delegates to [`WorkspaceState::from_rebase_preview`] so the caller sees the projected state
    /// without changing the repository. Otherwise it materializes the rebase, then reports the
    /// workspace state together with the final commit-replacement mappings returned by the
    /// materialized history.
    pub fn from_successful_rebase<M: RefMetadata>(
        rebase: SuccessfulRebase<'_, '_, M>,
        repo: &gix::Repository,
        dry_run: DryRun,
    ) -> anyhow::Result<WorkspaceState> {
        if dry_run.into() {
            return Self::from_rebase_preview(&rebase, rebase.history.commit_mappings());
        }

        let materialized = rebase.materialize()?;
        Self::from_workspace(
            materialized.workspace,
            repo,
            materialized.history.commit_mappings(),
        )
    }
}
