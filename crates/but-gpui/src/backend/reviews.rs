//! Pull requests: what the forge cache knows about each branch's, what a new one would
//! say, publishing it, and merging one. Calls to the forge go over the network, so these
//! run off the UI thread like the rest.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use but_api::legacy::forge;
use but_ctx::Context;
use but_forge::ForgeName;

use super::ops::holds;
use super::upstream::sync_failure;
use super::{
    CheckRun, Checks, Forge, ForgeUser, PushState, Review, ReviewLabel, ReviewState, open, push,
    runtime, short_ref,
};

/// Which forge hosts the project: the one its target's remote, or a forge account with a
/// matching host, names, or else `overridden`, the one its settings pick.
fn forge_from(ctx: &Context, overridden: Option<ForgeName>) -> Option<ForgeName> {
    let meta = ctx.project_meta().ok()?;
    let repo = ctx.repo.get().ok()?;
    // A target branch with no upstream has no remote to name a forge.
    meta.remote_target_ref(&repo).ok()??;
    let url = forge::remote_url(&meta, &repo).ok()?;
    Some(but_forge::derive_forge_repo_info(&url, overridden)?.forge)
}

/// [`forge_from`] with the project's settings' pick.
pub(super) fn forge_name(ctx: &Context) -> Option<ForgeName> {
    forge_from(ctx, saved_forge(ctx))
}

/// The forge the project's settings pick, for when nothing else names one.
fn saved_forge(ctx: &Context) -> Option<ForgeName> {
    (ctx.legacy_project.forge_override.as_deref()).and_then(ForgeName::from_override_str)
}

/// The forge the project's remote or accounts name, and the one its settings pick.
pub(super) fn detected_and_saved_forge(ctx: &Context) -> (Option<Forge>, Option<Forge>) {
    (
        forge_from(ctx, None).map(app_forge),
        saved_forge(ctx).map(app_forge),
    )
}

/// `name` as the app names forges.
pub(super) fn app_forge(name: ForgeName) -> Forge {
    match name {
        ForgeName::GitHub => Forge::GitHub,
        ForgeName::GitLab => Forge::GitLab,
        ForgeName::Bitbucket => Forge::Bitbucket,
        ForgeName::Azure => Forge::Azure,
    }
}

/// The pull requests the forge cache knows about, by number. Reads only the cache;
/// [`refresh_reviews`] is what asks the forge.
pub(super) fn cached_reviews(ctx: &Context) -> HashMap<u64, but_forge::ForgeReview> {
    ctx.db
        .get_cache()
        .ok()
        .and_then(|db| but_forge::list_cached_forge_reviews(&db).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|review| (review.number as u64, review))
        .collect()
}

/// Pull request `number` of the branch `name`, with what `cached` says about it and the CI
/// the cache has for the branch.
pub(super) fn review(
    ctx: &Context,
    cached: &HashMap<u64, but_forge::ForgeReview>,
    number: u64,
    name: &str,
    push: PushState,
) -> Review {
    let cache_only = Some(but_forge::CacheConfig::CacheOnly);
    let checks = forge::list_ci_checks_for_ref(ctx, name, cache_only).unwrap_or_default();
    let mut review = match cached.get(&number) {
        Some(found) => from_cache(found),
        None => Review {
            number,
            ..Review::default()
        },
    };
    // A merged pull request drops out of the open ones the forge lists.
    if push == PushState::Integrated {
        review.state = ReviewState::Merged;
    }
    review.checks = (!checks.is_empty()).then(|| summarize_checks(&checks));
    review.check_runs = check_runs(&checks);
    review
}

/// A pull request as the forge cache has it, without its CI checks.
pub(super) fn from_cache(found: &but_forge::ForgeReview) -> Review {
    Review {
        number: found.number as u64,
        title: found.title.clone(),
        state: review_state(found),
        url: Some(found.html_url.clone()),
        auto_merge: found.auto_merge_enabled,
        checks: None,
        check_runs: Vec::new(),
        labels: (found.labels.iter())
            .map(|label| ReviewLabel {
                name: label.name.clone(),
                // GitHub sends "d73a4a", others may send "#d73a4a".
                color: (label.color.as_deref())
                    .and_then(|hex| u32::from_str_radix(hex.trim_start_matches('#'), 16).ok()),
            })
            .collect(),
        reviewers: (found.reviewers.iter())
            .filter(|user| !user.is_bot)
            .map(forge_user)
            .collect(),
        author: found.author.as_ref().map(forge_user),
        source_branch: found.source_branch.clone(),
        target_branch: found.target_branch.clone(),
        body: found.body.clone().unwrap_or_default(),
        created_at: timestamp(found.created_at.as_deref()),
        updated_at: timestamp(found.modified_at.as_deref()),
        synced_at: (found
            .last_sync_at
            .and_local_timezone(chrono::Local)
            .single())
        .map(|at| at.timestamp()),
    }
}

fn forge_user(user: &but_forge::ForgeReviewUser) -> ForgeUser {
    ForgeUser {
        login: user.login.clone(),
        avatar_url: user.avatar_url.clone(),
    }
}

/// An RFC 3339 time from the forge, in seconds since the epoch.
fn timestamp(at: Option<&str>) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(at?)
        .ok()
        .map(|at| at.timestamp())
}

/// Each check, those that failed first, then those still running, then the rest, each
/// group in the order the forge lists them.
fn check_runs(checks: &[but_forge::CiCheck]) -> Vec<CheckRun> {
    use but_forge::{CiConclusion as C, CiStatus as S};
    let mut runs: Vec<_> = (checks.iter())
        .map(|check| CheckRun {
            name: check.name.clone(),
            url: match check.html_url.as_str() {
                "" => check.details_url.clone(),
                url => url.to_owned(),
            },
            failed: check_failed(check),
            status: match &check.status {
                S::Complete { conclusion, .. } => match conclusion {
                    C::Success => "Succeeded",
                    C::Neutral => "Neutral",
                    C::Skipped => "Skipped",
                    C::Failure => "Failed",
                    C::TimedOut => "Timed out",
                    C::Cancelled => "Cancelled",
                    C::ActionRequired => "Action required",
                    C::Unknown => "Finished",
                },
                S::InProgress => "In progress",
                S::Queued => "Queued",
                S::Unknown => "Waiting",
            }
            .to_owned(),
            started_at: check.started_at.map(|at| at.timestamp()),
            completed_at: match &check.status {
                S::Complete { completed_at, .. } => completed_at.map(|at| at.timestamp()),
                S::InProgress | S::Queued | S::Unknown => None,
            },
        })
        .collect();
    runs.sort_by_key(|run| match run.failed {
        Some(true) => 0,
        None => 1,
        Some(false) => 2,
    });
    runs
}

pub(super) fn review_state(review: &but_forge::ForgeReview) -> ReviewState {
    match (&review.merged_at, &review.closed_at, review.draft) {
        (Some(_), _, _) => ReviewState::Merged,
        (None, Some(_), _) => ReviewState::Closed,
        (None, None, true) => ReviewState::Draft,
        (None, None, false) => ReviewState::Open,
    }
}

fn summarize_checks(checks: &[but_forge::CiCheck]) -> Checks {
    checks.iter().fold(Checks::default(), |mut sum, check| {
        match check_failed(check) {
            Some(false) => sum.passed += 1,
            Some(true) => sum.failed += 1,
            None => sum.running += 1,
        }
        sum
    })
}

/// Whether a check that finished failed; `None` while it hasn't.
fn check_failed(check: &but_forge::CiCheck) -> Option<bool> {
    use but_forge::{CiConclusion as C, CiStatus as S};
    match &check.status {
        S::Complete { conclusion, .. } => Some(match conclusion {
            C::Success | C::Neutral | C::Skipped => false,
            C::Failure | C::TimedOut | C::Cancelled | C::ActionRequired | C::Unknown => true,
        }),
        S::InProgress | S::Queued | S::Unknown => None,
    }
}

/// Ask the forge about the project's pull requests, refreshing the cache that
/// [`load_workspace`](super::load_workspace) reads, and say whether that changed it. Fails
/// when the project has no forge or no account for it.
pub fn refresh_reviews(path: &Path) -> Result<bool> {
    let started = std::time::Instant::now();
    tracing::debug!(path = %path.display(), "refreshing reviews");
    let ctx = open(path)?;
    // Without when each was last synced, which every refresh changes.
    let cached = || {
        let mut reviews: Vec<_> = (cached_reviews(&ctx).into_values())
            .map(|review| but_forge::ForgeReview {
                last_sync_at: Default::default(),
                ..review
            })
            .collect();
        reviews.sort_by_key(|review| review.number);
        serde_json::to_string(&reviews).unwrap_or_default()
    };
    let before = cached();
    forge::list_reviews(&ctx, Some(but_forge::CacheConfig::NoCache))?;
    tracing::debug!(elapsed = ?started.elapsed(), path = %path.display(), "refreshed reviews");
    Ok(cached() != before)
}

/// Ask the forge about the CI of each branch with a pull request, a request per branch, and
/// say whether that changed what the cache has.
pub fn refresh_checks(path: &Path) -> Result<bool> {
    let started = std::time::Instant::now();
    let ctx = open(path)?;
    // Serializing a check leaves out when it was last synced.
    let cached = || {
        let mut refs = (ctx.db.get_cache().ok())
            .and_then(|db| db.ci_checks().list_all_references().ok())
            .unwrap_or_default();
        refs.sort();
        let checks: Vec<_> = (refs.iter())
            .map(|name| {
                let cache_only = Some(but_forge::CacheConfig::CacheOnly);
                let mut checks =
                    forge::list_ci_checks_for_ref(&ctx, name, cache_only).unwrap_or_default();
                checks.sort_by_key(|check| check.id);
                (name, checks)
            })
            .collect();
        serde_json::to_string(&checks).unwrap_or_default()
    };
    let before = cached();
    forge::warm_ci_checks_cache(&ctx)?;
    tracing::debug!(elapsed = ?started.elapsed(), path = %path.display(), "refreshed checks");
    Ok(cached() != before)
}

/// What a new pull request for `branch` starts with: the branch it would merge into, and a
/// description from the repository's template if it has one.
pub struct NewReview {
    pub target: String,
    pub template: Option<String>,
}

pub fn new_review(path: &Path, branch: &super::FullName) -> Result<NewReview> {
    let ctx = open(path)?;
    let target = forge::new_review_target(&ctx, branch.to_string())?;
    let template = match forge::review_template(&ctx).ok().flatten() {
        Some(template) => Some(template.content),
        None => forge::list_available_review_templates(&ctx)
            .ok()
            .and_then(|paths| paths.into_iter().next())
            .and_then(|path| forge::pr_template(&ctx, path.into(), forge_name(&ctx)?).ok()),
    };
    Ok(NewReview { target, template })
}

/// A pull request just opened, and why updating the other pull requests of its stack failed,
/// if it did.
pub struct Published {
    pub number: u64,
    pub sync_failed: Option<String>,
}

/// Push `branch` (a full ref) and the branches below it, those that have anything to push,
/// then open a pull request for it. `run_hooks` runs the pre-push hook first.
pub fn publish(
    path: &Path,
    branch: &super::FullName,
    title: String,
    body: String,
    draft: bool,
    run_hooks: bool,
) -> Result<Published> {
    // The pull request's head is the branch as the remote names it: what a push reports,
    // or for a branch pushed before, the remote branch it tracks.
    let (pushed, push_sync_failed) = push(path, branch, true, Some(&[]), run_hooks)?;
    let short = short_ref(branch).into_owned();
    let ctx = open(path)?;
    let pushed = match pushed {
        Some(name) => Some(name),
        None => tracked_name(&ctx, branch)?,
    };
    let outcome = runtime().handle().block_on(forge::publish_review(
        ctx.into_sync(),
        forge::PublishReviewInput {
            title,
            body,
            source_branch: pushed.unwrap_or_else(|| short.clone()),
            local_branch: short,
            draft,
        },
    ))?;
    Ok(Published {
        number: outcome.review.number as u64,
        sync_failed: sync_failure(outcome.review_sync).or(push_sync_failed),
    })
}

/// The name of the remote branch `branch` (a full ref) tracks, without its remote.
fn tracked_name(ctx: &Context, branch: &super::FullName) -> Result<Option<String>> {
    let info = but_api::legacy::workspace::head_info(ctx)?;
    let tracked = (info.lanes())
        .flat_map(|lane| lane.segments)
        .find(|segment| holds(segment, branch))
        .and_then(|segment| segment.remote_tracking_ref_name.as_ref())
        .and_then(|name| {
            // The longest remote name that fits, as remote names can hold slashes too.
            let remotes = ctx.repo.get().ok()?.remote_names();
            let (_remote, branch) =
                but_core::extract_remote_name_and_short_name(name.as_ref(), &remotes)?;
            Some(branch.to_string())
        });
    Ok(tracked)
}

/// Ways to merge a pull request, as the forge offers them.
pub use but_forge::ReviewMergeMethod as MergeMethod;

/// Merge pull request `number`, then fetch so the target has the merge and the workspace can
/// update from it. Asking for the request again records it merged in the cache, so the
/// reload after shows it merged without waiting for the next listing. Whether it merged, as
/// GitHub can put a stacked one in its merge queue instead.
pub fn merge(path: &Path, number: u64, method: MergeMethod) -> Result<bool> {
    let ctx = open(path)?.into_sync();
    (runtime().handle()).block_on(forge::merge_review(ctx, number as usize, Some(method)))?;
    let merged = match open(path).and_then(|ctx| forge::get_review(&ctx, number as usize)) {
        Ok(review) => review.merged_at.is_some(),
        Err(err) => {
            tracing::warn!(?err, "merged, but couldn't read the pull request back");
            true
        }
    };
    if let Err(err) = super::fetch(path) {
        tracing::warn!(?err, "merged, but couldn't fetch");
    }
    Ok(merged)
}

/// What the forge says about whether a pull request can merge now.
pub use but_forge::ReviewMergeStatus as MergeStatus;

/// Ask the forge whether pull request `number` can merge now. GitHub works it out when asked,
/// so it can answer that it doesn't know yet.
pub fn merge_status(path: &Path, number: u64) -> Result<MergeStatus> {
    let ctx = open(path)?.into_sync();
    (runtime().handle()).block_on(forge::get_review_merge_status(ctx, number as usize))
}

/// Turn auto-merge of pull request `number` on or off. Asking for the request again records
/// that in the cache, so the reload after shows it.
pub fn set_auto_merge(path: &Path, number: u64, on: bool) -> Result<()> {
    let ctx = open(path)?.into_sync();
    (runtime().handle()).block_on(forge::set_review_auto_merge(ctx, number as usize, on))?;
    if let Err(err) = open(path).and_then(|ctx| forge::get_review(&ctx, number as usize)) {
        tracing::warn!(
            ?err,
            "set auto-merge, but couldn't read the pull request back"
        );
    }
    Ok(())
}
