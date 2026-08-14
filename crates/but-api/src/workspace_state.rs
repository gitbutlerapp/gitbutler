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
pub(crate) fn forge_prs_by_head(db: &but_db::DbHandle) -> anyhow::Result<HashMap<String, usize>> {
    but_forge::pr_numbers_by_head(db)
}

impl WorkspaceState {
    /// Map each projected local reference to whether its commits contain conflicts.
    #[cfg(not(feature = "graph-workspace"))]
    pub fn conflicts_by_reference(&self) -> HashMap<Vec<u8>, bool> {
        self.head_info
            .stacks
            .iter()
            .flat_map(|stack| &stack.segments)
            .filter_map(|segment| {
                let ref_info = segment.ref_info.as_ref()?;
                Some((
                    ref_info.ref_name.as_bstr().to_vec(),
                    segment.commits.iter().any(|commit| commit.has_conflicts),
                ))
            })
            .collect()
    }

    /// Map each projected local reference to whether its commits contain conflicts.
    #[cfg(feature = "graph-workspace")]
    pub fn conflicts_by_reference(&self) -> HashMap<Vec<u8>, bool> {
        use but_workspace::ui::workspace::DetailedGraphRowData;

        self.graph_workspace
            .stacks
            .iter()
            .flat_map(|stack| {
                stack.reference_segments.iter().filter_map(|segment| {
                    let DetailedGraphRowData::Reference(reference) =
                        &stack.rows.get(segment.reference_idx)?.data
                    else {
                        return None;
                    };
                    let has_conflicts = segment.row_idxs.iter().any(|&row_idx| {
                        matches!(
                            stack.rows.get(row_idx).map(|row| &row.data),
                            Some(DetailedGraphRowData::Commit(commit)) if commit.has_conflicts
                        )
                    });
                    Some((reference.ref_name.full_name_bytes.to_vec(), has_conflicts))
                })
            })
            .collect()
    }

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
    fn from_workspace_with_prs<M: RefMetadata>(
        workspace: &but_graph::Workspace,
        meta: &mut M,
        repo: &gix::Repository,
        replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
        prs_by_head: &HashMap<String, usize>,
        db: &mut but_db::DbHandle,
        checkout_conflict_occurred: bool,
    ) -> anyhow::Result<WorkspaceState> {
        #[cfg(not(feature = "graph-workspace"))]
        {
            let _ = (meta, db);
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
            head_info.apply_forge_review_associations(repo, prs_by_head);

            Ok(WorkspaceState {
                replaced_commits,
                head_info,
                checkout_conflict_occurred,
            })
        }
        #[cfg(feature = "graph-workspace")]
        {
            // The graph_workspace projection needs its own equivalent enrichment;
            // that is out of scope here.
            let _ = prs_by_head;
            let mut workspace = workspace.clone();
            let graph_workspace =
                but_workspace::workspace::detailed_graph_workspace(&mut workspace, meta, repo, db)?;

            Ok(WorkspaceState {
                replaced_commits,
                graph_workspace: graph_workspace.into(),
                checkout_conflict_occurred,
            })
        }
    }

    /// Build a [`WorkspaceState`] from an already-prepared overlayed graph.
    ///
    /// This is the API-facing constructor for callers that already hold the
    /// workspace cache DB. It derives PR associations from the forge review
    /// cache before projecting the workspace state.
    ///
    /// It reports `checkout_conflict_occurred: false`, which is only true of a workspace that
    /// was never checked out. Use [`Self::from_materialized`] after a materialize with checkout.
    pub fn from_workspace_with_db<M: RefMetadata>(
        workspace: &but_graph::Workspace,
        meta: &mut M,
        repo: &gix::Repository,
        replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
        db: &mut but_db::DbHandle,
    ) -> anyhow::Result<WorkspaceState> {
        let prs_by_head = forge_prs_by_head(db)?;
        Self::from_workspace_with_prs(
            workspace,
            meta,
            repo,
            replaced_commits,
            &prs_by_head,
            db,
            false,
        )
    }

    /// Build a preview [`WorkspaceState`] from a successful rebase without materializing it.
    ///
    /// Use this when the caller needs to report the post-rebase workspace layout before
    /// writing the rebase result back to the repository, such as dry-run flows or
    /// operations that intentionally preview the outcome first and materialize later.
    ///
    /// The `replaced_commits` map should describe the commit rewrites visible in the
    /// preview graph, which typically comes from `rebase.history.commit_mappings()`.
    fn from_rebase_preview_with_prs<M: RefMetadata>(
        rebase: &mut SuccessfulRebase<'_, '_, M>,
        replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
        prs_by_head: &HashMap<String, usize>,
    ) -> anyhow::Result<WorkspaceState> {
        let workspace = rebase.overlayed_graph()?.into_workspace()?;
        let (repo, meta, db) = rebase.repo_meta_and_db_mut();
        Self::from_workspace_with_prs(
            &workspace,
            meta,
            repo,
            replaced_commits,
            prs_by_head,
            db,
            false,
        )
    }

    /// Build a preview [`WorkspaceState`] from a successful rebase without materializing it.
    ///
    /// This is the API-facing preview constructor: it reads PR associations from
    /// the forge review cache through the database handle the rebase carries,
    /// before projecting the preview state.
    pub(crate) fn from_rebase_preview<M: RefMetadata>(
        rebase: &mut SuccessfulRebase<'_, '_, M>,
        replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
    ) -> anyhow::Result<WorkspaceState> {
        let prs_by_head = forge_prs_by_head(rebase.db())?;
        Self::from_rebase_preview_with_prs(rebase, replaced_commits, &prs_by_head)
    }

    /// Build a [`WorkspaceState`] from a materialized rebase.
    ///
    /// This takes the whole [`MaterializeOutcome`] rather than its parts so that everything the
    /// projection reports — the workspace, the commit mappings, the PR associations, and whether
    /// the checkout conflicted — is read from one place. A caller cannot forget to pass one on,
    /// which is the failure mode [`Self::from_workspace_with_db`] invites for materialized work.
    pub fn from_materialized<M: RefMetadata>(
        materialized: MaterializeOutcome<'_, '_, M>,
        repo: &gix::Repository,
    ) -> anyhow::Result<WorkspaceState> {
        let prs_by_head = forge_prs_by_head(materialized.db)?;
        Self::from_workspace_with_prs(
            materialized.workspace,
            materialized.meta,
            repo,
            materialized.history.commit_mappings(),
            &prs_by_head,
            materialized.db,
            materialized.checkout_conflict_occurred,
        )
    }

    /// Build a [`WorkspaceState`] from a successful rebase, materializing it when needed.
    ///
    /// Use this as the default entry point when an operation ends with a [`SuccessfulRebase`] and
    /// the API should return the resulting workspace state. When `dry_run` is `true`, it projects
    /// the overlayed graph so the caller sees the outcome without changing the repository.
    /// Otherwise it materializes the rebase, then reports the workspace state together with the
    /// final commit-replacement mappings returned by the materialized history.
    ///
    /// PR associations come from the forge review cache, read through the database handle the
    /// rebase carries.
    pub fn from_successful_rebase<M: RefMetadata>(
        rebase: SuccessfulRebase<'_, '_, M>,
        repo: &gix::Repository,
        dry_run: DryRun,
    ) -> anyhow::Result<WorkspaceState> {
        if dry_run.into() {
            let mut rebase = rebase;
            let prs_by_head = forge_prs_by_head(rebase.db())?;
            let replaced_commits = rebase.history.commit_mappings();
            return Self::from_rebase_preview_with_prs(&mut rebase, replaced_commits, &prs_by_head);
        }

        Self::from_materialized(rebase.materialize(Default::default())?, repo)
    }
}
