use super::WorkspaceState;
use std::collections::{BTreeMap, HashMap};

use but_core::{DryRun, RefMetadata};
use but_rebase::graph_rebase::{MaterializeOutcome, SuccessfulRebase};

/// Build a `{ pushed short name -> PR number }` lookup from the forge review
/// cache, for resolving branch PR associations at projection time.
///
/// This is the same view of forge truth the `head_info` read command uses, so
/// that mutation responses — which the Lite app writes straight into its
/// head_info cache without refetching — stay consistent with the query.
pub(crate) fn forge_prs_by_head(ctx: &but_ctx::Context) -> anyhow::Result<HashMap<String, usize>> {
    let db = ctx.db.get_cache()?;
    but_forge::pr_numbers_by_head(&db)
}

impl WorkspaceState {
    /// Whether any commit in the projected workspace is in a conflicted state.
    #[cfg(not(feature = "graph-workspace"))]
    pub fn is_conflicted(&self) -> bool {
        self.head_info
            .stacks
            .iter()
            .flat_map(|stack| &stack.segments)
            .flat_map(|segment| &segment.commits)
            .any(|commit| commit.has_conflicts)
    }

    /// Whether any commit in the projected workspace is in a conflicted state.
    #[cfg(feature = "graph-workspace")]
    pub fn is_conflicted(&self) -> bool {
        use but_workspace::ui::workspace::DetailedGraphRowData;
        self.graph_workspace
            .stacks
            .iter()
            .flat_map(|stack| &stack.rows)
            .any(|row| {
                matches!(&row.data, DetailedGraphRowData::Commit(commit) if commit.has_conflicts)
            })
    }

    /// Build a [`WorkspaceState`] from an already-prepared overlayed graph.
    ///
    /// Use this when the caller already has a graph describing the workspace after the
    /// intended operation, regardless of whether that graph came from a preview, a
    /// materialized rebase, or another graph-producing workflow. The caller is
    /// responsible for supplying the matching `replaced_commits` map for that graph.
    ///
    /// `meta` is the ref-metadata matching `workspace`; the `graph-workspace`
    /// flavor needs it to compute the graph projection, the legacy flavor
    /// ignores it.
    ///
    /// This is the most direct constructor in this module and is the right choice when
    /// there is no need to inspect or materialize a [`SuccessfulRebase`].
    pub(crate) fn from_workspace<M: RefMetadata>(
        workspace: &but_graph::Workspace,
        meta: &mut M,
        repo: &gix::Repository,
        replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
        prs_by_head: &HashMap<String, usize>,
    ) -> anyhow::Result<WorkspaceState> {
        #[cfg(not(feature = "graph-workspace"))]
        {
            let _ = meta;
            let mut head_info = but_workspace::graph_to_ref_info(
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

            // Same pass the `head_info` read command runs, so mutation responses
            // (which Lite renders directly) carry cache-derived PR associations.
            head_info.apply_forge_pr_associations(repo, prs_by_head);

            Ok(WorkspaceState {
                replaced_commits,
                head_info,
            })
        }
        #[cfg(feature = "graph-workspace")]
        {
            // The graph_workspace projection needs its own equivalent enrichment;
            // that is out of scope here.
            let _ = prs_by_head;
            let mut workspace = workspace.clone();
            let graph_workspace =
                but_workspace::workspace::detailed_graph_workspace(&mut workspace, meta, repo)?;

            Ok(WorkspaceState {
                replaced_commits,
                graph_workspace: graph_workspace.into(),
            })
        }
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
        rebase: &mut SuccessfulRebase<'_, '_, M>,
        replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
        prs_by_head: &HashMap<String, usize>,
    ) -> anyhow::Result<WorkspaceState> {
        let workspace = rebase.overlayed_graph()?.into_workspace()?;
        let (repo, meta) = rebase.repo_and_meta_mut();
        Self::from_workspace(&workspace, meta, repo, replaced_commits, prs_by_head)
    }

    /// Build a [`WorkspaceState`] from an already-materialized rebase.
    ///
    /// Use this when the caller needs to perform additional bookkeeping after materialization
    /// before constructing the final workspace state.
    pub fn from_materialized_rebase<M: RefMetadata>(
        materialized: MaterializeOutcome<'_, '_, M>,
        repo: &gix::Repository,
        prs_by_head: &HashMap<String, usize>,
    ) -> anyhow::Result<WorkspaceState> {
        Self::from_workspace(
            materialized.workspace,
            materialized.meta,
            repo,
            materialized.history.commit_mappings(),
            prs_by_head,
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
        prs_by_head: &HashMap<String, usize>,
    ) -> anyhow::Result<WorkspaceState> {
        if dry_run.into() {
            let mut rebase = rebase;
            let replaced_commits = rebase.history.commit_mappings();
            return Self::from_rebase_preview(&mut rebase, replaced_commits, prs_by_head);
        }

        let materialized = rebase.materialize()?;
        Self::from_workspace(
            materialized.workspace,
            materialized.meta,
            repo,
            materialized.history.commit_mappings(),
            prs_by_head,
        )
    }
}
