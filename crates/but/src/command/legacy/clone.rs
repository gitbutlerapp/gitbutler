use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use anyhow::Context as _;
use colored::Colorize;

use crate::utils::OutputChannel;

/// Derive a directory name from a clone URL.
///
/// Examples:
/// - `https://github.com/user/repo.git` -> `repo`
/// - `git@github.com:user/repo.git` -> `repo`
/// - `https://github.com/user/repo` -> `repo`
fn directory_from_url(url: &str) -> anyhow::Result<String> {
    let name = url
        .rsplit('/')
        .next()
        .or_else(|| url.rsplit(':').next())
        .context("Could not derive directory name from URL")?;
    let name = name.strip_suffix(".git").unwrap_or(name);
    if name.is_empty() {
        anyhow::bail!("Could not derive directory name from URL '{url}'");
    }
    Ok(name.to_string())
}

/// Resolve a host shorthand to a domain name.
fn resolve_host(host: &str) -> &str {
    match host {
        "github" => "github.com",
        "gitlab" => "gitlab.com",
        _ => host,
    }
}

/// If `input` looks like a shorthand (`owner/repo`), expand it to a full URL
/// using the configured protocol and host defaults.
fn expand_shorthand(input: &str, protocol: &str, host: &str) -> String {
    if !input.contains(':') && !input.contains('@') && input.matches('/').count() == 1 {
        let domain = resolve_host(host);
        match protocol {
            "ssh" => format!("git@{domain}:{input}.git"),
            _ => format!("https://{domain}/{input}"),
        }
    } else {
        input.to_string()
    }
}

/// Clone a repository from `url` into `path` and then run GitButler setup.
pub fn run(url: String, path: Option<PathBuf>, out: &mut OutputChannel) -> anyhow::Result<()> {
    let (protocol, host) = load_clone_defaults();
    let url = expand_shorthand(&url, &protocol, &host);
    let target_dir = match path {
        Some(p) => p,
        None => PathBuf::from(directory_from_url(&url)?),
    };

    if target_dir.exists() {
        anyhow::bail!(
            "Destination path '{}' already exists.",
            target_dir.display()
        );
    }

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "{}",
            format!("Cloning into '{}'...", target_dir.display()).cyan()
        )?;
    }

    let should_interrupt = AtomicBool::new(false);

    let (mut checkout, _outcome) = gix::prepare_clone(url.as_str(), &target_dir)?
        .fetch_then_checkout(gix::progress::Discard, &should_interrupt)
        .context("Failed to fetch repository")?;

    let (repo, _) = checkout
        .main_worktree(gix::progress::Discard, &should_interrupt)
        .context("Failed to checkout worktree")?;

    if let Some(out) = out.for_human() {
        writeln!(out, "{}", "Clone complete.".green())?;
        writeln!(out)?;
    }

    let repo_path = repo
        .workdir()
        .unwrap_or_else(|| repo.git_dir())
        .to_path_buf();

    run_setup(&repo_path, out)
}

/// Load clone defaults from forge settings, falling back to "https" and "github".
fn load_clone_defaults() -> (String, String) {
    let defaults = but_path::app_data_dir()
        .ok()
        .map(but_forge_storage::Controller::from_path);
    let protocol = defaults
        .as_ref()
        .and_then(|s| s.clone_protocol().ok().flatten())
        .unwrap_or_else(|| "https".to_string());
    let host = defaults
        .as_ref()
        .and_then(|s| s.clone_host().ok().flatten())
        .unwrap_or_else(|| "github".to_string());
    (protocol, host)
}

/// Run the GitButler setup on an already-cloned repository.
fn run_setup(repo_path: &Path, out: &mut OutputChannel) -> anyhow::Result<()> {
    let repo = match but_api::legacy::projects::add_project_best_effort(repo_path.to_path_buf())? {
        gitbutler_project::AddProjectOutcome::Added(project)
        | gitbutler_project::AddProjectOutcome::AlreadyExists(project) => gix::open(project.git_dir())?,
        _ => gix::open(repo_path)?,
    };
    let mut ctx = but_ctx::Context::from_repo(repo)?;
    let mut guard = ctx.exclusive_worktree_access();
    super::setup::repo_quiet(&mut ctx, repo_path, out, guard.write_permission())
        .context("Failed to set up GitButler project")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_directory_from_url() {
        assert_eq!(
            directory_from_url("https://github.com/user/repo.git").unwrap(),
            "repo"
        );
        assert_eq!(
            directory_from_url("https://github.com/user/repo").unwrap(),
            "repo"
        );
        assert_eq!(
            directory_from_url("git@github.com:user/repo.git").unwrap(),
            "repo"
        );
        assert!(directory_from_url("").is_err());
    }

    #[test]
    fn test_expand_shorthand_defaults() {
        // Default: https + github
        assert_eq!(
            expand_shorthand("schacon/piper", "https", "github"),
            "https://github.com/schacon/piper"
        );
    }

    #[test]
    fn test_expand_shorthand_ssh() {
        assert_eq!(
            expand_shorthand("schacon/piper", "ssh", "github"),
            "git@github.com:schacon/piper.git"
        );
    }

    #[test]
    fn test_expand_shorthand_gitlab() {
        assert_eq!(
            expand_shorthand("user/repo", "https", "gitlab"),
            "https://gitlab.com/user/repo"
        );
        assert_eq!(
            expand_shorthand("user/repo", "ssh", "gitlab"),
            "git@gitlab.com:user/repo.git"
        );
    }

    #[test]
    fn test_expand_shorthand_custom_host() {
        assert_eq!(
            expand_shorthand("user/repo", "https", "gitea.example.com"),
            "https://gitea.example.com/user/repo"
        );
        assert_eq!(
            expand_shorthand("user/repo", "ssh", "gitea.example.com"),
            "git@gitea.example.com:user/repo.git"
        );
    }

    #[test]
    fn test_expand_shorthand_full_urls_unchanged() {
        assert_eq!(
            expand_shorthand("https://github.com/user/repo", "ssh", "gitlab"),
            "https://github.com/user/repo"
        );
        assert_eq!(
            expand_shorthand("git@github.com:user/repo.git", "https", "gitlab"),
            "git@github.com:user/repo.git"
        );
    }
}
