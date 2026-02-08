use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

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

/// Format a byte count as a human-readable string.
fn format_bytes(bytes: u64) -> String {
    let bytes = bytes as f64;
    if bytes >= 1_073_741_824.0 {
        format!("{:.2} GiB", bytes / 1_073_741_824.0)
    } else if bytes >= 1_048_576.0 {
        format!("{:.2} MiB", bytes / 1_048_576.0)
    } else if bytes >= 1_024.0 {
        format!("{:.1} KiB", bytes / 1_024.0)
    } else {
        format!("{bytes:.0} B")
    }
}

/// Capitalize the first character of a string.
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

/// Spawn a thread that renders a single updating progress bar to stderr.
/// The bar is cleared when the thread exits.
fn spawn_progress_renderer(progress: &Arc<prodash::tree::Root>, stop: Arc<AtomicBool>) -> std::thread::JoinHandle<()> {
    let progress = Arc::downgrade(progress);
    std::thread::spawn(move || {
        let mut snapshot = Vec::new();

        while !stop.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(150));
            let Some(progress) = progress.upgrade() else {
                break;
            };
            progress.sorted_snapshot(&mut snapshot);

            // Find the best task to display and any byte counter
            let mut best_name = String::new();
            let mut best_current: usize = 0;
            let mut best_total: Option<usize> = None;
            let mut bytes_received: Option<usize> = None;

            for (_key, task) in &snapshot {
                if let Some(ref prog) = task.progress {
                    let current = prog.step.load(Ordering::Relaxed);
                    if current == 0 && prog.done_at.is_none() {
                        continue;
                    }
                    let is_bytes = prog
                        .unit
                        .as_ref()
                        .map(|u: &prodash::unit::Unit| format!("{}", u.display(1, None, None)).contains('B'))
                        .unwrap_or(false);

                    if is_bytes {
                        bytes_received = Some(current);
                    } else if prog.done_at.is_some() {
                        best_name = task.name.clone();
                        best_current = current;
                        best_total = prog.done_at;
                    } else if best_total.is_none() {
                        best_name = task.name.clone();
                        best_current = current;
                    }
                }
            }

            if best_name.is_empty() && bytes_received.is_none() {
                continue;
            }

            let width = terminal_size::terminal_size().map(|(w, _)| w.0 as usize).unwrap_or(80);

            // Build suffix: " 45678/232966, 65.20 MiB"
            let mut suffix = String::new();
            if let Some(total) = best_total {
                suffix.push_str(&format!(" {best_current}/{total}"));
            } else if best_current > 0 {
                suffix.push_str(&format!(" {best_current}"));
            }
            if let Some(b) = bytes_received {
                if !suffix.is_empty() {
                    suffix.push_str(", ");
                } else {
                    suffix.push(' ');
                }
                suffix.push_str(&format_bytes(b as u64));
            }

            let label = if best_name.is_empty() {
                "Fetching".to_string()
            } else {
                capitalize(&best_name)
            };
            let colored_label = format!("{}", label.bold().green());
            let label_width = label.len();

            let chrome = label_width + 2 + 1 + suffix.len();
            let bar_width = if width > chrome + 5 { width - chrome - 1 } else { 20 };

            let bar = if let Some(total) = best_total {
                let fraction = if total > 0 {
                    (best_current as f64 / total as f64).min(1.0)
                } else {
                    0.0
                };
                let filled = (fraction * bar_width as f64) as usize;
                let arrow = if filled < bar_width { ">" } else { "=" };
                let empty = bar_width.saturating_sub(filled).saturating_sub(1);
                format!(
                    "{}{}{}",
                    "=".repeat(filled),
                    if filled < bar_width { arrow } else { "" },
                    " ".repeat(empty)
                )
            } else {
                " ".repeat(bar_width)
            };

            let _ = write!(std::io::stderr(), "\x1b[2K\r{colored_label} [{}]{suffix}", bar.cyan());
            let _ = std::io::stderr().flush();
        }

        // Clear the progress line
        let _ = write!(std::io::stderr(), "\x1b[2K\r");
        let _ = std::io::stderr().flush();
    })
}

/// Print clone statistics from the fetch outcome.
fn print_stats(
    outcome: &gix::remote::fetch::Outcome,
    elapsed: Duration,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let Some(out) = out.for_human() else {
        return Ok(());
    };

    let refs_updated = match &outcome.status {
        gix::remote::fetch::Status::Change { update_refs, .. } => update_refs.edits.len(),
        gix::remote::fetch::Status::NoPackReceived { update_refs, .. } => update_refs.edits.len(),
    };

    match &outcome.status {
        gix::remote::fetch::Status::Change { write_pack_bundle, .. } => {
            let objects = write_pack_bundle.index.num_objects;
            let pack_size = write_pack_bundle
                .data_path
                .as_ref()
                .and_then(|p| std::fs::metadata(p).ok())
                .map(|m| m.len());
            let idx_size = write_pack_bundle
                .index_path
                .as_ref()
                .and_then(|p| std::fs::metadata(p).ok())
                .map(|m| m.len());

            let secs = elapsed.as_secs_f64();
            writeln!(out)?;
            writeln!(out, "{}", "Clone complete.".green())?;
            writeln!(out)?;
            writeln!(out, "  {}  {}", "Objects:".bold(), objects)?;
            if let Some(size) = pack_size {
                writeln!(out, "  {}     {}", "Pack:".bold(), format_bytes(size))?;
            }
            if let Some(size) = idx_size {
                writeln!(out, "  {}    {}", "Index:".bold(), format_bytes(size))?;
            }
            writeln!(out, "  {}     {refs_updated}", "Refs:".bold())?;
            writeln!(out, "  {}     {secs:.1}s", "Time:".bold())?;
            writeln!(out)?;
        }
        gix::remote::fetch::Status::NoPackReceived { .. } => {
            writeln!(out)?;
            writeln!(out, "{}", "Clone complete.".green())?;
            writeln!(out, "  {}     {refs_updated}", "Refs:".bold())?;
            writeln!(out)?;
        }
    }

    Ok(())
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
        anyhow::bail!("Destination path '{}' already exists.", target_dir.display());
    }

    if let Some(out) = out.for_human() {
        writeln!(out, "{}", format!("Cloning into '{}'...", target_dir.display()).cyan())?;
    }

    let start = std::time::Instant::now();
    let should_interrupt = AtomicBool::new(false);

    let is_human = out.for_human().is_some();
    let progress = Arc::new(prodash::tree::Root::new());
    let stop_renderer = Arc::new(AtomicBool::new(false));
    let render_thread = if is_human {
        Some(spawn_progress_renderer(&progress, Arc::clone(&stop_renderer)))
    } else {
        None
    };

    let mut clone_progress = progress.add_child("clone");

    let (mut checkout, fetch_outcome) = gix::prepare_clone(url.as_str(), &target_dir)?
        .fetch_then_checkout(&mut clone_progress, &should_interrupt)
        .context("Failed to fetch repository")?;

    let (_repo, _) = checkout
        .main_worktree(&mut clone_progress, &should_interrupt)
        .context("Failed to checkout worktree")?;

    let elapsed = start.elapsed();

    // Stop the progress renderer (clear the line)
    stop_renderer.store(true, Ordering::Relaxed);
    if let Some(handle) = render_thread {
        let _ = handle.join();
    }
    drop(clone_progress);
    drop(progress);

    // Print stats summary
    print_stats(&fetch_outcome, elapsed, out)?;

    // Use the canonicalized target_dir for setup, matching how `but setup` uses
    // args.current_dir. This ensures path consistency for project registration.
    let repo_path = std::fs::canonicalize(&target_dir).unwrap_or_else(|_| target_dir.clone());

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
/// This mirrors the `but setup` flow: register the project first,
/// then open the repo from the registered project's git dir.
fn run_setup(repo_path: &Path, out: &mut OutputChannel) -> anyhow::Result<()> {
    let repo = match but_api::legacy::projects::add_project_best_effort(repo_path.to_path_buf())? {
        gitbutler_project::AddProjectOutcome::Added(project)
        | gitbutler_project::AddProjectOutcome::AlreadyExists(project) => gix::open(project.git_dir())?,
        _ => {
            anyhow::bail!(
                "Could not register '{}' as a GitButler project. Run 'but setup' manually.",
                repo_path.display()
            );
        }
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
        assert_eq!(directory_from_url("https://github.com/user/repo.git").unwrap(), "repo");
        assert_eq!(directory_from_url("https://github.com/user/repo").unwrap(), "repo");
        assert_eq!(directory_from_url("git@github.com:user/repo.git").unwrap(), "repo");
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
