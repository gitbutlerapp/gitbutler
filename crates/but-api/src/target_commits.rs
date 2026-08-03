//! Listing the target branch's history relative to the workspace.

use std::collections::{HashMap, HashSet};

use crate::json::HexHash;
use crate::workspace::target_branch_name;
use but_api_macros::but_api;
use but_forge::ForgeReview;
use tracing::{instrument, warn};

/// The target-commit listing stops after this many commits so degenerate
/// histories cannot produce unbounded responses. Clients place fork points
/// they cannot find in the clipped list at the end of their display.
const TARGET_COMMITS_LIMIT: usize = 1000;

/// The page size for cursor-based continuation of the target-commit listing,
/// when the caller does not specify one.
const TARGET_COMMITS_PAGE_SIZE: usize = 50;

/// List the target branch's first-parent commits from its tip down to the
/// workspace lower bound, ordered from newest to oldest.
///
/// Commits flagged as in the workspace are already reachable from it; the
/// remaining prefix is what an upstream integration would bring in. The
/// in-workspace tail runs to the fork point shared by all workspace stacks,
/// so clients can position each stack against the target history. The
/// first-parent walk shows a merge as a single commit rather than everything
/// it merged. The list is empty when the workspace has no target reference.
///
/// With `from`, the walk instead continues past the end of a previous
/// response: it starts below the given commit and returns up to `limit`
/// commits, allowing clients to page through target history older than the
/// workspace's fork point.
///
/// Each commit is enriched with the cached merged review (PR/MR) it landed,
/// when the forge cache recorded that commit as the review's integration
/// commit. This covers merge, squash, and rebase integrations to the extent
/// the forge reports them; it reads only the local cache and performs no
/// network requests or diffs, and enrichment failures degrade to unannotated
/// commits.
#[but_api(napi, json::TargetCommitPage)]
#[instrument(err(Debug))]
pub fn workspace_target_commits(
    ctx: &but_ctx::Context,
    from: Option<HexHash>,
    limit: Option<u32>,
) -> anyhow::Result<TargetCommitPage> {
    let (_guard, repo, ws, db) = ctx.workspace_and_db()?;
    let Some(target_ref) = ws.target_ref.as_ref() else {
        return Ok(TargetCommitPage::default());
    };
    let paging = from.is_some();
    let limit = limit
        .map(|limit| limit as usize)
        .unwrap_or(if paging {
            TARGET_COMMITS_PAGE_SIZE
        } else {
            TARGET_COMMITS_LIMIT
        })
        .min(TARGET_COMMITS_LIMIT);

    let mut cursor = match from {
        // Continue below the cursor commit.
        Some(HexHash(from)) => repo.find_commit(from)?.decode()?.parents().next(),
        None => Some(
            repo.find_reference(target_ref.ref_name.as_ref())?
                .peel_to_commit()?
                .id,
        ),
    };
    if limit == 0 {
        return Ok(TargetCommitPage {
            commits: Vec::new(),
            has_more: cursor.is_some(),
        });
    }

    let incoming: HashSet<_> = ws.incoming_target_commit_ids()?.into_iter().collect();
    let project_meta = ctx.project_meta()?;
    let mut reviews_by_integration_sha =
        match merged_reviews_by_integration_sha(&ws, &project_meta, &db) {
            Ok(reviews) => reviews,
            Err(err) => {
                warn!(
                    ?err,
                    "failed to read cached forge reviews; listing target commits without them"
                );
                HashMap::new()
            }
        };

    let natural_end = match (paging, cursor) {
        (false, Some(tip)) => natural_end_of_line(&repo, &ws, tip)?,
        _ => None,
    };
    let mut commits = Vec::new();
    while commits.len() < limit
        && let Some(id) = cursor
    {
        let in_workspace = !incoming.contains(&id);
        // Without a lower bound there is no shared history to walk into.
        if !paging && in_workspace && ws.lower_bound.is_none() {
            cursor = None;
            break;
        }
        let commit = repo.find_commit(id)?;
        cursor = commit
            .parent_ids()
            .next()
            .map(|parent_id| parent_id.detach());
        commits.push(TargetCommit {
            commit: commit.try_into()?,
            review: reviews_by_integration_sha.remove(&id),
            in_workspace,
        });
        if !paging
            && (natural_end == Some(id) || (natural_end.is_none() && ws.lower_bound == Some(id)))
        {
            cursor = None;
            break;
        }
    }
    Ok(TargetCommitPage {
        commits,
        has_more: cursor.is_some(),
    })
}

/// The first-parent line commit at which the base listing naturally ends: the
/// deepest fork point among the workspace's stack bases and the lower bound.
///
/// The lower bound is usually itself a line commit, but when a still-applied
/// stack was integrated through a merge commit, the bound (and that stack's
/// base) is the stack's own commit on the merge's *second* parent, which the
/// first-parent walk never meets. Such bounds resolve by following their own
/// first-parent chain down to where it rejoins the line — the stack's true
/// fork point. Bounds that fail to resolve within [`TARGET_COMMITS_LIMIT`]
/// are ignored; their stacks have no fork point in the listing either way.
fn natural_end_of_line(
    repo: &gix::Repository,
    ws: &but_graph::Workspace,
    target_tip: gix::ObjectId,
) -> anyhow::Result<Option<gix::ObjectId>> {
    let mut line = Vec::new();
    let mut position_by_id = HashMap::new();
    let mut cursor = Some(target_tip);
    while let Some(id) = cursor
        && line.len() < TARGET_COMMITS_LIMIT
    {
        position_by_id.insert(id, line.len());
        line.push(id);
        cursor = repo
            .find_commit(id)?
            .parent_ids()
            .next()
            .map(|id| id.detach());
    }

    let mut end: Option<usize> = None;
    let bounds = ws
        .stacks
        .iter()
        .filter_map(|stack| stack.base())
        .chain(ws.lower_bound);
    for bound in bounds {
        let mut cursor = Some(bound);
        let mut steps = 0;
        while let Some(id) = cursor
            && steps < TARGET_COMMITS_LIMIT
        {
            if let Some(&position) = position_by_id.get(&id) {
                end = Some(end.map_or(position, |deepest| deepest.max(position)));
                break;
            }
            cursor = repo
                .find_commit(id)?
                .parent_ids()
                .next()
                .map(|id| id.detach());
            steps += 1;
        }
    }
    Ok(end.map(|position| line[position]))
}

/// One bounded page of target commits and the state needed to continue it.
#[derive(Debug, Default)]
pub struct TargetCommitPage {
    /// The commits in this page, newest first.
    pub commits: Vec<TargetCommit>,
    /// Whether the relative walk was clipped before its natural bound.
    pub has_more: bool,
}

/// A commit on the target branch's first-parent line, its relation to the
/// workspace, and the cached merged review it landed, if known.
#[derive(Debug)]
pub struct TargetCommit {
    /// The commit itself.
    pub commit: but_workspace::ui::UpstreamCommit,
    /// The merged review this commit integrated, according to the forge cache.
    pub review: Option<ForgeReview>,
    /// Whether the commit is already reachable from the workspace.
    pub in_workspace: bool,
}

/// Merged reviews targeting the workspace's target branch, keyed by the target
/// commits the forge recorded as having landed them.
///
/// Keyed by `ObjectId` so the commit walk looks each up without re-encoding an
/// id to hex per commit; the few review SHAs are parsed once here instead.
fn merged_reviews_by_integration_sha(
    ws: &but_graph::Workspace,
    project_meta: &but_core::ref_metadata::ProjectMeta,
    db: &but_db::DbHandle,
) -> anyhow::Result<HashMap<gix::ObjectId, ForgeReview>> {
    let Some(target_branch_name) =
        target_branch_name(&ws.graph.symbolic_remote_names, project_meta)
    else {
        return Ok(HashMap::new());
    };

    let mut reviews_by_sha = HashMap::new();
    for review in but_forge::list_cached_forge_reviews(db)? {
        if !review.is_merged() || review.target_branch != target_branch_name {
            continue;
        }
        for sha in &review.integration_commit_shas {
            if let Ok(id) = gix::ObjectId::from_hex(sha.as_bytes()) {
                reviews_by_sha.insert(id, review.clone());
            }
        }
    }
    Ok(reviews_by_sha)
}

/// JSON transport types for the target-commit listing.
pub mod json {
    use serde::Serialize;

    /// JSON transport type for the cached merged review attached to a
    /// target commit.
    ///
    /// Only what the target-commit listing displays; the full review is
    /// available from the per-review APIs. `sourceBranch` is included so
    /// clients can match workspace branches to the commit that landed them.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct TargetCommitReview {
        /// The number identifying the review within its repository, e.g. `123`.
        pub number: i64,
        /// The title of the review.
        pub title: String,
        /// The URL to view the review in a web browser.
        pub html_url: String,
        /// The forge's symbol for this review type, e.g. `#` for GitHub pull
        /// requests and `!` for GitLab merge requests. Precedes `number` when
        /// displayed.
        pub unit_symbol: String,
        /// The short name of the branch the review proposed, e.g. `feature-branch`.
        pub source_branch: String,
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(TargetCommitReview);

    impl From<but_forge::ForgeReview> for TargetCommitReview {
        fn from(value: but_forge::ForgeReview) -> Self {
            Self {
                number: value.number,
                title: value.title,
                html_url: value.html_url,
                unit_symbol: value.unit_symbol,
                source_branch: value.source_branch,
            }
        }
    }

    /// JSON transport type for a commit on the target branch's first-parent line.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct TargetCommit {
        /// The commit itself.
        pub commit: but_workspace::ui::UpstreamCommit,
        /// The merged review this commit integrated, if the forge cache knows it.
        pub review: Option<TargetCommitReview>,
        /// Whether the commit is already reachable from the workspace.
        pub in_workspace: bool,
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(TargetCommit);

    impl From<super::TargetCommit> for TargetCommit {
        fn from(value: super::TargetCommit) -> Self {
            Self {
                commit: value.commit,
                review: value.review.map(Into::into),
                in_workspace: value.in_workspace,
            }
        }
    }

    /// A bounded page from the target branch's first-parent history.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct TargetCommitPage {
        /// The commits in this page, newest first.
        pub commits: Vec<TargetCommit>,
        /// Whether the relative walk was clipped before its natural bound.
        pub has_more: bool,
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(TargetCommitPage);

    impl From<super::TargetCommitPage> for TargetCommitPage {
        fn from(value: super::TargetCommitPage) -> Self {
            Self {
                commits: value.commits.into_iter().map(Into::into).collect(),
                has_more: value.has_more,
            }
        }
    }
}
