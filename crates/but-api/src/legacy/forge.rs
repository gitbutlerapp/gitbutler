//! In place of commands.rs
use anyhow::{Context as _, Result};
use but_api_macros::but_api;
use but_core::RepositoryExt;
use but_ctx::Context;
use but_forge::{
    ForgeName, ReviewTemplateFunctions, available_review_templates, get_review_template_functions,
};
use gitbutler_project::ProjectId;
use gitbutler_repo::RepoCommands;
use tracing::instrument;

/// (Deprecated) Get the list of PR template paths for the given project and forge.
/// This function is deprecated in favor of `list_available_review_templates`.
#[but_api]
#[instrument(err(Debug))]
pub fn pr_templates(project_id: ProjectId, forge: ForgeName) -> Result<Vec<String>> {
    let project = gitbutler_project::get_validated(project_id)?;
    Ok(available_review_templates(project.worktree_dir()?, &forge))
}

/// Get the list of review template paths for the given project.
#[but_api]
#[instrument(err(Debug))]
pub fn list_available_review_templates(project_id: ProjectId) -> Result<Vec<String>> {
    let project = gitbutler_project::get_validated(project_id)?;
    let ctx = Context::new_from_legacy_project(project.clone())?;
    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
    let forge = &base_branch
        .forge_repo_info
        .as_ref()
        .context("No forge could be determined for this repository branch")?
        .forge;

    Ok(available_review_templates(project.worktree_dir()?, forge))
}

/// (Deprecated) Get the PR template content for the given project and relative path.
///
/// This function is deprecated in favor of `review_template`, which serves the same purpose
/// but uses the updated storage location.
#[but_api]
#[instrument(err(Debug))]
pub fn pr_template(
    project_id: ProjectId,
    relative_path: std::path::PathBuf,
    forge: ForgeName,
) -> Result<String> {
    let project = gitbutler_project::get_validated(project_id)?;

    let ReviewTemplateFunctions {
        is_valid_review_template_path,
        ..
    } = get_review_template_functions(&forge);

    if !is_valid_review_template_path(&relative_path) {
        return Err(anyhow::format_err!(
            "Invalid review template path: {:?}",
            project.worktree_dir()?.join(relative_path),
        ));
    }
    project
        .read_file_from_workspace(&relative_path)?
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
pub fn review_template(project_id: ProjectId) -> Result<Option<json::ReviewTemplateInfo>> {
    let project = gitbutler_project::get_validated(project_id)?;
    let ctx = Context::new_from_legacy_project(project.clone())?;
    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
    let forge = &base_branch
        .forge_repo_info
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
                    project.worktree_dir()?.join(path),
                ));
            }
            let content = project
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
pub fn set_review_template(project_id: ProjectId, template_path: Option<String>) -> Result<()> {
    let project = gitbutler_project::get_validated(project_id)?;
    let repo = project.open_isolated_repo()?;
    let mut git_config = repo.git_settings()?;

    let ctx = Context::new_from_legacy_project(project.clone())?;
    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
    let forge = &base_branch
        .forge_repo_info
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
            return Err(anyhow::format_err!(
                "Invalid review template path: {:?}",
                project.worktree_dir()?.join(&path_buf),
            ));
        }
    }

    git_config.gitbutler_forge_review_template_path = template_path.map(|p| p.into());
    repo.set_git_settings(&git_config)
}

#[but_api]
#[instrument(err(Debug))]
pub fn determine_forge_from_url(url: String) -> Result<Option<ForgeName>> {
    Ok(but_forge::determine_forge_from_url(&url))
}

#[but_api]
#[instrument(err(Debug))]
pub fn list_reviews(
    project_id: ProjectId,
    cache_config: Option<but_forge::CacheConfig>,
) -> Result<Vec<but_forge::ForgeReview>> {
    let mut ctx = Context::new_from_legacy_project_id(project_id)?;
    let (storage, base_branch, project) = {
        let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            base_branch,
            ctx.legacy_project,
        )
    };
    let db = &mut *ctx.db.get_mut()?;
    but_forge::list_forge_reviews_with_cache(
        project.preferred_forge_user,
        &base_branch
            .forge_repo_info
            .context("No forge could be determined for this repository branch")?,
        &storage,
        db,
        cache_config,
    )
}

#[but_api]
#[instrument(skip(ctx), err(Debug))]
pub fn list_ci_checks(
    ctx: &mut Context,
    reference: String,
    cache_config: Option<but_forge::CacheConfig>,
) -> Result<Vec<but_forge::CiCheck>> {
    let (storage, base_branch) = {
        let base_branch = gitbutler_branch_actions::base::get_base_branch_data(ctx)?;
        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            base_branch,
        )
    };
    let db = &mut *ctx.db.get_mut()?;
    but_forge::ci_checks_for_ref_with_cache(
        ctx.legacy_project.preferred_forge_user.clone(),
        &base_branch
            .forge_repo_info
            .context("No forge could be determined for this repository branch")?,
        &storage,
        &reference,
        db,
        cache_config,
    )
}

#[but_api]
#[instrument(err(Debug))]
pub async fn publish_review(
    project_id: ProjectId,
    params: but_forge::CreateForgeReviewParams,
) -> Result<but_forge::ForgeReview> {
    let (storage, base_branch, project) = {
        let ctx = Context::new_from_legacy_project_id(project_id)?;
        let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            base_branch,
            ctx.legacy_project,
        )
    };
    but_forge::create_forge_review(
        &project.preferred_forge_user,
        &base_branch
            .forge_repo_info
            .context("No forge could be determined for this repository branch")?,
        &params,
        &storage,
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
pub fn warm_ci_checks_cache(project_id: ProjectId) -> Result<()> {
    let mut ctx = Context::new_from_legacy_project_id(project_id)?;

    // Get all stacks
    let stacks = crate::legacy::workspace::stacks(project_id, None)?;

    // Collect branch references that have CI checks cached
    let mut current_refs = std::collections::HashSet::new();

    // For each stack, get details and check branches
    for stack in stacks {
        if let Some(stack_id) = stack.id {
            let details = crate::legacy::workspace::stack_details(project_id, Some(stack_id))?;

            // Process each branch that has a PR
            for branch in &details.branch_details {
                if branch.pr_number.is_some() {
                    // Fetch CI checks with NoCache to force refresh
                    let _ = list_ci_checks(
                        &mut ctx,
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
            db.ci_checks().delete_for_reference(&cached_ref)?;
        }
    }

    Ok(())
}
