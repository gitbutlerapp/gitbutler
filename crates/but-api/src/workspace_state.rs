use super::WorkspaceState;
use std::collections::{BTreeMap, HashMap};

use but_core::{DryRun, RefMetadata};
use but_rebase::graph_rebase::RebasedEditor;

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
        prs_by_head: &HashMap<String, but_forge::ReviewAssociation>,
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
                    project_meta: workspace.project_meta().clone(),
                    traversal: but_graph::walk::Options::limited(),
                    expensive_commit_info: true,
                    ..Default::default()
                },
            )?
            .pruned_to_entrypoint();

            // Same pass the `head_info` read command runs, so mutation
            // responses (which Lite renders directly) carry the same
            // associations: open reviews from the forge cache for active
            // branches, durable stored identity for integrated ones.
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
        let prs_by_head = but_forge::review_associations_by_head(db)?;
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

    /// Like [`Self::from_workspace_with_db`], for a workspace a materialization with checkout
    /// just refreshed: `checkout_conflict_occurred` reports how that checkout went.
    pub fn from_workspace_with_db_and_checkout<M: RefMetadata>(
        workspace: &but_graph::Workspace,
        meta: &mut M,
        repo: &gix::Repository,
        replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
        db: &mut but_db::DbHandle,
        checkout_conflict_occurred: bool,
    ) -> anyhow::Result<WorkspaceState> {
        let prs_by_head = but_forge::review_associations_by_head(db)?;
        Self::from_workspace_with_prs(
            workspace,
            meta,
            repo,
            replaced_commits,
            &prs_by_head,
            db,
            checkout_conflict_occurred,
        )
    }

    /// Build a preview [`WorkspaceState`] from a successful rebase without materializing it.
    ///
    /// Use this when the caller needs to report the post-rebase workspace layout before
    /// writing the rebase result back to the repository, such as dry-run flows or
    /// operations that intentionally preview the outcome first and materialize later.
    ///
    /// The `replaced_commits` map should describe the commit rewrites visible in the
    /// preview graph, which typically comes from `rebase.commit_mappings()`.
    fn from_rebase_preview_with_prs<M: RefMetadata>(
        workspace: &but_graph::Workspace,
        rebase: &mut RebasedEditor<'_, M>,
        replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
        prs_by_head: &HashMap<String, but_forge::ReviewAssociation>,
        db: &mut but_db::DbHandle,
    ) -> anyhow::Result<WorkspaceState> {
        let preview = but_workspace::workspace::overlayed_workspace(workspace, rebase)?;
        let (repo, meta) = rebase.repo_and_meta_mut();
        Self::from_workspace_with_prs(
            &preview,
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
    /// the forge review cache before projecting the preview state.
    pub(crate) fn from_rebase_preview_with_db<M: RefMetadata>(
        workspace: &but_graph::Workspace,
        rebase: &mut RebasedEditor<'_, M>,
        replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
        db: &mut but_db::DbHandle,
    ) -> anyhow::Result<WorkspaceState> {
        let prs_by_head = but_forge::review_associations_by_head(db)?;
        Self::from_rebase_preview_with_prs(workspace, rebase, replaced_commits, &prs_by_head, db)
    }

    /// Build a [`WorkspaceState`] from a successful rebase, materializing it when needed.
    ///
    /// Use this as the default entry point when an operation ends with a [`RebasedEditor`] and
    /// the API should return the resulting workspace state. When `dry_run` is `true`, it projects
    /// the overlayed graph so the caller sees the outcome without changing the repository.
    /// Otherwise it materializes the rebase, refreshes `workspace` from the result, and reports
    /// the workspace state together with the final commit-replacement mappings and whether the
    /// checkout conflicted — everything the projection reports is read from one place, so a
    /// caller cannot forget to pass one on.
    ///
    /// PR associations come from the forge review cache.
    pub fn from_successful_rebase_with_db<M: RefMetadata>(
        workspace: &mut but_graph::Workspace,
        rebase: RebasedEditor<'_, M>,
        repo: &gix::Repository,
        dry_run: DryRun,
        db: &mut but_db::DbHandle,
    ) -> anyhow::Result<WorkspaceState> {
        let prs_by_head = but_forge::review_associations_by_head(db)?;
        if dry_run.into() {
            let mut rebase = rebase;
            let replaced_commits = rebase.commit_mappings();
            return Self::from_rebase_preview_with_prs(
                workspace,
                &mut rebase,
                replaced_commits,
                &prs_by_head,
                db,
            );
        }

        let replaced_commits = rebase.commit_mappings();
        let materialized = rebase.materialize_with_outcome()?;
        workspace.refresh_from_commit_graph(
            materialized.commit_graph,
            repo,
            &*materialized.meta,
            db,
        )?;
        Self::from_workspace_with_prs(
            workspace,
            materialized.meta,
            repo,
            replaced_commits,
            &prs_by_head,
            db,
            materialized.checkout_conflict_occurred,
        )
    }
}
