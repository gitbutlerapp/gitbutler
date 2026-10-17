//! Pull requests: what a new one would say, publishing it, and changing one that exists.
//! Calls to the forge go over the network, so these run off the UI thread like the rest.

use std::path::Path;

use anyhow::{Context as _, Result};
use but_api::legacy::forge;

use super::{open, push, runtime};

/// Which forge hosts the project, like "GitHub", when its target's remote says so.
pub fn forge_name(ctx: &but_ctx::Context) -> Option<String> {
    let meta = ctx.project_meta().ok()?;
    meta.target_ref.as_ref()?;
    let url = forge::remote_url(&meta, &*ctx.repo.get().ok()?).ok()?;
    let overridden = ctx
        .legacy_project
        .forge_override
        .as_deref()
        .and_then(but_forge::ForgeName::from_override_str);
    let info = but_forge::derive_forge_repo_info(&url, overridden)?;
    Some(format!("{:?}", info.forge))
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
            .and_then(|path| {
                let forge = but_forge::derive_forge_repo_info(
                    &forge::remote_url(&ctx.project_meta().ok()?, &*ctx.repo.get().ok()?).ok()?,
                    None,
                )?
                .forge;
                forge::pr_template(&ctx, path.into(), forge).ok()
            }),
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
fn tracked_name(ctx: &but_ctx::Context, branch: &str) -> Result<Option<String>> {
    let info = but_api::legacy::workspace::head_info(ctx)?;
    let tracked = (info.stacks.iter())
        .flat_map(|stack| &stack.segments)
        .find(|segment| (segment.ref_info.as_ref()).is_some_and(|r| r.ref_name.as_bstr() == branch))
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
#[derive(Debug, Clone, Copy)]
pub enum MergeMethod {
    Merge,
    Squash,
    Rebase,
}

pub fn merge(path: &Path, number: u64, method: MergeMethod) -> Result<()> {
    let method = match method {
        MergeMethod::Merge => but_forge::ReviewMergeMethod::Merge,
        MergeMethod::Squash => but_forge::ReviewMergeMethod::Squash,
        MergeMethod::Rebase => but_forge::ReviewMergeMethod::Rebase,
    };
    let ctx = open(path)?.into_sync();
    runtime()
        .handle()
        .block_on(forge::merge_review(ctx, number as usize, Some(method)))
}

pub fn set_draft(path: &Path, number: u64, draft: bool) -> Result<()> {
    let ctx = open(path)?.into_sync();
    runtime()
        .handle()
        .block_on(forge::set_review_draftiness(ctx, number as usize, draft))
}

/// How a pull request's reviewers last decided, by login.
#[derive(Debug, Default)]
pub struct Verdicts {
    pub approved: Vec<String>,
    pub changes_requested: Vec<String>,
}

/// What each reviewer of pull request `number` decided last: a comment doesn't change it,
/// and a dismissal takes it back.
pub fn verdicts(path: &Path, number: u64) -> Result<Verdicts> {
    use but_forge::ForgeReviewSubmissionState as State;
    let ctx = open(path)?.into_sync();
    let submissions = runtime()
        .handle()
        .block_on(forge::list_review_submissions(ctx, number as usize))?;
    let mut last = std::collections::BTreeMap::new();
    for submission in submissions {
        let Some(login) = submission.author.map(|author| author.login) else {
            continue;
        };
        match submission.state {
            State::Approved | State::ChangesRequested => {
                last.insert(login, submission.state);
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
    Ok(Verdicts {
        approved: with(State::Approved),
        changes_requested: with(State::ChangesRequested),
    })
}

/// A conversation left on a pull request's diff and not resolved yet.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub struct Comment {
    pub author: String,
    /// Its first line.
    pub opening: String,
    pub url: String,
}

/// Where a pull request's review stands: the reviewers' verdicts, the conversations
/// still open on its diff, and the latest comments people left on it.
#[derive(Debug, Default)]
pub struct Feedback {
    pub verdicts: Verdicts,
    pub threads: Vec<Thread>,
    pub comments: Vec<Comment>,
}

/// What [`verdicts`], the open threads and the latest comments of pull request `number`
/// come to. Each can fail on its own, as a forge may not offer it: only all of them
/// failing is an error.
pub fn feedback(path: &Path, number: u64) -> Result<Feedback> {
    use but_forge::ForgeReviewThreadSide as Side;
    let verdicts = verdicts(path, number);
    let threads = (|| -> Result<Vec<Thread>> {
        let ctx = open(path)?.into_sync();
        let threads = runtime()
            .handle()
            .block_on(forge::list_review_threads(ctx, number as usize))?;
        Ok((threads.into_iter())
            .filter(|thread| !thread.is_resolved)
            .filter_map(|thread| {
                let first = thread.comments.first()?;
                Some(Thread {
                    line: (thread.line)
                        .filter(|_| matches!(thread.side, Side::New))
                        .and_then(|line| u32::try_from(line).ok()),
                    author: (first.author.as_ref())
                        .map_or_else(String::new, |author| author.login.clone()),
                    opening: first.body.lines().next().unwrap_or_default().to_owned(),
                    replies: thread.comments.len() - 1,
                    url: first.html_url.clone(),
                    id: thread.id,
                    path: thread.path,
                })
            })
            .collect())
    })();
    /// More than this many are better read on the forge.
    const LATEST: usize = 3;
    let comments = (|| -> Result<Vec<Comment>> {
        let ctx = open(path)?.into_sync();
        let comments = runtime()
            .handle()
            .block_on(forge::list_review_comments(ctx, number as usize))?;
        // Oldest first; bots, like coverage reports, left out.
        let mut people: Vec<Comment> = (comments.into_iter())
            .filter_map(|comment| {
                let author = comment.author.filter(|author| !author.is_bot)?;
                Some(Comment {
                    author: author.login,
                    opening: comment.body.lines().next().unwrap_or_default().to_owned(),
                    url: comment.html_url,
                })
            })
            .collect();
        Ok(people.split_off(people.len().saturating_sub(LATEST)))
    })();
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
    let ctx = open(path)?.into_sync();
    runtime()
        .handle()
        .block_on(forge::set_review_thread_resolved(ctx, id, true))
}

/// Comment on pull request `number` as a whole.
pub fn comment(path: &Path, number: u64, body: String) -> Result<()> {
    let ctx = open(path)?.into_sync();
    runtime()
        .handle()
        .block_on(forge::create_review_comment(ctx, number as usize, body))?;
    Ok(())
}

/// Answer a conversation on a pull request's diff.
pub fn reply_to_thread(path: &Path, id: String, body: String) -> Result<()> {
    let ctx = open(path)?.into_sync();
    runtime()
        .handle()
        .block_on(forge::create_review_thread_reply(ctx, id, body))?;
    Ok(())
}

/// The labels the repository has, by name.
pub fn repo_labels(path: &Path) -> Result<Vec<String>> {
    let ctx = open(path)?.into_sync();
    let labels = runtime().handle().block_on(forge::list_repo_labels(ctx))?;
    Ok(labels.into_iter().map(|label| label.name).collect())
}

/// Put `label` on pull request `number`, or with `on` false take it off.
pub fn set_label(path: &Path, number: u64, label: String, on: bool) -> Result<()> {
    let ctx = open(path)?.into_sync();
    let runtime = runtime().handle();
    if on {
        runtime.block_on(forge::add_review_labels(ctx, number as usize, vec![label]))?;
    } else {
        runtime.block_on(forge::remove_review_label(ctx, number as usize, label))?;
    }
    Ok(())
}

/// Who can be asked to review: the logins of the repository's collaborators, bots left
/// out, in alphabetical order.
pub fn reviewer_candidates(path: &Path) -> Result<Vec<String>> {
    let ctx = open(path)?.into_sync();
    let users = runtime()
        .handle()
        .block_on(forge::list_reviewer_candidates(ctx))?;
    let mut logins: Vec<String> = (users.into_iter())
        .filter(|user| !user.is_bot)
        .map(|user| user.login)
        .collect();
    logins.sort_by_key(|login| login.to_lowercase());
    Ok(logins)
}

/// Ask `login` to review pull request `number`, or with `ask` false, no longer.
pub fn request_review(path: &Path, number: u64, login: String, ask: bool) -> Result<()> {
    let ctx = open(path)?.into_sync();
    let runtime = runtime().handle();
    if ask {
        runtime.block_on(forge::request_review(ctx, number as usize, vec![login]))
    } else {
        runtime.block_on(forge::withdraw_review_request(
            ctx,
            number as usize,
            vec![login],
        ))
    }
}

/// Have the forge merge a pull request once its checks pass, or stop it from doing so.
pub fn set_auto_merge(path: &Path, number: u64, enable: bool) -> Result<()> {
    let ctx = open(path)?.into_sync();
    runtime()
        .handle()
        .block_on(forge::set_review_auto_merge(ctx, number as usize, enable))
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
    let ctx = open(path)?.into_sync();
    runtime()
        .handle()
        .block_on(forge::update_review(
            ctx,
            number as usize,
            title,
            body,
            state,
            None,
        ))
        .context("The forge didn't take the change")
}
