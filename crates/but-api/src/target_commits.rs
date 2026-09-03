//! Listing the target branch's history relative to the workspace.

use std::collections::{HashMap, HashSet};

use crate::json::HexHash;
use crate::workspace::target_branch_name;
use bstr::ByteSlice;
use but_api_macros::but_api;
use but_forge::ForgeReview;
use serde::Serialize;
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
/// Each commit is enriched with the merged review (PR/MR) it landed, when
/// the forge cache recorded that commit as the review's integration commit
/// or, failing that, when the commit's own message names it (see
/// [`but_forge::merged_review_from_message`]). Enrichment reads only local
/// state and performs no network requests or diffs, and its failures degrade
/// to unannotated commits.
#[but_api(napi, provides = [TargetCommits])]
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
        Some(HexHash(from)) => present_first_parent(&repo.find_commit(from)?),
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
    // Review enrichment is best-effort: either source failing leaves the
    // commits unannotated rather than failing the listing.
    let mut reviews_by_integration_sha = merged_reviews_by_integration_sha(&ws, &project_meta, &db)
        .inspect_err(|err| warn!(?err, "failed to read cached forge reviews"))
        .unwrap_or_default();
    let forge_info = target_forge_info(&project_meta, &repo)
        .inspect_err(|err| warn!(?err, "failed to determine the target's forge"))
        .unwrap_or_default();

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
        cursor = present_first_parent(&commit);
        let commit: but_workspace::ui::UpstreamCommit = commit.try_into()?;
        let review = reviews_by_integration_sha
            .remove(&id)
            .or_else(|| TargetCommitReview::from_message(&commit, forge_info.as_ref()?));
        commits.push(TargetCommit {
            commit,
            review,
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

/// The first parent of `commit`, or `None` at a root commit or when the parent
/// object is not present locally, as below the boundary of a shallow clone.
fn present_first_parent(commit: &gix::Commit<'_>) -> Option<gix::ObjectId> {
    commit
        .parent_ids()
        .next()
        .map(|id| id.detach())
        .filter(|id| commit.repo.has_object(id))
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
        cursor = present_first_parent(&repo.find_commit(id)?);
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
            cursor = present_first_parent(&repo.find_commit(id)?);
            steps += 1;
        }
    }
    Ok(end.map(|position| line[position]))
}

/// One bounded page of target commits and the state needed to continue it.
#[derive(Debug, Default, Serialize)]
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

/// A commit on the target branch's first-parent line, its relation to the
/// workspace, and the merged review it landed, if known.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct TargetCommit {
    /// The commit itself.
    pub commit: but_workspace::ui::UpstreamCommit,
    /// The merged review this commit integrated, if known.
    pub review: Option<TargetCommitReview>,
    /// Whether the commit is already reachable from the workspace.
    pub in_workspace: bool,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(TargetCommit);

/// The merged review a target commit landed.
///
/// Only what the target-commit listing displays; the full review is
/// available from the per-review APIs. The source branch lets clients match
/// workspace branches to the commit that landed them; it is empty when
/// unknown.
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

impl From<ForgeReview> for TargetCommitReview {
    fn from(value: ForgeReview) -> Self {
        Self {
            number: value.number,
            title: value.title,
            html_url: value.html_url,
            unit_symbol: value.unit_symbol,
            source_branch: value.source_branch,
        }
    }
}

impl TargetCommitReview {
    /// The review the commit's own message says it merged, for commits the
    /// forge cache has no record of; see [`but_forge::merged_review_from_message`].
    fn from_message(
        commit: &but_workspace::ui::UpstreamCommit,
        forge_info: &but_forge::ForgeInfo,
    ) -> Option<Self> {
        let message = commit.message.to_str().ok()?;
        let review = but_forge::merged_review_from_message(&forge_info.name, message)?;
        Some(Self {
            number: review.number,
            title: review.title.to_owned(),
            html_url: format!(
                "{}{}{}",
                forge_info.base_url, forge_info.pr_url_path, review.number
            ),
            unit_symbol: forge_info.unit.symbol.clone(),
            source_branch: review.source_branch.unwrap_or_default().to_owned(),
        })
    }
}

/// The forge the target remote points at, when known.
fn target_forge_info(
    project_meta: &but_core::ref_metadata::ProjectMeta,
    repo: &gix::Repository,
) -> anyhow::Result<Option<but_forge::ForgeInfo>> {
    let remote_url = project_meta.remote_url_with_fallback(repo)?;
    // Accounts only refine custom hosts; the forge itself is known without them.
    let accounts = but_forge::get_all_forge_accounts()
        .inspect_err(|err| warn!(?err, "failed to load forge accounts"))
        .unwrap_or_default();
    Ok(but_forge::forge_info(&remote_url, &accounts))
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
) -> anyhow::Result<HashMap<gix::ObjectId, TargetCommitReview>> {
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
                reviews_by_sha.insert(id, review.clone().into());
            }
        }
    }
    Ok(reviews_by_sha)
}
