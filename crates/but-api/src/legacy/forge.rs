//! In place of commands.rs
use anyhow::{Context as _, Result};
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

/// Everything a forge endpoint needs per call: account storage, the
/// repository's forge coordinates, and the preferred account.
fn forge_endpoint_context(
    ctx: ThreadSafeContext,
) -> Result<(
    but_forge_storage::Controller,
    but_forge::ForgeRepoInfo,
    Option<but_forge::ForgeUser>,
)> {
    let ctx = ctx.into_thread_local();
    let project_meta = ctx.project_meta()?;
    let repo = ctx.repo.get()?;
    let forge_repo_info = but_forge::derive_forge_repo_info(&remote_url(&project_meta, &repo)?)
        .context("No forge could be determined for this repository branch")?;
    Ok((
        but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
        forge_repo_info,
        ctx.legacy_project.preferred_forge_user.clone(),
    ))
}

/// The name of the target branch within its remote, like `main` for `refs/remotes/origin/main`.
///
/// Errors if no target is set, or if its remote cannot be determined.
pub fn target_short_name(project_meta: &ProjectMeta, repo: &gix::Repository) -> Result<String> {
    let target_ref = project_meta.target_ref_or_err()?;
    but_core::extract_remote_name_and_short_name(target_ref.as_ref(), &repo.remote_names())
        .map(|(_remote_name, short_name)| short_name.to_string())
        .with_context(|| format!("failed to determine remote for branch {target_ref}"))
}

pub fn push_remote_url(project_meta: &ProjectMeta, repo: &gix::Repository) -> Result<String> {
    project_meta.push_remote_url(repo)
}

fn base_and_push_repo_info(
    project_meta: &ProjectMeta,
    repo: &gix::Repository,
) -> Result<(but_forge::ForgeRepoInfo, Option<but_forge::ForgeRepoInfo>)> {
    let base_remote_url = remote_url(project_meta, repo)?;
    let push_remote_url = push_remote_url(project_meta, repo)?;
    let forge_repo_info = but_forge::derive_forge_repo_info(&base_remote_url)
        .context("No forge could be determined for this repository branch")?;
    let forge_push_repo_info = if base_remote_url != push_remote_url {
        Some(
            but_forge::derive_forge_repo_info(&push_remote_url)
                .context("Failed to derive forge information for the push repository")?,
        )
    } else {
        None
    };
    Ok((forge_repo_info, forge_push_repo_info))
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
/// Returns no value when the project has no target yet or its target forge is unknown.
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn forge_provider(ctx: &Context) -> Result<Option<ForgeName>> {
    Ok(forge_info(ctx)?.map(|info| info.name))
}

/// Per-project forge display + URL config. Lets the renderer build
/// commit/PR URLs and pick labels without branching on forge name.
/// Returns no value when the project has no target yet or its target forge is unknown.
#[but_api(napi, provides = [ForgeInfo])]
#[instrument(err(Debug))]
pub fn forge_info(ctx: &Context) -> Result<Option<but_forge::ForgeInfo>> {
    let project_meta = ctx.project_meta()?;
    if project_meta.target_ref.is_none() {
        return Ok(None);
    }
    let repo = ctx.repo.get()?;
    let accounts = but_forge::get_all_forge_accounts()
        .inspect_err(|err| tracing::warn!("failed to load forge accounts: {err:#}"))
        .unwrap_or_default();
    Ok(but_forge::forge_info(
        &remote_url(&project_meta, &repo)?,
        &accounts,
    ))
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
    let accounts = but_forge::get_all_forge_accounts()
        .inspect_err(|err| tracing::warn!("failed to load forge accounts: {err:#}"))
        .unwrap_or_default();
    Ok(but_forge::compare_branch_url(
        &remote_url(&project_meta, &repo)?,
        &base,
        &branch,
        fork.as_deref(),
        &accounts,
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

#[but_api(napi, provides = [Reviews])]
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

    // Typed so the desktop can treat an unrecognized forge as an expected,
    // terminal listing state instead of retrying it on a timer.
    let forge_repo_info = forge_repo_info.context(but_error::Context::new_static(
        but_error::Code::ForgeUnrecognized,
        "No forge could be determined for this repository branch",
    ))?;
    but_forge::list_forge_reviews_with_cache(
        preferred_forge_user,
        &forge_repo_info,
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
    // Record the review as the branch's durable identity. Best-effort like
    // the publish path: the workspace mutation already happened, so a failed
    // metadata write must not fail the apply — nor skip the cache
    // invalidation below. An already-applied branch gets the association
    // too; its outcome carries no applied branch, so the review's own source
    // branch names it.
    let applied_branch = out.applied_branches.last().cloned().or_else(|| {
        // Only for a local branch that actually exists: metadata written for
        // an unknown ref would fabricate a stack entry.
        let name = gix::refs::Category::LocalBranch
            .to_full_name(review.source_branch.as_str())
            .ok()?;
        let exists = ctx
            .repo
            .get()
            .ok()?
            .try_find_reference(name.as_ref())
            .ok()
            .flatten()
            .is_some();
        exists.then_some(name)
    });
    if let Some(branch) = applied_branch {
        persist_review_association(ctx, branch.as_ref(), review_id).ok();
    }
    if out.status.persisted_mutation() {
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
        let remote = repo.find_remote(name)?;
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
        section.push("url", remote_url)?;
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
            auto_merge_enabled: false,
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
    fn persists_review_number_on_the_local_branch() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        git_at_dir(tmp.path()).args(["init"]).run();
        let ctx = but_ctx::Context::from_repo_for_testing(open_repo(tmp.path())?)?
            .with_memory_app_cache();
        let branch_name: gix::refs::FullName = "refs/heads/feature".try_into()?;

        persist_review_association(&ctx, branch_name.as_ref(), 42)?;

        assert_eq!(
            ctx.meta()?
                .branch(branch_name.as_ref())?
                .review
                .pull_request,
            Some(42)
        );
        Ok(())
    }

    #[test]
    fn unchanged_review_targets_do_not_need_pre_push_flattening() {
        let reviews = [
            (
                but_forge::ForgeReviewTargetUpdate {
                    number: 1,
                    target_branch: "main".into(),
                },
                Some("main".into()),
            ),
            (
                but_forge::ForgeReviewTargetUpdate {
                    number: 2,
                    target_branch: "bottom".into(),
                },
                Some("bottom".into()),
            ),
        ];

        assert!(
            review_target_flattening_plan(&reviews).is_none(),
            "an ordinary push should not contact the forge before pushing"
        );
    }

    #[test]
    fn reordered_review_targets_are_temporarily_flattened_to_trunk() {
        let reviews = [
            (
                but_forge::ForgeReviewTargetUpdate {
                    number: 1,
                    target_branch: "main".into(),
                },
                Some("old-bottom".into()),
            ),
            (
                but_forge::ForgeReviewTargetUpdate {
                    number: 2,
                    target_branch: "new-bottom".into(),
                },
                Some("main".into()),
            ),
        ];

        let (trunk, reviews_to_flatten) =
            review_target_flattening_plan(&reviews).expect("the reviewed stack was reordered");
        assert_eq!(trunk, "main");
        assert_eq!(
            reviews_to_flatten,
            std::collections::HashSet::from([1]),
            "only reviews not already targeting trunk need a preparatory update"
        );
    }

    #[test]
    fn unknown_review_targets_do_not_need_pre_push_flattening() {
        let reviews = [
            (
                but_forge::ForgeReviewTargetUpdate {
                    number: 1,
                    target_branch: "main".into(),
                },
                Some("main".into()),
            ),
            (
                but_forge::ForgeReviewTargetUpdate {
                    number: 2,
                    target_branch: "bottom".into(),
                },
                None,
            ),
        ];

        assert!(
            review_target_flattening_plan(&reviews).is_none(),
            "missing cache data must not cause remote mutations before a push"
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
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;
    but_forge::get_review_base_repo_url(
        &preferred_forge_user,
        &forge_repo_info,
        review_id,
        &storage,
    )
    .await
}

/// List the top-level conversation comments on a review, oldest first.
#[but_api(napi, provides = [ReviewComments])]
#[instrument(err(Debug))]
pub async fn list_review_comments(
    ctx: ThreadSafeContext,
    review_id: usize,
) -> Result<Vec<but_forge::ForgeReviewComment>> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;
    but_forge::list_review_comments(&preferred_forge_user, &forge_repo_info, review_id, &storage)
        .await
}

/// List the diff-anchored comment threads on a review, oldest first.
#[but_api(napi, provides = [ReviewThreads])]
#[instrument(err(Debug))]
pub async fn list_review_threads(
    ctx: ThreadSafeContext,
    review_id: usize,
) -> Result<Vec<but_forge::ForgeReviewThread>> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;
    but_forge::list_review_threads(&preferred_forge_user, &forge_repo_info, review_id, &storage)
        .await
}

/// Reply into one of a review's diff-anchored comment threads.
#[but_api(napi, invalidates = [ReviewThreads])]
#[instrument(err(Debug))]
pub async fn create_review_thread_reply(
    ctx: ThreadSafeContext,
    thread_id: String,
    body: String,
) -> Result<but_forge::ForgeReviewThreadComment> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;
    but_forge::create_review_thread_reply(
        &preferred_forge_user,
        &forge_repo_info,
        &thread_id,
        &body,
        &storage,
    )
    .await
}

/// List the individual reactions (with who reacted) on a review itself.
#[but_api(napi, provides = [ReviewReactions])]
#[instrument(err(Debug))]
pub async fn list_review_reactions(
    ctx: ThreadSafeContext,
    review_id: usize,
) -> Result<Vec<but_forge::ForgeReviewReaction>> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;
    but_forge::list_review_reactions(&preferred_forge_user, &forge_repo_info, review_id, &storage)
        .await
}

/// List the individual reactions (with who reacted) on one comment.
#[but_api(napi, provides = [CommentReactions])]
#[instrument(err(Debug))]
pub async fn list_comment_reactions(
    ctx: ThreadSafeContext,
    comment_id: i64,
) -> Result<Vec<but_forge::ForgeReviewReaction>> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;
    but_forge::list_comment_reactions(
        &preferred_forge_user,
        &forge_repo_info,
        comment_id,
        &storage,
    )
    .await
}

/// Add the caller's reaction to a review itself.
#[but_api(napi, invalidates = [ReviewReactions])]
#[instrument(err(Debug))]
pub async fn add_review_reaction(
    ctx: ThreadSafeContext,
    review_id: usize,
    kind: String,
) -> Result<but_forge::ForgeReviewReaction> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;
    but_forge::add_review_reaction(
        &preferred_forge_user,
        &forge_repo_info,
        review_id,
        &kind,
        &storage,
    )
    .await
}

/// Remove one of the caller's reactions from a review itself.
#[but_api(napi, invalidates = [ReviewReactions])]
#[instrument(err(Debug))]
pub async fn remove_review_reaction(
    ctx: ThreadSafeContext,
    review_id: usize,
    reaction_id: i64,
) -> Result<()> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;
    but_forge::remove_review_reaction(
        &preferred_forge_user,
        &forge_repo_info,
        review_id,
        reaction_id,
        &storage,
    )
    .await
}

/// Add the caller's reaction to one comment.
#[but_api(napi, invalidates = [CommentReactions, ReviewComments])]
#[instrument(err(Debug))]
pub async fn add_comment_reaction(
    ctx: ThreadSafeContext,
    comment_id: i64,
    kind: String,
) -> Result<but_forge::ForgeReviewReaction> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;
    but_forge::add_comment_reaction(
        &preferred_forge_user,
        &forge_repo_info,
        comment_id,
        &kind,
        &storage,
    )
    .await
}

/// Remove one of the caller's reactions from one comment.
#[but_api(napi, invalidates = [CommentReactions, ReviewComments])]
#[instrument(err(Debug))]
pub async fn remove_comment_reaction(
    ctx: ThreadSafeContext,
    comment_id: i64,
    reaction_id: i64,
) -> Result<()> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;
    but_forge::remove_comment_reaction(
        &preferred_forge_user,
        &forge_repo_info,
        comment_id,
        reaction_id,
        &storage,
    )
    .await
}

/// List the pushed commits and review requests on a review's timeline.
#[but_api(napi, provides = [ReviewTimeline])]
#[instrument(err(Debug))]
pub async fn list_review_timeline_events(
    ctx: ThreadSafeContext,
    review_id: usize,
) -> Result<Vec<but_forge::ForgeReviewTimelineEvent>> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;
    but_forge::list_review_timeline_events(
        &preferred_forge_user,
        &forge_repo_info,
        review_id,
        &storage,
    )
    .await
}

/// List the submitted reviews (approvals, change requests) on a review.
#[but_api(napi, provides = [ReviewSubmissions])]
#[instrument(err(Debug))]
pub async fn list_review_submissions(
    ctx: ThreadSafeContext,
    review_id: usize,
) -> Result<Vec<but_forge::ForgeReviewSubmission>> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;
    but_forge::list_review_submissions(&preferred_forge_user, &forge_repo_info, review_id, &storage)
        .await
}

/// Edit a top-level conversation comment on a review.
#[but_api(napi, invalidates = [ReviewComments])]
#[instrument(err(Debug))]
pub async fn update_review_comment(
    ctx: ThreadSafeContext,
    comment_id: i64,
    body: String,
) -> Result<but_forge::ForgeReviewComment> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;
    but_forge::update_review_comment(
        &preferred_forge_user,
        &forge_repo_info,
        comment_id,
        &body,
        &storage,
    )
    .await
}

/// Delete a top-level conversation comment on a review.
#[but_api(napi, invalidates = [ReviewComments])]
#[instrument(err(Debug))]
pub async fn delete_review_comment(ctx: ThreadSafeContext, comment_id: i64) -> Result<()> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;
    but_forge::delete_review_comment(
        &preferred_forge_user,
        &forge_repo_info,
        comment_id,
        &storage,
    )
    .await
}

/// The login this project's forge calls authenticate as, if any account is
/// configured. Resolved from stored accounts; no network.
#[but_api(napi, provides = [ForgeLogin])]
#[instrument(err(Debug))]
pub fn current_forge_login(ctx: &Context) -> Result<Option<String>> {
    let project_meta = ctx.project_meta()?;
    let repo = ctx.repo.get()?;
    let Some(forge_repo_info) =
        but_forge::derive_forge_repo_info(&remote_url(&project_meta, &repo)?)
    else {
        return Ok(None);
    };
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_forge::current_forge_login(
        &ctx.legacy_project.preferred_forge_user,
        &forge_repo_info,
        &storage,
    )
}

/// List the labels defined on the repository backing this project's reviews.
#[but_api(napi, provides = [RepoLabels])]
#[instrument(err(Debug))]
pub async fn list_repo_labels(ctx: ThreadSafeContext) -> Result<Vec<but_forge::ForgeReviewLabel>> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;
    but_forge::list_repo_labels(&preferred_forge_user, &forge_repo_info, &storage).await
}

/// Add labels to a review; returns the resulting label set.
#[but_api(napi, invalidates = [Reviews])]
#[instrument(err(Debug))]
pub async fn add_review_labels(
    ctx: ThreadSafeContext,
    review_id: usize,
    labels: Vec<String>,
) -> Result<Vec<but_forge::ForgeReviewLabel>> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;
    but_forge::add_review_labels(
        &preferred_forge_user,
        &forge_repo_info,
        review_id,
        &labels,
        &storage,
    )
    .await
}

/// Remove one label from a review.
#[but_api(napi, invalidates = [Reviews])]
#[instrument(err(Debug))]
pub async fn remove_review_label(
    ctx: ThreadSafeContext,
    review_id: usize,
    label: String,
) -> Result<()> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;
    but_forge::remove_review_label(
        &preferred_forge_user,
        &forge_repo_info,
        review_id,
        &label,
        &storage,
    )
    .await
}

/// List users who can be requested to review on this project's repository.
#[but_api(napi, provides = [ReviewerCandidates])]
#[instrument(err(Debug))]
pub async fn list_reviewer_candidates(
    ctx: ThreadSafeContext,
) -> Result<Vec<but_forge::ForgeReviewUser>> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;
    but_forge::list_reviewer_candidates(&preferred_forge_user, &forge_repo_info, &storage).await
}

/// Request reviews from the given users on a review.
#[but_api(napi, invalidates = [Reviews, ReviewTimeline])]
#[instrument(err(Debug))]
pub async fn request_review(
    ctx: ThreadSafeContext,
    review_id: usize,
    logins: Vec<String>,
) -> Result<()> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;
    but_forge::request_review(
        &preferred_forge_user,
        &forge_repo_info,
        review_id,
        &logins,
        &storage,
    )
    .await
}

/// Withdraw review requests for the given users on a review.
#[but_api(napi, invalidates = [Reviews])]
#[instrument(err(Debug))]
pub async fn withdraw_review_request(
    ctx: ThreadSafeContext,
    review_id: usize,
    logins: Vec<String>,
) -> Result<()> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;
    but_forge::withdraw_review_request(
        &preferred_forge_user,
        &forge_repo_info,
        review_id,
        &logins,
        &storage,
    )
    .await
}

/// Post a top-level conversation comment on a review.
#[but_api(napi, invalidates = [ReviewComments])]
#[instrument(err(Debug))]
pub async fn create_review_comment(
    ctx: ThreadSafeContext,
    review_id: usize,
    body: String,
) -> Result<but_forge::ForgeReviewComment> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;
    but_forge::create_review_comment(
        &preferred_forge_user,
        &forge_repo_info,
        review_id,
        &body,
        &storage,
    )
    .await
}

#[but_api(napi, provides = [MergeStatus])]
#[instrument(err(Debug))]
pub async fn get_review_merge_status(
    ctx: ThreadSafeContext,
    review_id: usize,
) -> Result<but_forge::ReviewMergeStatus> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;
    but_forge::get_review_merge_status(&preferred_forge_user, &forge_repo_info, review_id, &storage)
        .await
}

#[but_api(napi, provides = [Reviews])]
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

#[but_api(napi, provides = [RepoInfo])]
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

#[but_api(napi, provides = [Checks])]
#[instrument(skip(ctx), err(Debug))]
pub fn list_ci_checks(
    ctx: &Context,
    reference: String,
    cache_config: Option<but_forge::CacheConfig>,
) -> Result<Vec<but_forge::CiCheck>> {
    list_ci_checks_for_ref(ctx, &reference, cache_config)
}

pub fn list_ci_checks_for_ref(
    ctx: &Context,
    reference: &str,
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
        reference,
        db,
        cache_config,
    )
}

#[but_api(napi, invalidates = [Reviews, Branches, Workspace])]
#[instrument(err(Debug))]
pub async fn publish_review(
    ctx: ThreadSafeContext,
    params: PublishReviewInput,
) -> Result<but_forge::PublishReviewOutcome> {
    let branch = gix::refs::Category::LocalBranch.to_full_name(params.local_branch.as_str())?;
    let target_branch = {
        let ctx = ctx.clone().into_thread_local();
        review_creation_target(&ctx, branch.as_ref())?
    };
    let params = but_forge::CreateForgeReviewParams {
        title: params.title,
        body: params.body,
        source_branch: params.source_branch,
        target_branch,
        draft: params.draft,
    };
    let review = publish_review_only(ctx.clone(), branch.clone(), params).await?;
    let review_sync = sync_review_stack_after_review_creation(ctx, branch).await;
    Ok(but_forge::PublishReviewOutcome {
        review,
        review_sync,
    })
}

/// Parameters for creating a review through the public API.
///
/// `local_branch` identifies the workspace ref whose reviewed stack determines the target branch.
/// `source_branch` is the possibly-different remote head sent to the forge.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct PublishReviewInput {
    pub title: String,
    pub body: String,
    pub local_branch: String,
    pub source_branch: String,
    pub draft: bool,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(PublishReviewInput);

/// Create, cache, and persist a branch's review without synchronizing the
/// surrounding reviewed ref slice.
///
/// Batch review creation uses this primitive and synchronizes once after all reviews exist.
pub async fn publish_review_only(
    ctx: ThreadSafeContext,
    local_branch: gix::refs::FullName,
    params: but_forge::CreateForgeReviewParams,
) -> Result<but_forge::ForgeReview> {
    // Kept for local persistence after the (non-`Send`) forge call.
    let local_ctx = ctx.clone();
    let (storage, forge_repo_info, forge_push_repo_info, preferred_forge_user) = {
        let ctx = ctx.into_thread_local();
        let project_meta = ctx.project_meta()?;
        let repo = ctx.repo.get()?;
        let (forge_repo_info, forge_push_repo_info) =
            base_and_push_repo_info(&project_meta, &repo)?;

        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            forge_repo_info,
            forge_push_repo_info,
            ctx.legacy_project.preferred_forge_user.clone(),
        )
    };

    let review = but_forge::create_forge_review(
        &preferred_forge_user,
        &forge_repo_info,
        &forge_push_repo_info,
        &params,
        &storage,
    )
    .await?;

    // Optimistically insert the new review into the cache so the next projection
    // shows it immediately, rather than waiting for a full review-list sync. The
    // returned review's `source_branch` is the forge's own head-ref name, which
    // is exactly the key the projection resolver matches against. Best-effort:
    // a failed insert only delays the association until the next sync.
    {
        let mut ctx = local_ctx.into_thread_local();
        if let Ok(mut db) = ctx.db.get_cache_mut() {
            but_forge::cache_review(&mut db, &review).ok();
        }
        // Best-effort like the cache insert: the review exists on the forge
        // either way, and failing here would abort batch publishing after PRs
        // were already created. Until persisted, the cache fallback carries
        // the association. Metadata writes take the worktree guard so a
        // concurrent mutation's metadata snapshot cannot clobber this one;
        // the guard scope is this local write only — never the forge call
        // above — and callers must not already hold worktree access when
        // awaiting this function.
        if let Ok(review_number) = usize::try_from(review.number) {
            let _guard = ctx.exclusive_worktree_access();
            persist_review_association(&ctx, local_branch.as_ref(), review_number).ok();
        }
    }

    Ok(review)
}

/// Record `review_number` as the branch's review identity. The stored
/// `review_id` is left untouched: it identifies a GitButler review, which
/// publishing a forge review does not supersede.
fn persist_review_association(
    ctx: &Context,
    branch_name: &gix::refs::FullNameRef,
    review_number: usize,
) -> Result<()> {
    let mut meta = ctx.meta()?;
    let mut branch = meta.branch(branch_name)?;
    branch.review.pull_request = Some(review_number);
    meta.set_branch(&branch)
}

/// Merge a review on the forge.
#[but_api(napi, invalidates = [Reviews, MergeStatus, Checks, Branches])]
#[instrument(err(Debug))]
pub async fn merge_review(
    ctx: ThreadSafeContext,
    review_id: usize,
    merge_method: Option<but_forge::ReviewMergeMethod>,
) -> Result<()> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;

    but_forge::merge_review(
        &preferred_forge_user,
        &forge_repo_info,
        review_id,
        merge_method,
        &storage,
    )
    .await
}

/// Enable or disable a review's auto-merge.
#[but_api(napi, invalidates = [Reviews])]
#[instrument(err(Debug))]
pub async fn set_review_auto_merge(
    ctx: ThreadSafeContext,
    review_id: usize,
    enable: bool,
) -> Result<()> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;

    but_forge::set_review_auto_merge_state(
        &preferred_forge_user,
        &forge_repo_info,
        review_id,
        enable,
        &storage,
    )
    .await
}

/// Set a review to draft or ready-for-review
#[but_api(napi, invalidates = [Reviews, MergeStatus])]
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
#[but_api(napi, invalidates = [Reviews])]
#[instrument(err(Debug))]
pub async fn update_review(
    ctx: ThreadSafeContext,
    review_id: usize,
    title: Option<String>,
    body: Option<String>,
    state: Option<but_forge::ReviewState>,
    target_base: Option<String>,
) -> Result<()> {
    let (storage, forge_repo_info, preferred_forge_user) = forge_endpoint_context(ctx)?;

    let update_payload = but_forge::ReviewUpdatePayload::new(title, body, state, target_base);
    but_forge::update_review(
        &preferred_forge_user,
        &forge_repo_info,
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
    let cache_ctx = ctx.clone();
    let (
        storage,
        forge_repo_info,
        forge_push_repo_info,
        preferred_forge_user,
        description_mode,
        github_stacking_mode,
    ) = {
        let ctx = ctx.into_thread_local();
        let project_meta = ctx.project_meta()?;
        let repo = ctx.repo.get()?;
        let (forge_repo_info, forge_push_repo_info) =
            base_and_push_repo_info(&project_meta, &repo)?;
        let settings = repo.git_settings()?;
        let description_mode = match settings.gitbutler_review_stacking_description {
            Some(but_core::ReviewStackingDescription::Top) => {
                but_forge::ReviewStackingDescription::Top
            }
            Some(but_core::ReviewStackingDescription::Disabled) => {
                but_forge::ReviewStackingDescription::Disabled
            }
            Some(but_core::ReviewStackingDescription::Bottom) | None => {
                but_forge::ReviewStackingDescription::Bottom
            }
        };
        let github_stacking_mode = match settings.gitbutler_github_stacking_mode {
            Some(but_core::GitHubStackingMode::Native) => but_forge::GitHubStackingMode::Native,
            Some(but_core::GitHubStackingMode::Disabled) => but_forge::GitHubStackingMode::Disabled,
            Some(but_core::GitHubStackingMode::Auto) | None => but_forge::GitHubStackingMode::Auto,
        };

        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            forge_repo_info,
            forge_push_repo_info,
            ctx.legacy_project.preferred_forge_user.clone(),
            description_mode,
            github_stacking_mode,
        )
    };

    but_forge::sync_reviews(
        &preferred_forge_user,
        &forge_repo_info,
        &forge_push_repo_info,
        &reviews,
        &storage,
        description_mode,
        github_stacking_mode,
    )
    .await?;

    let ctx = cache_ctx.into_thread_local();
    cache_review_target_updates(&ctx, &reviews)
}

fn cache_review_target_updates(
    ctx: &Context,
    updates: &[but_forge::ForgeReviewUpdate],
) -> Result<()> {
    let targets = updates
        .iter()
        .filter_map(|update| {
            update
                .target_branch
                .as_ref()
                .map(|target| (update.number, target))
        })
        .collect::<std::collections::HashMap<_, _>>();
    if targets.is_empty() {
        return Ok(());
    }

    let cached_reviews = {
        let db = ctx.db.get_cache()?;
        but_forge::list_cached_forge_reviews(&db)?
    };
    let mut db = ctx.db.get_cache_mut()?;
    for mut review in cached_reviews {
        let Some(target) = targets.get(&review.number) else {
            continue;
        };
        review.target_branch = (*target).clone();
        but_forge::cache_review(&mut db, &review)?;
    }
    Ok(())
}

/// Prepare native forge state before temporarily flattening review targets for a reordered push.
///
/// Returns the ordered memberships of any native GitHub stacks that had to be dissolved, so a
/// failed push can restore them.
pub(crate) async fn prepare_review_target_updates(
    ctx: ThreadSafeContext,
    reviews: Vec<but_forge::ForgeReviewTargetUpdate>,
) -> Result<Vec<Vec<i64>>> {
    let (
        storage,
        forge_repo_info,
        forge_push_repo_info,
        preferred_forge_user,
        github_stacking_mode,
    ) = {
        let ctx = ctx.into_thread_local();
        let project_meta = ctx.project_meta()?;
        let repo = ctx.repo.get()?;
        let (forge_repo_info, forge_push_repo_info) =
            base_and_push_repo_info(&project_meta, &repo)?;
        let github_stacking_mode = match repo.git_settings()?.gitbutler_github_stacking_mode {
            Some(but_core::GitHubStackingMode::Native) => but_forge::GitHubStackingMode::Native,
            Some(but_core::GitHubStackingMode::Disabled) => but_forge::GitHubStackingMode::Disabled,
            Some(but_core::GitHubStackingMode::Auto) | None => but_forge::GitHubStackingMode::Auto,
        };
        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            forge_repo_info,
            forge_push_repo_info,
            ctx.legacy_project.preferred_forge_user.clone(),
            github_stacking_mode,
        )
    };
    let reviews = reviews.into_iter().map(Into::into).collect::<Vec<_>>();
    but_forge::prepare_review_target_updates(
        &preferred_forge_user,
        &forge_repo_info,
        &forge_push_repo_info,
        &reviews,
        &storage,
        github_stacking_mode,
    )
    .await
}

/// The original remote targets changed while preparing a reviewed stack for push.
pub(crate) struct ReviewTargetFlattening {
    original_targets: Vec<but_forge::ForgeReviewTargetUpdate>,
    /// Ordered memberships of native GitHub stacks dissolved before flattening.
    dissolved_stacks: Vec<Vec<i64>>,
}

/// Temporarily point a reordered reviewed stack at trunk before its refs are pushed.
///
/// This prevents any stale stacked base from containing a rewritten review head while the remote
/// refs transition to their new topology. The regular post-push synchronization restores the
/// desired stacked targets.
pub(crate) async fn flatten_review_targets_before_push(
    ctx: ThreadSafeContext,
    reviews: &[(but_forge::ForgeReviewTargetUpdate, Option<String>)],
) -> Result<Option<ReviewTargetFlattening>> {
    let Some((trunk, reviews_to_flatten)) = review_target_flattening_plan(reviews) else {
        return Ok(None);
    };

    let desired = reviews
        .iter()
        .map(|(desired, _)| desired.clone())
        .collect::<Vec<_>>();
    let dissolved_stacks = prepare_review_target_updates(ctx.clone(), desired).await?;

    let mut flattening = ReviewTargetFlattening {
        original_targets: Vec::new(),
        dissolved_stacks,
    };
    for (review, current_target) in reviews {
        if !reviews_to_flatten.contains(&review.number) {
            continue;
        }
        let current_target = current_target
            .as_ref()
            .expect("the flattening plan requires every current target");
        let update_result = update_review(
            ctx.clone(),
            review
                .number
                .try_into()
                .context("Review number cannot be represented as usize")?,
            None,
            None,
            None,
            Some(trunk.clone()),
        )
        .await;
        if let Err(err) = update_result {
            let err = err.context(format!(
                "Failed to temporarily retarget review #{} before pushing reordered refs",
                review.number
            ));
            if let Err(restore_err) = restore_review_targets(ctx, &flattening).await {
                return Err(err.context(format!(
                    "Additionally failed to restore already-retargeted reviews: {restore_err:#}"
                )));
            }
            return Err(err);
        }
        flattening
            .original_targets
            .push(but_forge::ForgeReviewTargetUpdate {
                number: review.number,
                target_branch: current_target.clone(),
            });
    }
    Ok(Some(flattening))
}

pub(crate) async fn restore_review_targets(
    ctx: ThreadSafeContext,
    flattening: &ReviewTargetFlattening,
) -> Result<()> {
    let mut errors = Vec::new();
    for review in &flattening.original_targets {
        if let Err(err) = update_review(
            ctx.clone(),
            review
                .number
                .try_into()
                .context("Review number cannot be represented as usize")?,
            None,
            None,
            None,
            Some(review.target_branch.clone()),
        )
        .await
        {
            errors.push(format!(
                "Failed to restore review #{} to `{}`: {err}",
                review.number, review.target_branch
            ));
        }
    }
    if !errors.is_empty() {
        if !flattening.dissolved_stacks.is_empty() {
            errors.push(
                "Native GitHub stack membership was not restored; the next successful push will \
                 recreate it."
                    .to_string(),
            );
        }
        anyhow::bail!(
            "Could not restore all review targets after the push failed:\n{}",
            errors.join("\n")
        )
    }
    if flattening.dissolved_stacks.is_empty() {
        return Ok(());
    }

    let (storage, forge_repo_info, preferred_forge_user) = {
        let ctx = ctx.into_thread_local();
        let project_meta = ctx.project_meta()?;
        let repo = ctx.repo.get()?;
        let (forge_repo_info, _) = base_and_push_repo_info(&project_meta, &repo)?;
        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            forge_repo_info,
            ctx.legacy_project.preferred_forge_user.clone(),
        )
    };
    but_forge::restore_native_stacks(
        &preferred_forge_user,
        &forge_repo_info,
        &flattening.dissolved_stacks,
        &storage,
    )
    .await
}

fn review_target_flattening_plan(
    reviews: &[(but_forge::ForgeReviewTargetUpdate, Option<String>)],
) -> Option<(String, std::collections::HashSet<i64>)> {
    if reviews.iter().any(|(_, current)| current.is_none()) {
        return None;
    }
    let targets_changed = reviews
        .iter()
        .any(|(desired, current)| current.as_deref() != Some(desired.target_branch.as_str()));
    if !targets_changed {
        return None;
    }

    let trunk = reviews.first()?.0.target_branch.clone();
    let reviews_to_flatten = reviews
        .iter()
        .filter(|(_, current)| current.as_deref() != Some(trunk.as_str()))
        .map(|(review, _)| review.number)
        .collect();
    Some((trunk, reviews_to_flatten))
}

/// Synchronize every review in the workspace stack containing `branch`.
///
/// The branch identifies the affected stack, but does not bound the reviews that are updated.
/// Failures are returned as a partial-success outcome because callers invoke this only after an
/// irreversible operation.
pub async fn sync_review_stack_for_branch(
    ctx: ThreadSafeContext,
    branch: gix::refs::FullName,
) -> but_forge::ReviewSyncOutcome {
    let updates = {
        let ctx = ctx.clone().into_thread_local();
        review_updates_for_branch(&ctx, branch.as_ref())
    };
    sync_review_updates(ctx, updates).await
}

async fn sync_review_updates(
    ctx: ThreadSafeContext,
    updates: Result<Vec<but_forge::ForgeReviewUpdate>>,
) -> but_forge::ReviewSyncOutcome {
    sync_review_update_groups(ctx, updates.map(|updates| vec![updates])).await
}

async fn sync_review_update_groups(
    ctx: ThreadSafeContext,
    update_groups: Result<Vec<Vec<but_forge::ForgeReviewUpdate>>>,
) -> but_forge::ReviewSyncOutcome {
    let update_groups = match update_groups {
        Ok(update_groups) => update_groups,
        Err(err) => {
            return but_forge::ReviewSyncOutcome::Failed {
                message: err.to_string(),
            };
        }
    };

    let mut updated = false;
    let mut errors = Vec::new();
    for (index, updates) in update_groups.into_iter().enumerate() {
        if updates.is_empty() {
            continue;
        }
        updated = true;
        if let Err(err) = update_review_footers(ctx.clone(), updates).await {
            errors.push(format!("Review stack {}: {err}", index + 1));
        }
    }
    if !errors.is_empty() {
        but_forge::ReviewSyncOutcome::Failed {
            message: errors.join("\n"),
        }
    } else if updated {
        but_forge::ReviewSyncOutcome::Succeeded
    } else {
        but_forge::ReviewSyncOutcome::NotNeeded
    }
}

/// Synchronize every review stack affected by a successful push.
///
/// Updating only unreviewed refs cannot change review topology and does not contact the forge.
/// A reviewed ref that was connected to a pushed ref immediately before the push is also affected:
/// it may have moved into another workspace stack and need its old stack metadata removed.
pub async fn sync_review_stacks_after_push(
    ctx: ThreadSafeContext,
    branch: gix::refs::FullName,
    pushed_branches: Vec<(String, String, String)>,
) -> but_forge::ReviewSyncOutcome {
    let updates = {
        let ctx = ctx.clone().into_thread_local();
        review_updates_after_push(&ctx, branch.as_ref(), &pushed_branches)
    };
    sync_review_update_groups(ctx, updates).await
}

/// Find the local ref associated with `review_number` and synchronize its complete review stack.
pub async fn sync_review_stack_for_review(
    ctx: ThreadSafeContext,
    review_number: i64,
) -> but_forge::ReviewSyncOutcome {
    let branch = {
        let ctx = ctx.clone().into_thread_local();
        local_branch_for_review(&ctx, review_number)
    };
    match branch {
        Ok(branch) => sync_review_stack_for_branch(ctx, branch).await,
        Err(err) => but_forge::ReviewSyncOutcome::Failed {
            message: err.to_string(),
        },
    }
}

/// Synchronize the complete workspace review stack containing a newly-created review.
pub async fn sync_review_stack_after_review_creation(
    ctx: ThreadSafeContext,
    branch: gix::refs::FullName,
) -> but_forge::ReviewSyncOutcome {
    sync_review_stack_for_branch(ctx, branch).await
}

fn review_updates_after_push(
    ctx: &Context,
    branch: &gix::refs::FullNameRef,
    pushed_branches: &[(String, String, String)],
) -> Result<Vec<Vec<but_forge::ForgeReviewUpdate>>> {
    let info = crate::legacy::workspace::head_info(ctx)?;
    let selected_stack_index = info
        .stacks
        .iter()
        .position(|stack| {
            stack.segments.iter().any(|segment| {
                segment
                    .ref_info
                    .as_ref()
                    .is_some_and(|ref_info| ref_info.ref_name == branch)
            })
        })
        .with_context(|| {
            format!(
                "Branch `{}` is not part of the current workspace",
                branch.shorten()
            )
        })?;

    let open_reviews = open_review_numbers(ctx)?;
    let reviewed_branches = info
        .stacks
        .iter()
        .flat_map(|stack| &stack.segments)
        .filter(|segment| review_number(segment, &open_reviews).is_some())
        .filter_map(|segment| {
            segment
                .ref_info
                .as_ref()?
                .ref_name
                .shorten()
                .to_str()
                .ok()
                .map(ToOwned::to_owned)
        })
        .collect::<std::collections::HashSet<_>>();
    let previous_review_tips = pushed_branches
        .iter()
        .filter(|(name, _, _)| reviewed_branches.contains(name))
        .map(|(_, before, _)| before.parse::<gix::ObjectId>())
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|tip| !tip.is_null())
        .collect::<Vec<_>>();
    if previous_review_tips.is_empty() {
        return Ok(Vec::new());
    }

    let repo = ctx.repo.get()?;
    let mut affected_stack_indices = std::collections::BTreeSet::from([selected_stack_index]);
    for (stack_index, stack) in info.stacks.iter().enumerate() {
        'segments: for segment in &stack.segments {
            if review_number(segment, &open_reviews).is_none() {
                continue;
            }
            let Ok(head) = remote_head(&repo, segment) else {
                continue;
            };
            for previous_tip in &previous_review_tips {
                if remote_contains(&repo, *previous_tip, head.remote_tip)?
                    || remote_contains(&repo, head.remote_tip, *previous_tip)?
                {
                    affected_stack_indices.insert(stack_index);
                    break 'segments;
                }
            }
        }
    }

    let base_branch = target_short_name(&ctx.project_meta()?, &*ctx.repo.get()?)?;
    Ok(affected_stack_indices
        .into_iter()
        .map(|index| {
            review_updates_for_stack(&info.stacks[index], &base_branch, &open_reviews)
                .into_iter()
                .map(|(_, update)| update.into())
                .collect()
        })
        .collect())
}

fn review_updates_for_branch(
    ctx: &Context,
    branch: &gix::refs::FullNameRef,
) -> Result<Vec<but_forge::ForgeReviewUpdate>> {
    let base_branch = target_short_name(&ctx.project_meta()?, &*ctx.repo.get()?)?;
    let info = crate::legacy::workspace::head_info(ctx)?;
    let (stack, _) = stack_and_segment_for_branch(&info, branch)?;
    let open_reviews = open_review_numbers(ctx)?;
    Ok(review_updates_for_stack(stack, &base_branch, &open_reviews)
        .into_iter()
        .map(|(_, update)| update.into())
        .collect())
}

pub(crate) fn review_target_updates_for_branch(
    ctx: &Context,
    branch: &gix::refs::FullNameRef,
) -> Result<
    Vec<(
        gix::refs::FullName,
        but_forge::ForgeReviewTargetUpdate,
        Option<String>,
    )>,
> {
    let info = crate::legacy::workspace::head_info(ctx)?;
    let (stack, _) = stack_and_segment_for_branch(&info, branch)?;
    let open_reviews = open_review_numbers(ctx)?;
    if !stack
        .segments
        .iter()
        .any(|segment| review_number(segment, &open_reviews).is_some())
    {
        return Ok(Vec::new());
    }
    let base_branch = target_short_name(&ctx.project_meta()?, &*ctx.repo.get()?)?;
    let db = ctx.db.get_cache()?;
    let cached_targets = but_forge::list_cached_forge_reviews(&db)?
        .into_iter()
        .map(|review| (review.number, review.target_branch))
        .collect::<std::collections::HashMap<_, _>>();
    Ok(review_updates_for_stack(stack, &base_branch, &open_reviews)
        .into_iter()
        .map(|(branch, update)| {
            let current_target = cached_targets.get(&update.number).cloned();
            (branch, update, current_target)
        })
        .collect())
}

/// One `(branch ref, target update)` pair per reviewed active segment,
/// bottom-to-top. Refs and updates are derived from one pass over the same
/// segments, so they cannot fall out of alignment; segments without an
/// active review (via [`review_number`] — integrated ones included) are
/// inert for target computation and contribute nothing.
fn review_updates_for_stack(
    stack: &but_workspace::branch::Stack,
    base_branch: &str,
    open_reviews: &std::collections::HashSet<i64>,
) -> Vec<(gix::refs::FullName, but_forge::ForgeReviewTargetUpdate)> {
    let reviewed = stack
        .segments
        .iter()
        .rev()
        .filter_map(|segment| {
            let number = review_number(segment, open_reviews)?;
            let ref_name = segment.ref_info.as_ref()?.ref_name.clone();
            let short = ref_name.shorten().to_str().ok()?.to_owned();
            Some((ref_name, short, number))
        })
        .collect::<Vec<_>>();
    let heads = reviewed
        .iter()
        .map(|(_, short, number)| (short.clone(), Some(*number)))
        .collect::<Vec<_>>();
    let updates = but_forge::compute_review_target_updates(&heads, base_branch);
    reviewed
        .into_iter()
        .map(|(ref_name, _, _)| ref_name)
        .zip(updates)
        .collect()
}

fn review_creation_target(ctx: &Context, branch: &gix::refs::FullNameRef) -> Result<String> {
    let info = crate::legacy::workspace::head_info(ctx)?;
    let (stack, selected_index) = stack_and_segment_for_branch(&info, branch)?;
    let repo = ctx.repo.get()?;

    let open_reviews = open_review_numbers(ctx)?;
    let mut reviewed_ancestors = stack.segments[selected_index + 1..]
        .iter()
        .rev()
        .filter(|segment| review_number(segment, &open_reviews).is_some())
        .map(|segment| remote_head(&repo, segment))
        .collect::<Result<Vec<_>>>()?;
    let selected = remote_head(&repo, &stack.segments[selected_index])?;

    for pair in reviewed_ancestors
        .iter()
        .map(|head| head.remote_tip)
        .chain(std::iter::once(selected.remote_tip))
        .collect::<Vec<_>>()
        .windows(2)
    {
        if !remote_contains(&repo, pair[0], pair[1])? {
            anyhow::bail!(
                "Branch `{}` is pushed, but its remote ancestry does not match the reviewed workspace stack; push it and its ancestors before creating a review",
                branch.shorten()
            );
        }
    }

    match reviewed_ancestors.pop() {
        Some(nearest_reviewed_ancestor) => Ok(nearest_reviewed_ancestor.branch_name),
        None => target_short_name(&ctx.project_meta()?, &repo),
    }
}

#[derive(Debug)]
struct RemoteHead {
    branch_name: String,
    remote_tip: gix::ObjectId,
}

fn stack_and_segment_for_branch<'a>(
    info: &'a but_workspace::RefInfo,
    branch: &gix::refs::FullNameRef,
) -> Result<(&'a but_workspace::branch::Stack, usize)> {
    info.stacks
        .iter()
        .find_map(|stack| {
            stack
                .segments
                .iter()
                .position(|segment| {
                    segment
                        .ref_info
                        .as_ref()
                        .is_some_and(|ref_info| ref_info.ref_name == branch)
                })
                .map(|index| (stack, index))
        })
        .with_context(|| {
            format!(
                "Branch `{}` is not part of the current workspace",
                branch.shorten()
            )
        })
}

/// The numbers of reviews the forge cache currently knows as open.
///
/// The mutation flows gate on this: only an open review is a valid target for
/// retargeting, footer syncs, creation targets, or merge selection. A
/// segment's metadata number can also be settled display state — an
/// integrated branch's landed identity, or a merge still awaiting
/// integration detection — which must never reach the forge as a mutation.
pub fn open_review_numbers(ctx: &Context) -> Result<std::collections::HashSet<i64>> {
    let db = ctx.db.get_cache()?;
    Ok(but_forge::cached_review_states(&db)?
        .into_iter()
        .filter_map(|(number, settled)| (!settled).then_some(number))
        .collect())
}

/// The number of the segment's active review: its recorded number, when the
/// cache knows that review as open. There is deliberately no ungated variant.
fn review_number(
    segment: &but_workspace::ref_info::Segment,
    open_reviews: &std::collections::HashSet<i64>,
) -> Option<i64> {
    let number = segment
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.review.pull_request)
        .map(|number| number as i64)?;
    open_reviews.contains(&number).then_some(number)
}

fn remote_head(
    repo: &gix::Repository,
    segment: &but_workspace::ref_info::Segment,
) -> Result<RemoteHead> {
    let branch_name = segment
        .ref_info
        .as_ref()
        .context("Workspace segment has no local branch")?
        .ref_name
        .shorten()
        .to_str()
        .context("Workspace branch name is not valid UTF-8")?
        .to_owned();
    let remote_ref = segment.remote_tracking_ref_name.as_ref().with_context(|| {
        format!("Branch `{branch_name}` must be pushed before creating a review")
    })?;
    let remote_tip = repo
        .find_reference(remote_ref.as_ref())
        .with_context(|| {
            format!(
                "Remote-tracking ref `{}` for branch `{branch_name}` was not found",
                remote_ref.shorten()
            )
        })?
        .peel_to_id()?
        .detach();
    Ok(RemoteHead {
        branch_name,
        remote_tip,
    })
}

fn remote_contains(
    repo: &gix::Repository,
    ancestor: gix::ObjectId,
    descendant: gix::ObjectId,
) -> Result<bool> {
    if ancestor == descendant {
        return Ok(true);
    }
    match repo.merge_base(ancestor, descendant) {
        Ok(base) => Ok(base == ancestor),
        Err(gix::repository::merge_base::Error::FindMergeBase(_))
        | Err(gix::repository::merge_base::Error::NotFound { .. }) => Ok(false),
        Err(err) => Err(err.into()),
    }
}

fn local_branch_for_review(ctx: &Context, wanted: i64) -> Result<gix::refs::FullName> {
    let info = crate::legacy::workspace::head_info(ctx)?;
    let open_reviews = open_review_numbers(ctx)?;
    info.stacks
        .iter()
        .flat_map(|stack| &stack.segments)
        .find_map(|segment| {
            // Via review_number so a settled review's number cannot resolve
            // into review-sync targets.
            if review_number(segment, &open_reviews)? == wanted {
                segment
                    .ref_info
                    .as_ref()
                    .map(|ref_info| ref_info.ref_name.clone())
            } else {
                None
            }
        })
        .with_context(|| {
            format!("Review #{wanted} is not associated with a local workspace branch")
        })
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
    // Get all applied stacks and their branches
    let workspace = crate::legacy::workspace::head_info(ctx)?;

    // Collect branch references that have CI checks cached
    let mut current_refs = std::collections::HashSet::new();

    // Process each branch that has a PR
    for segment in workspace
        .stacks
        .iter()
        .flat_map(|stack| stack.segments.iter())
    {
        let has_pull_request = segment
            .metadata
            .as_ref()
            .is_some_and(|meta| meta.review.pull_request.is_some());
        if !has_pull_request {
            continue;
        }
        let Some(name) = segment
            .ref_info
            .as_ref()
            .map(|ref_info| ref_info.ref_name.shorten().to_string())
        else {
            continue;
        };

        // Fetch CI checks with NoCache to force refresh
        let _ = list_ci_checks(ctx, name.clone(), Some(but_forge::CacheConfig::NoCache));
        // Ignore errors for individual branches to ensure we process all branches

        // Track this reference as having CI checks
        current_refs.insert(name);
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
