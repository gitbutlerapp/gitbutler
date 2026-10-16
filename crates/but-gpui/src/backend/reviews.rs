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
    if let Some(force) = push_first {
        push(path, branch, force)?;
    }
    let short = branch
        .strip_prefix("refs/heads/")
        .unwrap_or(branch)
        .to_owned();
    let ctx = open(path)?.into_sync();
    let outcome = runtime().handle().block_on(forge::publish_review(
        ctx,
        forge::PublishReviewInput {
            title,
            body,
            local_branch: short.clone(),
            source_branch: short,
            draft,
        },
    ))?;
    Ok(outcome.review.number as u64)
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
