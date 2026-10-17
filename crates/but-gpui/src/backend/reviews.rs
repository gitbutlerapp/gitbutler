//! Pull requests: what the forge cache knows about each branch's, what a new one would
//! say, publishing it, and changing one that exists. Calls to the forge go over the
//! network, so these run off the UI thread like the rest.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context as _, Result};
use but_api::legacy::forge;
use but_ctx::Context;

use super::ops::holds;
use super::{Checks, Forge, PushState, Review, ReviewState, open, push, runtime, title};

/// Make a call to the forge, which goes over the network, and wait for its answer.
fn forge_call<T, F: Future<Output = Result<T>>>(
    path: &Path,
    call: impl FnOnce(but_ctx::ThreadSafeContext) -> F,
) -> Result<T> {
    runtime().handle().block_on(call(open(path)?.into_sync()))
}

/// Which forge hosts the project, when its target's remote says so or the project picks
/// one in its settings.
fn forge_name(ctx: &Context) -> Option<but_forge::ForgeName> {
    let meta = ctx.project_meta().ok()?;
    meta.target_ref.as_ref()?;
    let url = forge::remote_url(&meta, &*ctx.repo.get().ok()?).ok()?;
    let overridden = ctx
        .legacy_project
        .forge_override
        .as_deref()
        .and_then(but_forge::ForgeName::from_override_str);
    Some(but_forge::derive_forge_repo_info(&url, overridden)?.forge)
}

/// [`forge_name`] as the app names forges.
pub(super) fn forge_of(ctx: &Context) -> Option<Forge> {
    use but_forge::ForgeName as F;
    Some(match forge_name(ctx)? {
        F::GitHub => Forge::GitHub,
        F::GitLab => Forge::GitLab,
        F::Bitbucket => Forge::Bitbucket,
        F::Azure => Forge::Azure,
    })
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
    let found = cached.get(&number);
    let cache_only = Some(but_forge::CacheConfig::CacheOnly);
    let checks = forge::list_ci_checks_for_ref(ctx, name, cache_only).unwrap_or_default();
    Review {
        number,
        title: found.map(|review| review.title.clone()).unwrap_or_default(),
        state: match found {
            // A merged pull request drops out of the open ones the forge lists.
            _ if push == PushState::Integrated => ReviewState::Merged,
            Some(review) => review_state(review),
            None => ReviewState::Open,
        },
        url: found.map(|review| review.html_url.clone()),
        checks: (!checks.is_empty()).then(|| summarize_checks(&checks)),
        failed_checks: (checks.iter())
            .filter(|check| check_failed(check) == Some(true))
            .map(|check| {
                let url = match check.html_url.as_str() {
                    "" => check.details_url.clone(),
                    url => url.to_owned(),
                };
                (check.name.clone(), url)
            })
            .collect(),
        auto_merge: found.is_some_and(|review| review.auto_merge_enabled),
        author: (found.and_then(|review| review.author.as_ref()))
            .map(|author| author.login.clone()),
        labels: (found.iter().flat_map(|review| &review.labels))
            .map(|label| label.name.clone())
            .collect(),
        reviewers: (found.iter().flat_map(|review| &review.reviewers))
            .filter(|user| !user.is_bot)
            .map(|user| user.login.clone())
            .collect(),
    }
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
/// [`load_workspace`](super::load_workspace) reads. Fails when the project has no forge or
/// no account for it.
pub fn refresh_reviews(path: &Path) -> Result<()> {
    let started = std::time::Instant::now();
    tracing::debug!(path = %path.display(), "refreshing reviews");
    let ctx = open(path)?;
    forge::list_reviews(&ctx, Some(but_forge::CacheConfig::NoCache))?;
    tracing::debug!(elapsed = ?started.elapsed(), path = %path.display(), "refreshed reviews");
    Ok(())
}

/// Ask the forge about the CI of each branch with a pull request, a request per branch.
pub fn refresh_checks(path: &Path) -> Result<()> {
    let started = std::time::Instant::now();
    let ctx = open(path)?;
    forge::warm_ci_checks_cache(&ctx)?;
    tracing::debug!(elapsed = ?started.elapsed(), path = %path.display(), "refreshed checks");
    Ok(())
}

/// What a new pull request for `branch` starts with: the branch it would merge into, and a
/// description from the repository's template if it has one.
pub struct NewReview {
    pub target: String,
    pub template: Option<String>,
}

pub fn new_review(path: &Path, branch: &str) -> Result<NewReview> {
    let ctx = open(path)?;
    let target = forge::new_review_target(&ctx, branch.to_owned())?;
    let template = match forge::review_template(&ctx).ok().flatten() {
        Some(template) => Some(template.content),
        None => forge::list_available_review_templates(&ctx)
            .ok()
            .and_then(|paths| paths.into_iter().next())
            .and_then(|path| forge::pr_template(&ctx, path.into(), forge_name(&ctx)?).ok()),
    };
    Ok(NewReview { target, template })
}

/// Push `branch` (a full ref) if it needs it, then open a pull request for it. Returns its
/// number.
pub fn publish(
    path: &Path,
    branch: &str,
    push_first: Option<bool>,
    title: String,
    body: String,
    draft: bool,
) -> Result<u64> {
    // The pull request's head is the branch as the remote names it: what a push reports,
    // or for a branch pushed before, the remote branch it tracks.
    let pushed = match push_first {
        Some(force) => push(path, branch, force, true)?,
        None => None,
    };
    let short = branch
        .strip_prefix("refs/heads/")
        .unwrap_or(branch)
        .to_owned();
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
    Ok(outcome.review.number as u64)
}

/// The name of the remote branch `branch` (a full ref) tracks, without its remote.
fn tracked_name(ctx: &Context, branch: &str) -> Result<Option<String>> {
    let info = but_api::legacy::workspace::head_info(ctx)?;
    let tracked = (info.stacks.iter())
        .flat_map(|stack| &stack.segments)
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

/// The title and description pull request `number` has now, from the forge.
pub fn review_text(path: &Path, number: u64) -> Result<(String, String)> {
    let review = forge::get_review(&open(path)?, number as usize)?;
    Ok((review.title, review.body.unwrap_or_default()))
}

/// Ways to merge a pull request, as the forge offers them.
pub use but_forge::ReviewMergeMethod as MergeMethod;

pub fn merge(path: &Path, number: u64, method: MergeMethod) -> Result<()> {
    forge_call(path, |ctx| {
        forge::merge_review(ctx, number as usize, Some(method))
    })
}

pub fn set_draft(path: &Path, number: u64, draft: bool) -> Result<()> {
    forge_call(path, |ctx| {
        forge::set_review_draftiness(ctx, number as usize, draft)
    })
}

/// How a pull request's reviewers last decided, by login.
#[derive(Debug, Default)]
pub struct Verdicts {
    pub approved: Vec<String>,
    pub changes_requested: Vec<String>,
}

/// Each reviewer's last decision, from their submissions oldest first: a comment doesn't
/// change it, and a dismissal takes it back.
fn fold_verdicts(
    submissions: impl IntoIterator<Item = (String, but_forge::ForgeReviewSubmissionState)>,
) -> Verdicts {
    use but_forge::ForgeReviewSubmissionState as State;
    let mut last = std::collections::BTreeMap::new();
    for (login, state) in submissions {
        match state {
            State::Approved | State::ChangesRequested => {
                last.insert(login, state);
            }
            State::Dismissed => {
                last.remove(&login);
            }
            State::Commented => {}
        }
    }
    let with = |wanted: State| -> Vec<String> {
        (last.iter())
            .filter(|(_, state)| **state == wanted)
            .map(|(login, _)| login.clone())
            .collect()
    };
    Verdicts {
        approved: with(State::Approved),
        changes_requested: with(State::ChangesRequested),
    }
}

/// A conversation left on a pull request's diff and not resolved yet.
#[derive(Debug)]
pub struct Thread {
    /// The forge's own identifier, to resolve it by.
    pub id: String,
    pub path: String,
    /// Its line on the new side of the diff as it is now, when it still has one.
    pub line: Option<u32>,
    /// Who opened it, and the first line of what they wrote.
    pub author: String,
    pub opening: String,
    pub replies: usize,
    pub url: String,
}

/// A comment on a pull request as a whole, rather than on a line of its diff.
#[derive(Debug)]
pub struct Comment {
    pub author: String,
    /// Its first line.
    pub opening: String,
    pub url: String,
}

/// Where a pull request's review stands: the reviewers' verdicts, the conversations
/// still open on its diff, and the latest comments people left on it.
#[derive(Debug)]
pub struct Feedback {
    pub verdicts: Verdicts,
    pub threads: Vec<Thread>,
    pub comments: Vec<Comment>,
}

/// What the reviewers of pull request `number` last decided, its open threads and its
/// latest comments. Each can fail on its own, as a forge may not offer it: only all of
/// them failing is an error.
pub fn feedback(path: &Path, number: u64) -> Result<Feedback> {
    use but_forge::ForgeReviewThreadSide as Side;
    let number = number as usize;
    let ctx = open(path)?.into_sync();
    let (submissions, threads, comments) = runtime().handle().block_on(futures::future::join3(
        forge::list_review_submissions(ctx.clone(), number),
        forge::list_review_threads(ctx.clone(), number),
        forge::list_review_comments(ctx, number),
    ));
    let verdicts = submissions.map(|submissions| {
        fold_verdicts(
            submissions
                .into_iter()
                .filter_map(|submission| Some((submission.author?.login, submission.state))),
        )
    });
    let threads = threads.map(|threads| {
        (threads.into_iter())
            .filter(|thread| !thread.is_resolved)
            .filter_map(|thread| {
                let first = thread.comments.first()?;
                Some(Thread {
                    line: (thread.line)
                        .filter(|_| matches!(thread.side, Side::New))
                        .and_then(|line| u32::try_from(line).ok()),
                    author: (first.author.as_ref())
                        .map_or_else(String::new, |author| author.login.clone()),
                    opening: title(first.body.as_bytes()),
                    replies: thread.comments.len() - 1,
                    url: first.html_url.clone(),
                    id: thread.id,
                    path: thread.path,
                })
            })
            .collect::<Vec<_>>()
    });
    /// More than this many are better read on the forge.
    const LATEST: usize = 3;
    let comments = comments.map(|comments| {
        // Oldest first; bots, like coverage reports, left out.
        let mut people: Vec<Comment> = (comments.into_iter())
            .filter_map(|comment| {
                let author = comment.author.filter(|author| !author.is_bot)?;
                Some(Comment {
                    author: author.login,
                    opening: title(comment.body.as_bytes()),
                    url: comment.html_url,
                })
            })
            .collect();
        people.split_off(people.len().saturating_sub(LATEST))
    });
    match (verdicts, threads, comments) {
        (Err(err), Err(_), Err(_)) => Err(err),
        (verdicts, threads, comments) => Ok(Feedback {
            verdicts: verdicts.unwrap_or_default(),
            threads: threads.unwrap_or_default(),
            comments: comments.unwrap_or_default(),
        }),
    }
}

/// Mark a conversation on a pull request's diff resolved.
pub fn resolve_thread(path: &Path, id: String) -> Result<()> {
    forge_call(path, |ctx| forge::set_review_thread_resolved(ctx, id, true))
}

/// Comment on pull request `number` as a whole.
pub fn comment(path: &Path, number: u64, body: String) -> Result<()> {
    forge_call(path, |ctx| {
        forge::create_review_comment(ctx, number as usize, body)
    })
    .map(drop)
}

/// Answer a conversation on a pull request's diff.
pub fn reply_to_thread(path: &Path, id: String, body: String) -> Result<()> {
    forge_call(path, |ctx| forge::create_review_thread_reply(ctx, id, body)).map(drop)
}

/// The labels the repository has, by name.
pub fn repo_labels(path: &Path) -> Result<Vec<String>> {
    let labels = forge_call(path, forge::list_repo_labels)?;
    Ok(labels.into_iter().map(|label| label.name).collect())
}

/// Put `label` on pull request `number`, or with `on` false take it off.
pub fn set_label(path: &Path, number: u64, label: String, on: bool) -> Result<()> {
    let number = number as usize;
    if on {
        forge_call(path, |ctx| {
            forge::add_review_labels(ctx, number, vec![label])
        })
        .map(drop)
    } else {
        forge_call(path, |ctx| forge::remove_review_label(ctx, number, label))
    }
}

/// Who can be asked to review: the logins of the repository's collaborators, bots left
/// out, in alphabetical order.
pub fn reviewer_candidates(path: &Path) -> Result<Vec<String>> {
    let users = forge_call(path, forge::list_reviewer_candidates)?;
    let mut logins: Vec<String> = (users.into_iter())
        .filter(|user| !user.is_bot)
        .map(|user| user.login)
        .collect();
    logins.sort_by_key(|login| login.to_lowercase());
    Ok(logins)
}

/// Ask `login` to review pull request `number`, or with `ask` false, no longer.
pub fn request_review(path: &Path, number: u64, login: String, ask: bool) -> Result<()> {
    let (number, logins) = (number as usize, vec![login]);
    if ask {
        forge_call(path, |ctx| forge::request_review(ctx, number, logins))
    } else {
        forge_call(path, |ctx| {
            forge::withdraw_review_request(ctx, number, logins)
        })
    }
}

/// Have the forge merge a pull request once its checks pass, or stop it from doing so.
pub fn set_auto_merge(path: &Path, number: u64, enable: bool) -> Result<()> {
    forge_call(path, |ctx| {
        forge::set_review_auto_merge(ctx, number as usize, enable)
    })
}

/// Change a pull request's title and description, or close or reopen it.
pub fn update(
    path: &Path,
    number: u64,
    text: Option<(String, String)>,
    open_state: Option<bool>,
) -> Result<()> {
    let (title, body) = text.unzip();
    let state = open_state.map(|open| {
        if open {
            but_forge::ReviewState::Open
        } else {
            but_forge::ReviewState::Closed
        }
    });
    forge_call(path, |ctx| {
        forge::update_review(ctx, number as usize, title, body, state, None)
    })
    .context("The forge didn't take the change")
}

#[cfg(test)]
mod tests {
    use but_forge::ForgeReviewSubmissionState as State;

    use super::fold_verdicts;

    #[test]
    fn a_reviewers_last_decision_counts() {
        let verdicts = fold_verdicts([
            ("alice".into(), State::ChangesRequested),
            ("alice".into(), State::Commented),
            ("alice".into(), State::Approved),
            ("bob".into(), State::Approved),
            ("bob".into(), State::ChangesRequested),
            ("carol".into(), State::Approved),
            ("carol".into(), State::Dismissed),
            ("dana".into(), State::Commented),
        ]);
        assert_eq!(
            verdicts.approved,
            ["alice"],
            "a comment after a decision keeps it"
        );
        assert_eq!(
            verdicts.changes_requested,
            ["bob"],
            "a later change request replaces an approval, and a dismissal takes one back"
        );
    }
}
