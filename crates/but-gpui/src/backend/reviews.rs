//! Pull requests: what the forge cache knows about each branch's, what a new one would
//! say, publishing it, and merging one. Calls to the forge go over the network, so these
//! run off the UI thread like the rest.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Result;
use but_api::legacy::forge;
use but_ctx::Context;
use but_forge::ForgeName;

use super::ops::holds;
use super::upstream::sync_failure;
use super::{
    CheckRun, Checks, DiffLine, Forge, ForgeUser, Note, PushState, Remark, Review, ReviewLabel,
    ReviewState, Thread, open, push, runtime, short_ref,
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

/// A review's verdict, as the forge gives it.
pub use but_forge::ForgeReviewSubmissionState as Verdict;

/// How many of the diff's lines a thread quotes at least, as GitHub does.
const EXCERPT_LINES: usize = 4;

/// What's been said on pull request `number`, newest first. Only GitHub's is read; other
/// forges' reads as nothing.
pub fn conversation(path: &Path, number: u64) -> Result<Vec<Remark>> {
    let started = std::time::Instant::now();
    let ctx = open(path)?.into_sync();
    let (comments, reviews, threads) = runtime().handle().block_on(async {
        futures::try_join!(
            forge::list_review_comments(ctx.clone(), number as usize),
            forge::list_review_submissions(ctx.clone(), number as usize),
            forge::list_review_threads(ctx, number as usize),
        )
    })?;
    tracing::debug!(elapsed = ?started.elapsed(), number, "read the conversation");
    Ok(remarks(comments, reviews, threads))
}

/// `comments`, `reviews` and `threads` as the conversation shows them, newest first, as Lite's,
/// those without a time last: each review followed by the threads it started, each a remark of
/// its own so a long review draws only what's on screen, and nothing that says nothing, as a
/// comment of only HTML comments, or the empty review GitHub makes of an answer in a thread,
/// which shows in the thread.
fn remarks(
    comments: Vec<but_forge::ForgeReviewComment>,
    reviews: Vec<but_forge::ForgeReviewSubmission>,
    threads: Vec<but_forge::ForgeReviewThread>,
) -> Vec<Remark> {
    let listed: HashSet<i64> = reviews.iter().map(|review| review.id).collect();
    let mut started: HashMap<i64, Vec<Thread>> = HashMap::new();
    // Each remark with those that follow it, by when it was written.
    let mut dated: Vec<(Option<i64>, Vec<Remark>)> = Vec::new();
    for thread in threads {
        let first = thread.comments.first();
        let at = first.and_then(|comment| timestamp(comment.created_at.as_deref()));
        let review = (first.and_then(|comment| comment.review_id)).filter(|id| listed.contains(id));
        match review {
            Some(id) => started.entry(id).or_default().push(thread_of(thread)),
            None => dated.push((
                at,
                vec![Remark::Thread {
                    thread: thread_of(thread),
                    in_review: false,
                }],
            )),
        }
    }
    for comment in comments {
        let note = note(
            comment.author.as_ref(),
            &comment.body,
            comment.created_at.as_deref(),
            comment.modified_at.as_deref(),
            comment.html_url,
        );
        if !note.body.is_empty() {
            dated.push((note.at, vec![Remark::Comment(note)]));
        }
    }
    for review in reviews {
        let threads = started.remove(&review.id).unwrap_or_default();
        let body = review.body.as_deref().unwrap_or_default();
        let note = note(
            review.author.as_ref(),
            body,
            review.submitted_at.as_deref(),
            None,
            review.html_url,
        );
        if review.state == Verdict::Commented && note.body.is_empty() && threads.is_empty() {
            continue;
        }
        let (at, verdict) = (note.at, review.state);
        let followed = (threads.into_iter()).map(|thread| Remark::Thread {
            thread,
            in_review: true,
        });
        dated.push((
            at,
            std::iter::once(Remark::Review { verdict, note })
                .chain(followed)
                .collect(),
        ));
    }
    dated.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
    dated.into_iter().flat_map(|(_, remarks)| remarks).collect()
}

fn note(
    author: Option<&but_forge::ForgeReviewUser>,
    body: &str,
    at: Option<&str>,
    edited_at: Option<&str>,
    url: String,
) -> Note {
    let at = timestamp(at);
    // GitHub's REST names a bot "name[bot]", its GraphQL "name"; the tag says it either way.
    let bot = author.is_some_and(|user| user.is_bot || user.login.ends_with("[bot]"));
    Note {
        author: author.map(|user| ForgeUser {
            login: (user.login.strip_suffix("[bot]"))
                .unwrap_or(&user.login)
                .to_owned(),
            avatar_url: user.avatar_url.clone(),
        }),
        bot,
        body: crate::ui::markdown::parse(body),
        at,
        edited: (timestamp(edited_at).zip(at)).is_some_and(|(edited, at)| edited > at),
        url,
    }
}

fn thread_of(thread: but_forge::ForgeReviewThread) -> Thread {
    // Where it was left, as the excerpt numbers it: the forge's current lines move with later
    // pushes, and an outdated thread has none.
    let span = (thread.line.zip(thread.start_line)).map_or(0, |(end, start)| (end - start).max(0));
    let lines = (thread.original_line.or(thread.line)).map(|end| ((end - span).max(1), end));
    let quoted = lines.map_or(1, |(start, end)| (end - start + 1) as usize);
    let excerpt = (thread.comments.first())
        .and_then(|comment| comment.diff_hunk.as_deref())
        .map(|hunk| excerpt(&thread.path, hunk, quoted))
        .unwrap_or_default();
    Thread {
        id: thread.id,
        lines,
        resolved: thread.is_resolved,
        outdated: thread.is_outdated,
        excerpt,
        comments: (thread.comments.into_iter())
            .map(|comment| {
                note(
                    comment.author.as_ref(),
                    &comment.body,
                    comment.created_at.as_deref(),
                    comment.modified_at.as_deref(),
                    comment.html_url,
                )
            })
            .collect(),
        path: thread.path,
    }
}

/// The last `count` lines of `hunk`, a hunk of `path` as GitHub quotes one up to where a
/// thread was left, and at least [`EXCERPT_LINES`] of them, numbered and highlighted.
fn excerpt(path: &str, hunk: &str, count: usize) -> Vec<DiffLine> {
    let Some((old_start, new_start)) = hunk.lines().next().and_then(hunk_starts) else {
        return Vec::new();
    };
    let mut lines = super::diff::parse_hunk(&but_core::unified_diff::DiffHunk {
        old_start,
        old_lines: 0,
        new_start,
        new_lines: 0,
        diff: hunk.into(),
    })
    .lines;
    super::diff::highlight_lines(path, lines.iter_mut());
    lines.split_off(lines.len().saturating_sub(count.max(EXCERPT_LINES)))
}

/// Where a hunk header, as "@@ -12,5 +12,6 @@", starts on the old side and the new.
fn hunk_starts(header: &str) -> Option<(u32, u32)> {
    let (old, new) = header.strip_prefix("@@ -")?.split_once(" +")?;
    let start = |range: &str| range.split([',', ' ']).next()?.parse().ok();
    Some((start(old)?, start(new)?))
}

#[cfg(test)]
mod tests {
    use but_forge::{
        ForgeReviewComment, ForgeReviewSubmission, ForgeReviewThread, ForgeReviewThreadComment,
        ForgeReviewThreadSide, ForgeReviewUser,
    };

    use super::{Remark, Verdict, excerpt, remarks};
    use crate::backend::{LineKind, Note};

    fn user(login: &str, is_bot: bool) -> Option<ForgeReviewUser> {
        Some(ForgeReviewUser {
            id: 1,
            login: login.into(),
            name: None,
            email: None,
            avatar_url: None,
            is_bot,
        })
    }

    fn at(minute: u32) -> Option<String> {
        Some(format!("2026-10-27T10:{minute:02}:00Z"))
    }

    fn comment(id: i64, login: &str, body: &str, minute: Option<u32>) -> ForgeReviewComment {
        ForgeReviewComment {
            id,
            body: body.into(),
            author: user(login, false),
            created_at: minute.and_then(at),
            modified_at: minute.and_then(at),
            html_url: format!("https://github.com/o/r/pull/1#issuecomment-{id}"),
            reactions: Vec::new(),
        }
    }

    fn review(
        id: i64,
        login: &str,
        state: Verdict,
        body: &str,
        minute: u32,
    ) -> ForgeReviewSubmission {
        ForgeReviewSubmission {
            id,
            author: user(login, login.ends_with("[bot]")),
            state,
            body: (!body.is_empty()).then(|| body.into()),
            submitted_at: at(minute),
            html_url: format!("https://github.com/o/r/pull/1#pullrequestreview-{id}"),
            reactions: Vec::new(),
        }
    }

    /// A thread on `path`, its comments each by a login, in the review given, at a minute.
    fn thread(path: &str, comments: &[(&str, i64, u32)]) -> ForgeReviewThread {
        ForgeReviewThread {
            id: format!("thread-{path}"),
            path: path.into(),
            line: Some(3),
            start_line: Some(3),
            original_line: Some(3),
            side: ForgeReviewThreadSide::New,
            is_resolved: false,
            is_outdated: false,
            comments: (comments.iter())
                .map(|(login, review, minute)| ForgeReviewThreadComment {
                    id: i64::from(*minute),
                    body: format!("{login} on {path}"),
                    author: user(login, false),
                    created_at: at(*minute),
                    modified_at: at(*minute),
                    html_url: format!("https://github.com/o/r/pull/1#discussion_r{minute}"),
                    diff_hunk: None,
                    review_id: Some(*review),
                })
                .collect(),
        }
    }

    fn describe(remarks: &[Remark]) -> String {
        let note = |note: &Note| {
            let who = note.author.as_ref().map_or("?", |author| &author.login);
            let tag = if note.bot { " (bot)" } else { "" };
            let words: Vec<_> = note.body.iter().map(|block| block.text.as_str()).collect();
            format!("{who}{tag}: {:?}", words.join(" / "))
        };
        let thread = |thread: &super::Thread| {
            let said: Vec<_> = thread.comments.iter().map(note).collect();
            format!("  thread {} [{}]", thread.path, said.join(", "))
        };
        let mut out = Vec::new();
        for remark in remarks {
            match remark {
                Remark::Comment(said) => out.push(format!("comment {}", note(said))),
                Remark::Review {
                    verdict,
                    note: said,
                } => out.push(format!("review {verdict:?} {}", note(said))),
                Remark::Thread {
                    thread: said,
                    in_review,
                } => out.push(match in_review {
                    true => thread(said),
                    false => thread(said).trim_start().into(),
                }),
            }
        }
        out.join("\n")
    }

    #[test]
    fn a_conversation_files_threads_under_their_review_newest_first() {
        let comments = vec![
            comment(1, "alice", "First!", Some(1)),
            comment(2, "linear[bot]", "<!-- linear-linkback -->", Some(2)),
            comment(3, "bob", "Undated", None),
            comment(4, "carol", "Latest", Some(9)),
        ];
        let reviews = vec![
            review(10, "codex[bot]", Verdict::Commented, "Some suggestions", 3),
            // GitHub's review for alice's answer in the thread, which says nothing itself.
            review(11, "alice", Verdict::Commented, "", 5),
            review(12, "dave", Verdict::Approved, "", 7),
        ];
        let threads = vec![
            thread("src/a.rs", &[("codex", 10, 3), ("alice", 11, 5)]),
            // Its review isn't listed, so it stands on its own.
            thread("src/b.rs", &[("erin", 99, 4)]),
        ];
        // Threads sit under the review that started them, and an answer's empty review and
        // a comment of only HTML comments are left out. Undated, bob's comes last.
        snapbox::assert_data_eq!(
            describe(&remarks(comments, reviews, threads)),
            snapbox::str![[r#"
comment carol: "Latest"
review Approved dave: ""
thread src/b.rs [erin: "erin on src/b.rs"]
review Commented codex (bot): "Some suggestions"
  thread src/a.rs [codex: "codex on src/a.rs", alice: "alice on src/a.rs"]
comment alice: "First!"
comment bob: "Undated"
"#]]
        );
    }

    #[test]
    fn an_excerpt_is_the_hunk_s_last_lines_numbered_from_its_header() {
        let hunk = "@@ -10,6 +10,7 @@ fn main() {\n a\n b\n-c\n+C\n+D\n d\n e";
        let lines: Vec<_> = (excerpt("main.rs", hunk, 1).iter())
            .map(|line| (line.kind, line.old, line.new, line.text.to_string()))
            .collect();
        assert_eq!(
            lines,
            [
                (LineKind::Added, None, Some(12), "C".into()),
                (LineKind::Added, None, Some(13), "D".into()),
                (LineKind::Context, Some(13), Some(14), "d".into()),
                (LineKind::Context, Some(14), Some(15), "e".into()),
            ],
            "a one-line thread quotes four lines, numbered on from the header's starts"
        );
    }
}
