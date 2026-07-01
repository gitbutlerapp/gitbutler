//! In place of commands.rs
use anyhow::{Context as _, Result, bail};
use bstr::ByteSlice;
use but_api_macros::but_api;
use but_core::{
    RefMetadata as _, RepositoryExt,
    git_config::{edit_repo_config, ensure_config_value},
    ref_metadata::ProjectMeta,
};
use but_ctx::{Context, ThreadSafeContext};
use but_forge::{
    ForgeName, ReviewTemplateFunctions, available_review_templates, get_review_template_functions,
};
use gitbutler_git::GitContextExt;
use gitbutler_repo::{FileInfo, RepoCommands};
use tracing::instrument;

pub fn remote_url(project_meta: &ProjectMeta, repo: &gix::Repository) -> Result<String> {
    project_meta.remote_url_with_fallback(repo)
}

pub fn push_remote_url(project_meta: &ProjectMeta, repo: &gix::Repository) -> Result<String> {
    project_meta.push_remote_url(repo)
}

fn review_template_content(file: FileInfo) -> Result<String> {
    if file.size.is_none() {
        return Ok(String::new());
    }
    if !file.is_valid_utf8() {
        anyhow::bail!("PR template exists but must be valid UTF-8 text or markdown");
    }
    Ok(file.content.unwrap_or_default())
}

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
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn forge_provider(ctx: &Context) -> Result<Option<ForgeName>> {
    let project_meta = ctx.project_meta()?;
    let repo = ctx.repo.get()?;
    let forge_repo_info = but_forge::derive_forge_repo_info(&remote_url(&project_meta, &repo)?);
    Ok(forge_repo_info.map(|info| info.forge))
}

/// Per-project forge display + URL config. Lets the renderer build
/// commit/PR URLs and pick labels without branching on forge name.
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn forge_info(ctx: &Context) -> Result<Option<but_forge::ForgeInfo>> {
    let project_meta = ctx.project_meta()?;
    let repo = ctx.repo.get()?;
    Ok(but_forge::forge_info(&remote_url(&project_meta, &repo)?))
}

/// Web compare URL for a branch — drives the "Open in browser"
/// affordances without making the renderer hold per-forge URL
/// templates. `fork` is the owner namespace for fork compares.
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn forge_compare_branch_url(
    ctx: &Context,
    base: String,
    branch: String,
    fork: Option<String>,
) -> Result<Option<String>> {
    let project_meta = ctx.project_meta()?;
    let repo = ctx.repo.get()?;
    Ok(but_forge::compare_branch_url(
        &remote_url(&project_meta, &repo)?,
        &base,
        &branch,
        fork.as_deref(),
    ))
}

/// Get the list of review template paths for the given project.
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn list_available_review_templates(ctx: &Context) -> Result<Vec<String>> {
    let project_meta = ctx.project_meta()?;
    let repo = ctx.repo.get()?;
    let forge_repo_info = but_forge::derive_forge_repo_info(&remote_url(&project_meta, &repo)?);
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
    let file = ctx.read_file_from_workspace(&relative_path)?;
    review_template_content(file)
}

/// Information about the project's review template.
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
pub struct ReviewTemplateInfo {
    /// The relative path to the review template within the repository.
    pub path: String,
    /// The content of the review template.
    pub content: String,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ReviewTemplateInfo);

/// Get the review template content for the given project and relative path.
///
/// This function determines the forge of a project and retrieves the review template
/// from the git config.
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn review_template(ctx: &Context) -> Result<Option<ReviewTemplateInfo>> {
    let project_meta = ctx.project_meta()?;
    let repo = ctx.repo.get()?;
    let forge_repo_info = but_forge::derive_forge_repo_info(&remote_url(&project_meta, &repo)?);
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
            let file = ctx.read_file_from_workspace(&path)?;
            let content = review_template_content(file)?;

            Ok(Some(ReviewTemplateInfo {
                path: template_path,
                content,
            }))
        }
        None => Ok(None),
    }
}

/// Set the review template path in the git configuration for the given project.
/// The template path will be validated.
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn set_review_template(ctx: &but_ctx::Context, template_path: Option<String>) -> Result<()> {
    let repo = ctx.open_isolated_repo()?;
    let mut git_config = repo.git_settings()?;

    let project_meta = ctx.project_meta()?;
    let forge_repo_info = but_forge::derive_forge_repo_info(&remote_url(&project_meta, &repo)?);
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

#[but_api(napi)]
#[instrument(err(Debug))]
pub fn list_reviews(
    ctx: &Context,
    cache_config: Option<but_forge::CacheConfig>,
) -> Result<Vec<but_forge::ForgeReview>> {
    let (storage, forge_repo_info, preferred_forge_user) = {
        let project_meta = ctx.project_meta()?;
        let repo = ctx.repo.get()?;
        let forge_repo_info = but_forge::derive_forge_repo_info(&remote_url(&project_meta, &repo)?);

        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            forge_repo_info,
            ctx.legacy_project.preferred_forge_user.clone(),
        )
    };

    let db = &mut *ctx.db.get_cache_mut()?;

    but_forge::list_forge_reviews_with_cache(
        preferred_forge_user,
        &forge_repo_info.context("No forge could be determined for this repository branch")?,
        &storage,
        db,
        cache_config,
    )
}

/// Applies a forge review by resolving it to its source branch.
///
/// This fetches the review's head repository through a configured or newly
/// created remote, applies the fetched remote-tracking branch, and records the
/// review number on the applied branch metadata.
#[but_api(napi, crate::branch::json::ApplyOutcome)]
#[instrument(err(Debug))]
pub fn review_apply(
    ctx: &mut but_ctx::Context,
    review_id: usize,
) -> Result<but_workspace::branch::apply::Outcome> {
    let (forge_repo_info, preferred_forge_user, target_protocol) = {
        let project_meta = ctx.project_meta()?;
        let repo = ctx.repo.get()?;
        let remote_url = project_meta.remote_url_with_fallback(&repo)?;
        let forge_repo_info = but_forge::derive_forge_repo_info(&remote_url)
            .context("No supported forge could be determined for this repository")?;
        let target_protocol = forge_repo_info.protocol.clone();
        (
            forge_repo_info,
            ctx.legacy_project.preferred_forge_user.clone(),
            target_protocol,
        )
    };

    if forge_repo_info.forge != but_forge::ForgeName::GitHub {
        bail!("Applying reviews is currently only supported for GitHub pull requests");
    }

    let review = {
        let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
        let db = &mut *ctx.db.get_cache_mut()?;
        but_forge::get_forge_review(
            &preferred_forge_user,
            &forge_repo_info,
            review_id,
            db,
            &storage,
        )?
    };

    let head_url = review_head_url(&review, &target_protocol)
        .with_context(|| format!("Review #{review_id} does not include a source repository URL"))?;

    let mut guard = ctx.exclusive_worktree_access();
    let remote_name = ensure_review_remote(ctx, &head_url, &review, review_id)?;
    ctx.fetch(&remote_name, Some("apply review".into()))
        .with_context(|| format!("Failed to fetch review remote '{remote_name}'"))?;
    ctx.reload_repo_and_invalidate_workspace(guard.write_permission())?;

    let remote_ref: gix::refs::FullName =
        format!("refs/remotes/{remote_name}/{}", review.source_branch)
            .try_into()
            .with_context(|| {
                format!(
                    "Review #{} source branch '{}' is not a valid remote-tracking reference",
                    review_id, review.source_branch
                )
            })?;

    let out = crate::branch::apply_with_perm(ctx, remote_ref.as_ref(), guard.write_permission())?;
    if out.status.persisted_mutation()
        && let Some(applied_branch_ref) = out.applied_branches.last()
    {
        let mut meta = ctx.meta()?;
        let mut branch = meta.branch(applied_branch_ref.as_ref())?;
        branch.review.pull_request = Some(review_id);
        meta.set_branch(&branch)?;
        ctx.invalidate_workspace_cache()?;
    }
    Ok(out)
}

fn review_head_url(review: &but_forge::ForgeReview, target_protocol: &str) -> Option<String> {
    let prefers_ssh = target_protocol.eq_ignore_ascii_case("ssh")
        || target_protocol.to_ascii_lowercase().contains("ssh");
    if prefers_ssh {
        review
            .repository_ssh_url
            .clone()
            .or_else(|| review.repository_https_url.clone())
    } else {
        review
            .repository_https_url
            .clone()
            .or_else(|| review.repository_ssh_url.clone())
    }
}

fn ensure_review_remote(
    ctx: &but_ctx::Context,
    remote_url: &str,
    review: &but_forge::ForgeReview,
    review_id: usize,
) -> Result<String> {
    let repo = ctx.open_isolated_repo()?;
    if let Some(existing) = find_remote_by_url(&repo, remote_url)? {
        return Ok(existing);
    }

    let owner_hint = review
        .repo_owner
        .as_deref()
        .or_else(|| review.author.as_ref().map(|author| author.login.as_str()));
    let base_name = sanitize_remote_name(owner_hint.unwrap_or(""), review_id);
    let remote_name = unique_remote_name(&repo, &base_name)?;
    add_remote_to_config(&repo, &remote_name, remote_url)?;
    Ok(remote_name)
}

fn find_remote_by_url(repo: &gix::Repository, remote_url: &str) -> Result<Option<String>> {
    for name in repo.remote_names().iter() {
        let remote = repo.find_remote(name.as_ref())?;
        let Some(url) = remote.url(gix::remote::Direction::Fetch) else {
            continue;
        };
        let configured = url.to_bstring().to_str_lossy().into_owned();
        if remote_urls_match(&configured, remote_url) {
            return Ok(Some(name.to_string()));
        }
    }
    Ok(None)
}

fn remote_urls_match(configured: &str, candidate: &str) -> bool {
    if configured == candidate {
        return true;
    }
    let configured_info = but_forge::derive_forge_repo_info(configured);
    let candidate_info = but_forge::derive_forge_repo_info(candidate);
    configured_info.is_some() && configured_info == candidate_info
}

fn sanitize_remote_name(input: &str, review_id: usize) -> String {
    let mut out = String::new();
    let mut last_was_dash = false;
    for ch in input.chars().flat_map(char::to_lowercase) {
        let safe = ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-');
        let next = if safe { ch } else { '-' };
        if next == '-' {
            if !last_was_dash {
                out.push(next);
            }
            last_was_dash = true;
        } else {
            out.push(next);
            last_was_dash = false;
        }
    }
    let out = out.trim_matches(|ch| matches!(ch, '.' | '_' | '-'));
    if out.is_empty() || out == "head" {
        format!("pr-{review_id}")
    } else {
        out.to_owned()
    }
}

fn unique_remote_name(repo: &gix::Repository, base: &str) -> Result<String> {
    let mut candidate = base.to_owned();
    let mut suffix = 2;
    while repo.find_remote(candidate.as_str()).is_ok() {
        candidate = format!("{base}-{suffix}");
        suffix += 1;
    }
    Ok(candidate)
}

fn add_remote_to_config(repo: &gix::Repository, name: &str, remote_url: &str) -> Result<()> {
    edit_repo_config(repo, gix::config::Source::Local, |config| {
        let mut section = config.section_mut_or_create_new("remote", Some(name.into()))?;
        section.push("url".try_into()?, Some(remote_url.into()));
        ensure_config_value(
            config,
            &format!("remote.{name}.fetch"),
            &format!("+refs/heads/*:refs/remotes/{name}/*"),
        )?;
        Ok(())
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use but_testsupport::{CommandExt, git_at_dir, open_repo};

    #[test]
    fn missing_review_template_returns_empty_content() {
        let content =
            review_template_content(FileInfo::default()).expect("missing template is allowed");

        assert_eq!(content, "");
    }

    #[test]
    fn binary_review_template_errors_as_non_utf8() {
        let err = review_template_content(FileInfo::binary("PULL_REQUEST_TEMPLATE.md".as_ref(), 4))
            .expect_err("binary template must be rejected");

        assert_eq!(
            err.to_string(),
            "PR template exists but must be valid UTF-8 text or markdown"
        );
    }

    #[test]
    fn review_remote_name_is_sanitized() {
        assert_eq!(
            sanitize_remote_name("Alice Cooper!", 42),
            "alice-cooper",
            "forge owner names become git remote-safe names"
        );
        assert_eq!(
            sanitize_remote_name("...", 42),
            "pr-42",
            "empty sanitized names fall back to the review number"
        );
    }

    #[test]
    fn review_without_head_repository_url_has_no_applyable_source() {
        let review = but_forge::ForgeReview {
            html_url: "https://github.com/acme/widgets/pull/42".into(),
            number: 42,
            title: "Fork PR".into(),
            body: None,
            author: None,
            labels: Vec::new(),
            draft: false,
            source_branch: "fork-feature".into(),
            target_branch: "main".into(),
            sha: "0000000000000000000000000000000000000000".into(),
            integration_commit_shas: Vec::new(),
            created_at: None,
            modified_at: None,
            merged_at: None,
            closed_at: None,
            repository_ssh_url: None,
            repository_https_url: None,
            repo_owner: Some("alice".into()),
            head_repo_is_fork: true,
            reviewers: Vec::new(),
            unit_symbol: "#".into(),
            last_sync_at: Default::default(),
        };

        assert!(
            review_head_url(&review, "https").is_none(),
            "reviews without a head repository URL cannot be fetched for apply"
        );
    }

    #[test]
    fn review_remote_name_collision_gets_suffix() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        git_at_dir(tmp.path()).args(["init"]).run();
        git_at_dir(tmp.path())
            .args([
                "remote",
                "add",
                "alice",
                "https://github.com/elsewhere/widgets.git",
            ])
            .run();
        let repo = open_repo(tmp.path())?;

        assert_eq!(
            unique_remote_name(&repo, "alice")?,
            "alice-2",
            "new fork remotes should not overwrite an existing remote"
        );
        Ok(())
    }

    #[test]
    fn review_remote_reuses_matching_remote_by_exact_url() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        git_at_dir(tmp.path()).args(["init"]).run();
        git_at_dir(tmp.path())
            .args(["remote", "add", "alice", "/tmp/alice/widgets.git"])
            .run();
        let repo = open_repo(tmp.path())?;

        assert_eq!(
            find_remote_by_url(&repo, "/tmp/alice/widgets.git")?,
            Some("alice".to_string()),
            "existing exact-url fork remotes should be reused"
        );
        Ok(())
    }

    #[test]
    fn review_remote_reuses_matching_remote_by_forge_identity() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        git_at_dir(tmp.path()).args(["init"]).run();
        git_at_dir(tmp.path())
            .args(["remote", "add", "alice", "git@github.com:alice/widgets.git"])
            .run();
        let repo = open_repo(tmp.path())?;

        assert_eq!(
            find_remote_by_url(&repo, "https://github.com/alice/widgets.git")?,
            Some("alice".to_string()),
            "matching GitHub remotes should be reused across SSH/HTTPS URL forms"
        );
        Ok(())
    }
}

#[but_api(napi)]
#[instrument(err(Debug))]
pub async fn get_review_base_repo_url(
    ctx: ThreadSafeContext,
    review_id: usize,
) -> Result<Option<String>> {
    let (storage, forge_repo_info, preferred_forge_user) = {
        let ctx = ctx.into_thread_local();
        let project_meta = ctx.project_meta()?;
        let repo = ctx.repo.get()?;
        let forge_repo_info = but_forge::derive_forge_repo_info(&remote_url(&project_meta, &repo)?);
        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            forge_repo_info,
            ctx.legacy_project.preferred_forge_user.clone(),
        )
    };
    but_forge::get_review_base_repo_url(
        &preferred_forge_user,
        &forge_repo_info.context("No forge could be determined for this repository branch")?,
        review_id,
        &storage,
    )
    .await
}

#[but_api(napi)]
#[instrument(err(Debug))]
pub async fn get_review_merge_status(
    ctx: ThreadSafeContext,
    review_id: usize,
) -> Result<but_forge::ReviewMergeStatus> {
    let (storage, forge_repo_info, preferred_forge_user) = {
        let ctx = ctx.into_thread_local();
        let project_meta = ctx.project_meta()?;
        let repo = ctx.repo.get()?;
        let forge_repo_info = but_forge::derive_forge_repo_info(&remote_url(&project_meta, &repo)?);
        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            forge_repo_info,
            ctx.legacy_project.preferred_forge_user.clone(),
        )
    };
    but_forge::get_review_merge_status(
        &preferred_forge_user,
        &forge_repo_info.context("No forge could be determined for this repository branch")?,
        review_id,
        &storage,
    )
    .await
}

#[but_api(napi)]
#[instrument(err(Debug))]
pub fn get_review(ctx: &Context, review_id: usize) -> Result<but_forge::ForgeReview> {
    let (storage, forge_repo_info, preferred_forge_user) = {
        let project_meta = ctx.project_meta()?;
        let repo = ctx.repo.get()?;
        let forge_repo_info = but_forge::derive_forge_repo_info(&remote_url(&project_meta, &repo)?)
            .context("No forge could be determined for this repository.")?;

        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            forge_repo_info,
            ctx.legacy_project.preferred_forge_user.clone(),
        )
    };

    let db = &mut *ctx.db.get_cache_mut()?;
    but_forge::get_forge_review(
        &preferred_forge_user,
        &forge_repo_info,
        review_id,
        db,
        &storage,
    )
}

#[but_api(napi)]
#[instrument(err(Debug))]
pub async fn get_repo_info(ctx: ThreadSafeContext) -> Result<but_forge::RepoInfo> {
    let (storage, forge_repo_info, preferred_forge_user) = {
        let ctx = ctx.into_thread_local();
        let project_meta = ctx.project_meta()?;
        let repo_ = ctx.repo.get()?;
        let forge_repo_info =
            but_forge::derive_forge_repo_info(&remote_url(&project_meta, &repo_)?);
        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            forge_repo_info,
            ctx.legacy_project.preferred_forge_user.clone(),
        )
    };
    but_forge::get_repo_info(
        &preferred_forge_user,
        &forge_repo_info.context("No forge could be determined for this repository branch")?,
        &storage,
    )
    .await
}

#[but_api(napi)]
#[instrument(skip(ctx), err(Debug))]
pub fn list_ci_checks(
    ctx: &Context,
    reference: String,
    cache_config: Option<but_forge::CacheConfig>,
) -> Result<Vec<but_forge::CiCheck>> {
    let (storage, forge_repo_info, preferred_forge_user) = {
        let project_meta = ctx.project_meta()?;
        let repo = ctx.repo.get()?;
        let forge_repo_info = but_forge::derive_forge_repo_info(&remote_url(&project_meta, &repo)?);

        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            forge_repo_info,
            ctx.legacy_project.preferred_forge_user.clone(),
        )
    };
    let db = &mut *ctx.db.get_cache_mut()?;

    but_forge::ci_checks_for_ref_with_cache(
        preferred_forge_user,
        &forge_repo_info.context("No forge could be determined for this repository branch")?,
        &storage,
        &reference,
        db,
        cache_config,
    )
}

#[but_api(napi)]
#[instrument(err(Debug))]
pub async fn publish_review(
    ctx: ThreadSafeContext,
    params: but_forge::CreateForgeReviewParams,
) -> Result<but_forge::ForgeReview> {
    let (storage, forge_repo_info, forge_push_repo_info, preferred_forge_user) = {
        let ctx = ctx.into_thread_local();
        let project_meta = ctx.project_meta()?;
        let repo = ctx.repo.get()?;
        let base_remote_url = remote_url(&project_meta, &repo)?;
        let push_remote_url = push_remote_url(&project_meta, &repo)?;
        let forge_repo_info = but_forge::derive_forge_repo_info(&base_remote_url)
            .context("No forge could be determined for this repository branch")?;
        let forge_push_repo_info = if base_remote_url != push_remote_url {
            let info = but_forge::derive_forge_repo_info(&push_remote_url).context(
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
#[but_api(napi)]
#[instrument(err(Debug))]
pub async fn merge_review(
    ctx: ThreadSafeContext,
    review_id: usize,
    merge_method: Option<but_forge::ReviewMergeMethod>,
) -> Result<()> {
    let (storage, forge_repo_info, preferred_forge_user) = {
        let ctx = ctx.into_thread_local();
        let project_meta = ctx.project_meta()?;
        let repo = ctx.repo.get()?;
        let forge_repo_info = but_forge::derive_forge_repo_info(&remote_url(&project_meta, &repo)?);

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
        merge_method,
        &storage,
    )
    .await
}

/// Enable or disable a review's auto-merge.
#[but_api(napi)]
#[instrument(err(Debug))]
pub async fn set_review_auto_merge(
    ctx: ThreadSafeContext,
    review_id: usize,
    enable: bool,
) -> Result<()> {
    let (storage, forge_repo_info, preferred_forge_user) = {
        let ctx = ctx.into_thread_local();
        let project_meta = ctx.project_meta()?;
        let repo = ctx.repo.get()?;
        let forge_repo_info = but_forge::derive_forge_repo_info(&remote_url(&project_meta, &repo)?);

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
#[but_api(napi)]
#[instrument(err(Debug))]
pub async fn set_review_draftiness(
    ctx: ThreadSafeContext,
    review_id: usize,
    draft: bool,
) -> Result<()> {
    let (storage, forge_repo_info, preferred_forge_user) = {
        let ctx = ctx.into_thread_local();
        let project_meta = ctx.project_meta()?;
        let repo = ctx.repo.get()?;
        let forge_repo_info = but_forge::derive_forge_repo_info(&remote_url(&project_meta, &repo)?);

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

/// Update arbitrary fields of a single review (title, body, state, target base).
/// Each `None` leaves that field unchanged on the forge.
#[but_api(napi)]
#[instrument(err(Debug))]
pub async fn update_review(
    ctx: ThreadSafeContext,
    review_id: usize,
    title: Option<String>,
    body: Option<String>,
    state: Option<but_forge::ReviewState>,
    target_base: Option<String>,
) -> Result<()> {
    let (storage, forge_repo_info, preferred_forge_user) = {
        let ctx = ctx.into_thread_local();
        let project_meta = ctx.project_meta()?;
        let repo = ctx.repo.get()?;
        let forge_repo_info = but_forge::derive_forge_repo_info(&remote_url(&project_meta, &repo)?);
        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            forge_repo_info,
            ctx.legacy_project.preferred_forge_user.clone(),
        )
    };

    let update_payload = but_forge::ReviewUpdatePayload::new(title, body, state, target_base);
    but_forge::update_review(
        &preferred_forge_user,
        &forge_repo_info.context("No forge could be determined for this repository branch")?,
        review_id,
        update_payload,
        &storage,
    )
    .await
}

/// Update stacked reviews: description footers and, optionally, target branches.
#[but_api(napi)]
#[instrument(err(Debug))]
pub async fn update_review_footers(
    ctx: ThreadSafeContext,
    reviews: Vec<but_forge::ForgeReviewUpdate>,
) -> Result<()> {
    let (storage, forge_repo_info, preferred_forge_user) = {
        let ctx = ctx.into_thread_local();
        let project_meta = ctx.project_meta()?;
        let repo = ctx.repo.get()?;
        let forge_repo_info = but_forge::derive_forge_repo_info(&remote_url(&project_meta, &repo)?);

        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            forge_repo_info,
            ctx.legacy_project.preferred_forge_user.clone(),
        )
    };

    but_forge::sync_reviews(
        &preferred_forge_user,
        &forge_repo_info.context("No forge could be determined for this repository branch")?,
        &reviews,
        &storage,
    )
    .await
}

#[but_api(napi)]
#[instrument(err(Debug))]
pub async fn list_reviews_for_branch(
    ctx: ThreadSafeContext,
    branch: String,
    filter: Option<but_forge::ForgeReviewFilter>,
) -> Result<Vec<but_forge::ForgeReview>> {
    let (storage, forge_repo_info, project) = {
        let ctx = ctx.into_thread_local();
        let project_meta = ctx.project_meta()?;
        let repo = ctx.repo.get()?;
        let forge_repo_info = but_forge::derive_forge_repo_info(&remote_url(&project_meta, &repo)?);
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
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn warm_ci_checks_cache(ctx: &Context) -> Result<()> {
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
                    let _ = list_ci_checks(
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
    let db = &mut *ctx.db.get_cache_mut()?;
    let all_cached_refs = db.ci_checks().list_all_references()?;

    // Delete CI checks for references that are no longer in applied stacks
    for cached_ref in all_cached_refs {
        if !current_refs.contains(&cached_ref) {
            db.ci_checks_mut()?.delete_for_reference(&cached_ref)?;
        }
    }

    Ok(())
}
