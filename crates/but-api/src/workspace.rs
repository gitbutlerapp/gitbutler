//! Functions that operate on the workspace.

use std::{borrow::Cow, collections::HashSet};

use crate::WorkspaceState;
use but_api_macros::but_api;
use but_core::{
    DryRun, RefMetadata, extract_remote_name_and_short_name, is_workspace_ref_name,
    sync::RepoExclusive,
};
use but_forge::ForgeReview;
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_serde::BStringForFrontend;
use but_workspace::{IntegrateUpstreamOutcome, ReviewIntegrationHint};
use tracing::{instrument, warn};

/// Result of integrating upstream changes into the current workspace.
#[derive(Debug, Clone)]
pub struct WorkspaceIntegrateUpstreamOutcome {
    /// The post-operation or preview workspace state.
    pub workspace_state: WorkspaceState,
    /// Dirty worktree paths that would conflict when applied onto the resulting workspace head.
    pub worktree_conflicts: Vec<BStringForFrontend>,
}

/// JSON transport types for workspace APIs.
pub mod json {
    use but_serde::BStringForFrontend;
    use serde::{Deserialize, Serialize};

    /// JSON transport type for how a stack bottom should be updated.
    #[derive(Debug, Clone, Copy, Serialize, Deserialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub enum BottomUpdateKind {
        /// Rebase the selected bottom-most commit onto the target branch.
        Rebase,
        /// Create a merge commit at the top of the selected stack.
        Merge,
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(BottomUpdateKind);

    impl From<BottomUpdateKind> for but_workspace::BottomUpdateKind {
        fn from(value: BottomUpdateKind) -> Self {
            match value {
                BottomUpdateKind::Rebase => Self::Rebase,
                BottomUpdateKind::Merge => Self::Merge,
            }
        }
    }

    /// JSON transport type describing one stack bottom to update.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct BottomUpdate {
        /// How the selected stack bottom should be updated.
        pub kind: BottomUpdateKind,
        /// The bottom-most commit or empty bottom reference to update.
        pub selector: crate::commit::json::RelativeTo,
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(BottomUpdate);

    impl From<BottomUpdate> for but_workspace::BottomUpdate {
        fn from(value: BottomUpdate) -> Self {
            let BottomUpdate { kind, selector } = value;
            Self {
                kind: kind.into(),
                selector: selector.into(),
            }
        }
    }

    /// JSON transport type returned by upstream integration.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct WorkspaceIntegrateUpstreamOutcome {
        /// The post-operation or preview workspace state.
        pub workspace_state: crate::json::WorkspaceState,
        /// Dirty worktree paths that would conflict when applied onto the resulting workspace head.
        #[cfg_attr(feature = "export-schema", schemars(with = "Vec<String>"))]
        pub worktree_conflicts: Vec<BStringForFrontend>,
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(WorkspaceIntegrateUpstreamOutcome);

    impl TryFrom<super::WorkspaceIntegrateUpstreamOutcome> for WorkspaceIntegrateUpstreamOutcome {
        type Error = anyhow::Error;

        fn try_from(value: super::WorkspaceIntegrateUpstreamOutcome) -> Result<Self, Self::Error> {
            Ok(Self {
                workspace_state: value.workspace_state.try_into()?,
                worktree_conflicts: value.worktree_conflicts,
            })
        }
    }
}

fn target_branch_name(
    symbolic_remote_names: &[String],
    project_meta: &but_core::ref_metadata::ProjectMeta,
) -> Option<String> {
    let target_ref = project_meta.target_ref.as_ref()?;
    let mut symbolic_remote_names = symbolic_remote_names.iter().collect::<Vec<_>>();
    symbolic_remote_names.sort_by_key(|name| name.len());
    let remote_names = symbolic_remote_names
        .iter()
        .map(|name| Cow::Borrowed(name.as_str().into()))
        .collect();
    Some(
        extract_remote_name_and_short_name(target_ref.as_ref(), &remote_names)
            .map(|(_, short_name)| short_name.to_string())
            .unwrap_or_else(|| target_ref.shorten().to_string()),
    )
}

fn review_integration_hints_from_reviews(
    target_branch_name: &str,
    incoming_commit_ids: &HashSet<String>,
    reviews: impl IntoIterator<Item = ForgeReview>,
) -> Vec<ReviewIntegrationHint> {
    let mut seen = HashSet::new();

    reviews
        .into_iter()
        .filter(|review| {
            review.is_merged()
                && review.target_branch == target_branch_name
                && review
                    .integration_commit_shas
                    .iter()
                    .any(|sha| incoming_commit_ids.contains(sha))
        })
        .filter_map(|review| gix::ObjectId::from_hex(review.sha.as_bytes()).ok())
        .filter(|head_commit_at_merge| seen.insert(*head_commit_at_merge))
        .map(|head_commit_at_merge| ReviewIntegrationHint {
            head_commit_at_merge,
        })
        .collect()
}

fn forge_review_integration_hints(
    workspace: &but_graph::Workspace,
    project_meta: &but_core::ref_metadata::ProjectMeta,
    db: &but_db::DbHandle,
) -> anyhow::Result<Vec<ReviewIntegrationHint>> {
    let Some(target_branch_name) =
        target_branch_name(&workspace.graph.symbolic_remote_names, project_meta)
    else {
        return Ok(vec![]);
    };

    let incoming_commit_ids = workspace
        .incoming_target_commit_ids()?
        .into_iter()
        .map(|id| id.to_hex().to_string())
        .collect::<HashSet<_>>();
    if incoming_commit_ids.is_empty() {
        return Ok(vec![]);
    }

    let associated_reviews = db
        .forge_reviews()
        .list_all()?
        .into_iter()
        .map(but_forge::ForgeReview::try_from)
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(review_integration_hints_from_reviews(
        &target_branch_name,
        &incoming_commit_ids,
        associated_reviews,
    ))
}

/// Integrate upstream changes into the current workspace without recording an
/// oplog entry.
///
/// This acquires exclusive worktree access from `ctx`, applies the requested
/// upstream updates, and returns the resulting workspace state plus information about
/// worktree conflicts. When `dry_run`
/// is enabled, the returned workspace previews the integration without
/// materializing the rebase. See
/// [`workspace_integrate_upstream_only_with_perm()`] for lower-level details.
#[but_api(try_from = json::WorkspaceIntegrateUpstreamOutcome)]
#[instrument(err(Debug))]
pub fn workspace_integrate_upstream_only(
    ctx: &mut but_ctx::Context,
    updates: Vec<json::BottomUpdate>,
    dry_run: DryRun,
) -> anyhow::Result<WorkspaceIntegrateUpstreamOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    workspace_integrate_upstream_only_with_perm(
        ctx,
        updates.into_iter().map(Into::into).collect(),
        dry_run,
        guard.write_permission(),
    )
}

/// Integrate upstream changes into the current workspace and record an oplog
/// snapshot on success.
///
/// This acquires exclusive worktree access from `ctx`, applies the requested
/// upstream updates, and commits a best-effort `MergeUpstream` oplog snapshot
/// when the integration succeeds. When `dry_run` is enabled, the returned
/// workspace previews the integration and no oplog entry is persisted. See
/// [`workspace_integrate_upstream_with_perm()`] for lower-level details.
#[but_api(napi, try_from = json::WorkspaceIntegrateUpstreamOutcome)]
#[instrument(err(Debug))]
pub fn workspace_integrate_upstream(
    ctx: &mut but_ctx::Context,
    updates: Vec<json::BottomUpdate>,
    dry_run: DryRun,
) -> anyhow::Result<WorkspaceIntegrateUpstreamOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    workspace_integrate_upstream_with_perm(
        ctx,
        updates.into_iter().map(Into::into).collect(),
        dry_run,
        guard.write_permission(),
    )
}

/// Integrate upstream changes under caller-held exclusive repository access
/// and record an oplog snapshot on success.
///
/// It prepares a best-effort `MergeUpstream` oplog snapshot, performs the
/// integration, and commits the snapshot only if the mutation succeeds. When
/// `dry_run` is enabled, it returns a preview of the resulting workspace state
/// plus worktree conflicts and skips oplog persistence.
/// For lower-level implementation details, see [`but_workspace::integrate_upstream()`].
pub fn workspace_integrate_upstream_with_perm(
    ctx: &mut but_ctx::Context,
    updates: Vec<but_workspace::BottomUpdate>,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<WorkspaceIntegrateUpstreamOutcome> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(OperationKind::MergeUpstream),
        perm.read_permission(),
        dry_run,
    );

    let result = workspace_integrate_upstream_only_with_perm(ctx, updates, dry_run, perm);
    if let Some(snapshot) = maybe_oplog_entry
        && result.is_ok()
    {
        snapshot.commit(ctx, perm).ok();
    }

    result
}

/// Integrate upstream changes under caller-held exclusive repository access.
///
/// This delegates to [`but_workspace::integrate_upstream()`] and returns the
/// resulting workspace state plus worktree conflicts info.
/// When `dry_run` is enabled, it returns a preview of the resulting workspace
/// without materializing the rebase.
pub fn workspace_integrate_upstream_only_with_perm(
    ctx: &mut but_ctx::Context,
    updates: Vec<but_workspace::BottomUpdate>,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<WorkspaceIntegrateUpstreamOutcome> {
    let mut meta = ctx.meta()?;
    let (workspace_state, worktree_conflicts) = {
        let (repo, mut ws, db) = ctx.workspace_mut_and_db_with_perm(perm)?;
        let project_meta = ctx.project_meta()?;
        let review_hints = match forge_review_integration_hints(&ws, &project_meta, &db) {
            Ok(review_hints) => review_hints,
            Err(err) => {
                warn!(
                    ?err,
                    "failed to derive forge review integration hints; continuing without hints"
                );
                Vec::new()
            }
        };
        let IntegrateUpstreamOutcome {
            rebase,
            ws_meta,
            project_meta,
        } = but_workspace::integrate_upstream_with_hints(
            &mut ws,
            &mut meta,
            project_meta,
            &repo,
            updates,
            &review_hints,
        )?;
        let worktree_conflicts = but_workspace::worktree_conflicts_for_rebase(&rebase)?;

        if dry_run.into() {
            let workspace_state =
                WorkspaceState::from_rebase_preview(&rebase, rebase.history.commit_mappings())?;
            return Ok(WorkspaceIntegrateUpstreamOutcome {
                workspace_state,
                worktree_conflicts,
            });
        }

        let materialized = rebase.materialize()?;
        project_meta.persist_to_local_config(&repo)?;

        if let Some(ref_name) = materialized.workspace.ref_name()
            && let Some(ws_meta) = ws_meta
            && is_workspace_ref_name(ref_name)
        {
            let mut md = materialized.meta.workspace(ref_name)?;
            *md = ws_meta;
            md.set_project_meta(project_meta);
            materialized.meta.set_workspace(&md)?;
        }

        let workspace_state = WorkspaceState::from_workspace(
            materialized.workspace,
            &repo,
            materialized.history.commit_mappings(),
        )?;
        (workspace_state, worktree_conflicts)
    };
    ctx.invalidate_workspace_cache()?;

    Ok(WorkspaceIntegrateUpstreamOutcome {
        workspace_state,
        worktree_conflicts,
    })
}

#[cfg(test)]
mod tests {
    use super::{review_integration_hints_from_reviews, target_branch_name};
    use std::collections::HashSet;

    fn review(
        sha: &str,
        integration_commit_shas: &[&str],
        target_branch: &str,
        merged_at: Option<&str>,
    ) -> but_forge::ForgeReview {
        but_forge::ForgeReview {
            html_url: "https://example.test/review/1".into(),
            number: 1,
            title: "review".into(),
            body: None,
            author: None,
            labels: vec![],
            draft: false,
            source_branch: "feature".into(),
            target_branch: target_branch.into(),
            sha: sha.into(),
            integration_commit_shas: integration_commit_shas
                .iter()
                .map(|s| s.to_string())
                .collect(),
            created_at: None,
            modified_at: None,
            merged_at: merged_at.map(str::to_owned),
            closed_at: merged_at.map(str::to_owned),
            repository_ssh_url: None,
            repository_https_url: None,
            repo_owner: None,
            head_repo_is_fork: false,
            reviewers: vec![],
            unit_symbol: "#".into(),
            last_sync_at: Default::default(),
        }
    }

    #[test]
    fn target_branch_name_prefers_longest_matching_remote_name() {
        let project_meta = but_core::ref_metadata::ProjectMeta {
            target_ref: Some(
                "refs/remotes/foo/bar/main"
                    .try_into()
                    .expect("target ref is a valid full ref name"),
            ),
            ..Default::default()
        };
        let remote_names = vec!["foo".to_string(), "foo/bar".to_string()];

        assert_eq!(
            target_branch_name(&remote_names, &project_meta).as_deref(),
            Some("main"),
            "the longest matching remote name should be stripped from the target ref"
        );
    }

    #[test]
    fn review_hints_keep_only_merged_reviews_on_the_target_branch() {
        let hints = review_integration_hints_from_reviews(
            "main",
            &HashSet::from(["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()]),
            vec![
                review(
                    "1234567890abcdef1234567890abcdef12345678",
                    &["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],
                    "main",
                    Some("2026-06-24T12:00:00Z"),
                ),
                review(
                    "abcdef1234567890abcdef1234567890abcdef12",
                    &["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],
                    "release",
                    Some("2026-06-24T12:00:00Z"),
                ),
                review(
                    "fedcba9876543210fedcba9876543210fedcba98",
                    &["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],
                    "main",
                    None,
                ),
            ],
        );

        assert_eq!(
            hints.len(),
            1,
            "only merged reviews targeting the current branch produce hints"
        );
        assert_eq!(
            hints[0].head_commit_at_merge.to_hex().to_string(),
            "1234567890abcdef1234567890abcdef12345678",
            "the hint should use the review head SHA reported by the forge"
        );
    }

    #[test]
    fn review_hints_dedupe_duplicate_review_heads() {
        let hints = review_integration_hints_from_reviews(
            "main",
            &HashSet::from(["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()]),
            vec![
                review(
                    "1234567890abcdef1234567890abcdef12345678",
                    &["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],
                    "main",
                    Some("2026-06-24T12:00:00Z"),
                ),
                review(
                    "1234567890abcdef1234567890abcdef12345678",
                    &["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],
                    "main",
                    Some("2026-06-24T12:00:00Z"),
                ),
            ],
        );

        assert_eq!(
            hints.len(),
            1,
            "multiple incoming commits may map to the same merged review head"
        );
    }

    #[test]
    fn review_hints_ignore_reviews_without_matching_incoming_commit() {
        let hints = review_integration_hints_from_reviews(
            "main",
            &HashSet::from(["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()]),
            vec![review(
                "1234567890abcdef1234567890abcdef12345678",
                &["bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"],
                "main",
                Some("2026-06-24T12:00:00Z"),
            )],
        );

        assert!(
            hints.is_empty(),
            "cached review hints should only match reviews whose landing commit is among incoming upstream commits"
        );
    }
}
