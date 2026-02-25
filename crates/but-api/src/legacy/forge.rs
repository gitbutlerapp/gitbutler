//! In place of commands.rs
use anyhow::{Context as _, Result};
use but_api_macros::but_api;
use but_core::RepositoryExt;
use but_ctx::{Context, ThreadSafeContext};
use but_forge::{
    ForgeName, ReviewTemplateFunctions, available_review_templates, get_review_template_functions,
};
use gitbutler_repo::RepoCommands;
use tracing::instrument;

/// (Deprecated) Get the list of PR template paths for the given project and forge.
/// This function is deprecated in favor of `list_available_review_templates`.
#[but_api]
#[instrument(err(Debug))]
pub fn pr_templates(ctx: &but_ctx::Context, forge: ForgeName) -> Result<Vec<String>> {
    Ok(available_review_templates(&ctx.workdir_or_fail()?, &forge))
}

/// Get the forge provider name.
///
/// This is determined by the forge the base branch is pointing to.
#[but_api]
#[instrument(err(Debug))]
pub fn forge_provider(ctx: &Context) -> Result<Option<ForgeName>> {
    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(ctx)?;
    let forge_repo_info = but_forge::derive_forge_repo_info(&base_branch.remote_url);
    Ok(forge_repo_info.map(|info| info.forge))
}

/// Get the list of review template paths for the given project.
#[but_api]
#[instrument(err(Debug))]
pub fn list_available_review_templates(ctx: &Context) -> Result<Vec<String>> {
    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(ctx)?;
    let forge_repo_info = but_forge::derive_forge_repo_info(&base_branch.remote_url);
    let forge = &forge_repo_info
        .as_ref()
        .context("No forge could be determined for this repository branch")?
        .forge;

    Ok(available_review_templates(&ctx.workdir_or_gitdir()?, forge))
}

/// (Deprecated) Get the PR template content for the given project and relative path.
///
/// This function is deprecated in favor of `review_template`, which serves the same purpose
/// but uses the updated storage location.
#[but_api]
#[instrument(err(Debug))]
pub fn pr_template(
    ctx: &but_ctx::Context,
    relative_path: std::path::PathBuf,
    forge: ForgeName,
) -> Result<String> {
    let ReviewTemplateFunctions {
        is_valid_review_template_path,
        ..
    } = get_review_template_functions(&forge);

    if !is_valid_review_template_path(&relative_path) {
        return Err(anyhow::format_err!(
            "Invalid review template path: {:?}",
            ctx.workdir_or_fail()?.join(relative_path),
        ));
    }
    ctx.read_file_from_workspace(&relative_path)?
        .content
        .context("PR template was not valid UTF-8")
}

mod json {
    /// Information about the project's review template.
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct ReviewTemplateInfo {
        /// The relative path to the review template within the repository.
        pub path: String,
        /// The content of the review template.
        pub content: String,
    }
}

/// Get the review template content for the given project and relative path.
///
/// This function determines the forge of a project and retrieves the review template
/// from the git config.
#[but_api]
#[instrument(err(Debug))]
pub fn review_template(ctx: &Context) -> Result<Option<json::ReviewTemplateInfo>> {
    let project = gitbutler_project::get_validated(ctx.legacy_project.id)?;
    let ctx = Context::new_from_legacy_project(project.clone())?;
    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
    let forge_repo_info = but_forge::derive_forge_repo_info(&base_branch.remote_url);
    let forge = &forge_repo_info
        .as_ref()
        .context("No forge could be determined for this repository branch")?
        .forge;

    let repo = ctx.repo.get()?;
    match repo.git_settings()?.gitbutler_forge_review_template_path {
        Some(review_template_path) => {
            let ReviewTemplateFunctions {
                is_valid_review_template_path,
                ..
            } = get_review_template_functions(forge);
            let template_path = review_template_path.to_string();
            let path = std::path::PathBuf::from(&template_path);

            if !is_valid_review_template_path(&path) {
                return Err(anyhow::format_err!(
                    "Invalid review template path: {:?}",
                    ctx.workdir_or_fail()?.join(path),
                ));
            }
            let content = ctx
                .read_file_from_workspace(&path)?
                .content
                .context("PR template was not valid UTF-8")?;

            Ok(Some(json::ReviewTemplateInfo {
                path: template_path,
                content,
            }))
        }
        None => Ok(None),
    }
}

/// Set the review template path in the git configuration for the given project.
/// The template path will be validated.
#[but_api]
#[instrument(err(Debug))]
pub fn set_review_template(ctx: &but_ctx::Context, template_path: Option<String>) -> Result<()> {
    let repo = ctx.open_isolated_repo()?;
    let mut git_config = repo.git_settings()?;

    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(ctx)?;
    let forge_repo_info = but_forge::derive_forge_repo_info(&base_branch.remote_url);
    let forge = &forge_repo_info
        .as_ref()
        .context("No forge could be determined for this repository branch")?
        .forge;

    let ReviewTemplateFunctions {
        is_valid_review_template_path,
        ..
    } = get_review_template_functions(forge);

    if let Some(ref path) = template_path {
        let path_buf = std::path::PathBuf::from(path);
        if !is_valid_review_template_path(&path_buf) {
            let wd = ctx.workdir_or_fail()?.join(&path_buf);
            return Err(anyhow::format_err!("Invalid review template path: {wd:?}"));
        }
    }

    git_config.gitbutler_forge_review_template_path = template_path.map(|p| p.into());
    repo.set_git_settings(&git_config)
}

#[but_api]
#[instrument(err(Debug))]
pub fn list_reviews(
    ctx: &mut Context,
    cache_config: Option<but_forge::CacheConfig>,
) -> Result<Vec<but_forge::ForgeReview>> {
    let (storage, forge_repo_info, preferred_forge_user) = {
        let base_branch = gitbutler_branch_actions::base::get_base_branch_data(ctx)?;
        let forge_repo_info = but_forge::derive_forge_repo_info(&base_branch.remote_url);

        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            forge_repo_info,
            ctx.legacy_project.preferred_forge_user.clone(),
        )
    };

    let db = &mut *ctx.db.get_mut()?;

    but_forge::list_forge_reviews_with_cache(
        preferred_forge_user,
        &forge_repo_info.context("No forge could be determined for this repository branch")?,
        &storage,
        db,
        cache_config,
    )
}

#[but_api]
#[instrument(err(Debug))]
pub fn get_review(ctx: &mut Context, review_id: usize) -> Result<but_forge::ForgeReview> {
    let (storage, forge_repo_info, preferred_forge_user) = {
        let base_branch = gitbutler_branch_actions::base::get_base_branch_data(ctx)?;
        let forge_repo_info = but_forge::derive_forge_repo_info(&base_branch.remote_url)
            .context("No forge could be determined for this repository.")?;

        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            forge_repo_info,
            ctx.legacy_project.preferred_forge_user.clone(),
        )
    };

    let db = &mut *ctx.db.get_mut()?;
    but_forge::get_forge_review(
        &preferred_forge_user,
        &forge_repo_info,
        review_id,
        db,
        &storage,
    )
}

#[but_api]
#[instrument(skip(ctx), err(Debug))]
pub fn list_ci_checks_and_update_cache(
    ctx: &mut Context,
    reference: String,
    cache_config: Option<but_forge::CacheConfig>,
) -> Result<Vec<but_forge::CiCheck>> {
    let (storage, forge_repo_info, preferred_forge_user) = {
        let base_branch = gitbutler_branch_actions::base::get_base_branch_data(ctx)?;
        let forge_repo_info = but_forge::derive_forge_repo_info(&base_branch.remote_url);

        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            forge_repo_info,
            ctx.legacy_project.preferred_forge_user.clone(),
        )
    };
    let db = &mut *ctx.db.get_mut()?;

    but_forge::ci_checks_for_ref_with_cache(
        preferred_forge_user,
        &forge_repo_info.context("No forge could be determined for this repository branch")?,
        &storage,
        &reference,
        db,
        cache_config,
    )
}

#[but_api]
#[instrument(err(Debug))]
pub async fn publish_review(
    ctx: ThreadSafeContext,
    params: but_forge::CreateForgeReviewParams,
) -> Result<but_forge::ForgeReview> {
    let (storage, forge_repo_info, forge_push_repo_info, preferred_forge_user) = {
        let ctx = ctx.into_thread_local();
        let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
        let forge_repo_info = but_forge::derive_forge_repo_info(&base_branch.remote_url)
            .context("No forge could be determined for this repository branch")?;
        let forge_push_repo_info = if base_branch.remote_url != base_branch.push_remote_url {
            let info = but_forge::derive_forge_repo_info(&base_branch.push_remote_url).context(
                "Failed to derive forge repository information from the push remote URL.",
            )?;
            Some(info)
        } else {
            None
        };

        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            forge_repo_info,
            forge_push_repo_info,
            ctx.legacy_project.preferred_forge_user.clone(),
        )
    };

    but_forge::create_forge_review(
        &preferred_forge_user,
        &forge_repo_info,
        &forge_push_repo_info,
        &params,
        &storage,
    )
    .await
}

/// Merge a review on the forge.
#[but_api]
#[instrument(err(Debug))]
pub async fn merge_review(ctx: ThreadSafeContext, review_id: usize) -> Result<()> {
    let (storage, forge_repo_info, preferred_forge_user) = {
        let ctx = ctx.into_thread_local();
        let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
        let forge_repo_info = but_forge::derive_forge_repo_info(&base_branch.remote_url);

        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            forge_repo_info,
            ctx.legacy_project.preferred_forge_user.clone(),
        )
    };

    but_forge::merge_review(
        &preferred_forge_user,
        &forge_repo_info.context("No forge could be determined for this repository branch")?,
        review_id,
        &storage,
    )
    .await
}

/// Enable or disable a review's auto-merge.
#[but_api]
#[instrument(err(Debug))]
pub async fn set_review_auto_merge(
    ctx: ThreadSafeContext,
    review_id: usize,
    enable: bool,
) -> Result<()> {
    let (storage, forge_repo_info, preferred_forge_user) = {
        let ctx = ctx.into_thread_local();
        let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
        let forge_repo_info = but_forge::derive_forge_repo_info(&base_branch.remote_url);

        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            forge_repo_info,
            ctx.legacy_project.preferred_forge_user.clone(),
        )
    };

    but_forge::set_review_auto_merge_state(
        &preferred_forge_user,
        &forge_repo_info.context("No forge could be determined for this repository branch")?,
        review_id,
        enable,
        &storage,
    )
    .await
}

/// Set a review to draft or ready-for-review
#[but_api]
#[instrument(err(Debug))]
pub async fn set_review_draftiness(
    ctx: ThreadSafeContext,
    review_id: usize,
    draft: bool,
) -> Result<()> {
    let (storage, forge_repo_info, preferred_forge_user) = {
        let ctx = ctx.into_thread_local();
        let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
        let forge_repo_info = but_forge::derive_forge_repo_info(&base_branch.remote_url);

        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            forge_repo_info,
            ctx.legacy_project.preferred_forge_user.clone(),
        )
    };

    but_forge::set_review_draftiness(
        &preferred_forge_user,
        &forge_repo_info.context("No forge could be determined for this repository branch")?,
        review_id,
        draft,
        &storage,
    )
    .await
}

/// Update the stacked review descriptions to have the correct footers.
#[but_api]
#[instrument(err(Debug))]
pub async fn update_review_footers(
    ctx: ThreadSafeContext,
    reviews: Vec<but_forge::ForgeReviewDescriptionUpdate>,
) -> Result<()> {
    let (storage, forge_repo_info, preferred_forge_user) = {
        let ctx = ctx.into_thread_local();
        let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
        let forge_repo_info = but_forge::derive_forge_repo_info(&base_branch.remote_url);

        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            forge_repo_info,
            ctx.legacy_project.preferred_forge_user.clone(),
        )
    };

    but_forge::update_review_description_tables(
        &preferred_forge_user,
        &forge_repo_info.context("No forge could be determined for this repository branch")?,
        &reviews,
        &storage,
    )
    .await
}

#[but_api]
#[instrument(err(Debug))]
pub async fn list_reviews_for_branch(
    ctx: ThreadSafeContext,
    branch: String,
    filter: Option<but_forge::ForgeReviewFilter>,
) -> Result<Vec<but_forge::ForgeReview>> {
    let (storage, forge_repo_info, project) = {
        let ctx = ctx.into_thread_local();
        let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
        let forge_repo_info = but_forge::derive_forge_repo_info(&base_branch.remote_url);
        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            forge_repo_info,
            ctx.legacy_project.clone(),
        )
    };

    but_forge::list_forge_reviews_for_branch(
        project.preferred_forge_user,
        &forge_repo_info.context("No forge could be determined for this repository branch")?,
        &branch,
        &storage,
        filter,
    )
    .await
}

/// Warm up the CI checks cache for all applied branches with PRs.
/// This function fetches CI check data from the forge and caches it in the database
/// without returning any data. It only processes branches that have associated pull requests.
/// Additionally, it cleans up stale CI check entries for references that are no longer
/// part of any applied stack.
#[but_api]
#[instrument(err(Debug))]
pub fn warm_ci_checks_cache(ctx: &mut Context) -> Result<()> {
    // Get all stacks
    let stacks = crate::legacy::workspace::stacks(ctx, None)?;

    // Collect branch references that have CI checks cached
    let mut current_refs = std::collections::HashSet::new();

    // For each stack, get details and check branches
    for stack in stacks {
        if let Some(stack_id) = stack.id {
            let details = crate::legacy::workspace::stack_details(ctx, Some(stack_id))?;

            // Process each branch that has a PR
            for branch in &details.branch_details {
                if branch.pr_number.is_some() {
                    // Fetch CI checks with NoCache to force refresh
                    let _ = list_ci_checks_and_update_cache(
                        ctx,
                        branch.name.to_string(),
                        Some(but_forge::CacheConfig::NoCache),
                    );
                    // Ignore errors for individual branches to ensure we process all branches

                    // Track this reference as having CI checks
                    current_refs.insert(branch.name.to_string());
                }
            }
        }
    }

    // Clean up stale CI check entries from the database
    let db = &mut *ctx.db.get_mut()?;
    let all_cached_refs = db.ci_checks().list_all_references()?;

    // Delete CI checks for references that are no longer in applied stacks
    for cached_ref in all_cached_refs {
        if !current_refs.contains(&cached_ref) {
            db.ci_checks_mut()?.delete_for_reference(&cached_ref)?;
        }
    }

    Ok(())
}
