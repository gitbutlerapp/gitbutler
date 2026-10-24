//! Settings the app shares with GitButler's other clients, a project's own settings, and
//! the editors and terminals files can open in.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use but_api::legacy::{config, projects};
use but_core::settings::git::ui::GitConfigSettings;
use but_settings::AppSettingsWithDiskSync;

use super::ops::{changes_source, full_name};
use super::{Forge, open};

/// A program to open files or the project in, by id and name.
pub type Program = (String, String);

pub fn editors() -> Vec<Program> {
    but_api::open::list_editors()
        .unwrap_or_default()
        .into_iter()
        .map(|editor| (editor.id, editor.name))
        .collect()
}

pub fn terminals() -> Vec<Program> {
    but_api::open::terminal::get_terminal_options_for_platform(std::env::consts::OS.into())
        .unwrap_or_default()
        .into_iter()
        .map(|terminal| (terminal.identifier, terminal.display_name))
        .collect()
}

/// Open `file` of the project at `path`, or of its linked `worktree`, in `editor` at `line`.
pub fn open_in_editor(
    path: &Path,
    worktree: Option<&str>,
    editor: &str,
    file: &str,
    line: Option<u32>,
) -> Result<()> {
    let mut ctx = open(path)?;
    let source = changes_source(worktree);
    but_api::open::open_in_program(&mut ctx, editor.to_owned(), source, file.to_owned(), line)
}

/// Open the project at `path` in `terminal`, or with none chosen, in the one the platform has.
pub fn open_in_terminal(path: &Path, terminal: Option<String>) -> Result<()> {
    let terminal = match terminal {
        Some(terminal) => terminal,
        None => {
            but_api::open::terminal::get_recommended_terminal_for_platform(
                std::env::consts::OS.into(),
            )?
            .context("found no terminal to open")?
            .identifier
        }
    };
    but_api::open::open_in_terminal(terminal, path.display().to_string())
}

/// How often to fetch in the background, in minutes; `None` when it's off.
pub fn fetch_interval() -> Option<u64> {
    let settings = app_settings().ok()?;
    let minutes = settings.get().ok()?.fetch.auto_fetch_interval_minutes;
    u64::try_from(minutes).ok().filter(|minutes| *minutes > 0)
}

pub fn set_fetch_interval(minutes: Option<u64>) -> Result<()> {
    let minutes = minutes.map_or(-1, |minutes| minutes as isize);
    app_settings()?.update_fetch(but_settings::api::FetchUpdate {
        auto_fetch_interval_minutes: Some(minutes),
    })
}

/// GitButler's feature flags, as the other apps share them.
fn feature_flags() -> Option<but_settings::app_settings::FeatureFlags> {
    Some(app_settings().ok()?.get().ok()?.feature_flags.clone())
}

/// Whether GitButler's experimental support for linked worktrees is on.
pub fn worktrees_enabled() -> bool {
    feature_flags().is_some_and(|flags| flags.worktree_manipulation)
}

pub fn set_worktrees_enabled(on: bool) -> Result<()> {
    app_settings()?.update_feature_flags(but_settings::api::FeatureFlagsUpdate {
        worktree_manipulation: Some(on),
        ..Default::default()
    })
}

/// Whether the desktop app opens this client rather than its web app when it starts.
pub fn opens_at_start() -> bool {
    feature_flags().is_some_and(|flags| flags.gpui)
}

pub fn set_opens_at_start(on: bool) -> Result<()> {
    app_settings()?.update_feature_flags(but_settings::api::FeatureFlagsUpdate {
        gpui: Some(on),
        ..Default::default()
    })
}

/// What the Project section of the settings shows.
#[derive(Debug, Clone)]
pub struct ProjectSettings {
    pub sign_commits: bool,
    /// The forge the project's remote, or a forge account with its host, names.
    pub detected_forge: Option<Forge>,
    /// The forge picked for the project, which counts only when none is detected.
    pub forge_override: Option<Forge>,
}

pub fn project_settings(path: &Path) -> Result<ProjectSettings> {
    let ctx = open(path)?;
    let git = config::get_gb_config(&ctx)?;
    let (detected_forge, forge_override) = super::reviews::detected_and_saved_forge(&ctx);
    Ok(ProjectSettings {
        sign_commits: git.gitbutler_sign_commits.unwrap_or(false),
        detected_forge,
        forge_override,
    })
}

/// Pick the forge the project uses when its remote doesn't name one, or clear the pick
/// (`None`). Saved as GitButler's other apps save it.
pub fn set_forge_override(id: &str, forge: Option<Forge>) -> Result<()> {
    let id = id.parse().context("Invalid project id")?;
    let mut request = gitbutler_project::UpdateRequest::default_with_id(id);
    match forge {
        Some(forge) => request.forge_override = Some(forge.name().to_lowercase()),
        None => request.unset_forge_override = true,
    }
    projects::update_project(request).map(|_| ())
}

/// Change the project's name.
pub fn rename_project(id: &str, name: String) -> Result<()> {
    projects::update_project_settings(
        id.parse().context("Invalid project id")?,
        projects::ProjectSettingsUpdate {
            title: Some(name),
            description: None,
            force_push_protection: None,
            omit_certificate_check: None,
        },
    )
}

/// Whether Git has no author for the project at `path` to record commits with, as on a
/// machine where `user.name` and `user.email` were never set.
pub fn author_missing(path: &Path) -> Result<bool> {
    let author = config::get_author_info(&open(path)?)?;
    Ok(author.name.is_none() || author.email.is_none())
}

/// Make `name` and `email` the author in the user's global Git config where it has none, as
/// GitButler's desktop app does.
pub fn set_author(path: &Path, name: &str, email: &str) -> Result<()> {
    let (name, email) = (name.trim().to_owned(), email.trim().to_owned());
    config::store_author_globally_if_unset(&open(path)?, name, email)
}

/// Only the fields set are written to the repository's own config; what it reads from the
/// user's global config stays there.
pub fn set_sign_commits(path: &Path, sign: bool) -> Result<()> {
    let git = GitConfigSettings {
        gitbutler_sign_commits: Some(sign),
        ..Default::default()
    };
    config::set_gb_config(&open(path)?, git)
}

/// Sign a test commit with the project's settings, to tell whether signing works, or why not
/// in the signing program's own words, as a failed commit says it.
pub fn test_signing(path: &Path) -> Result<()> {
    but_api::legacy::repo::check_signing_settings(&open(path)?)
        .map_err(|err| anyhow::anyhow!(super::signing_failure(&err)))?;
    Ok(())
}

/// Whether Git's credentials for the target's remote work: fetch from it, or with `push`, push
/// an empty branch there and delete it again, as GitButler's other apps test them.
pub fn test_credentials(path: &Path, push: bool) -> Result<()> {
    let ctx = open(path)?;
    let (remote, branch) = {
        let repo = ctx.repo.get()?;
        let target = (ctx.project_meta()?.remote_target_ref(&repo)?)
            .context("The target branch has no remote to test against")?;
        but_core::extract_remote_name_and_short_name(target.as_ref(), &repo.remote_names())
            .context("Couldn't tell which remote the target branch is on")?
    };
    if push {
        but_api::legacy::git::git_test_push(&ctx, remote, branch.to_string())
    } else {
        but_api::legacy::git::git_test_fetch(&ctx, remote, None)
    }
}

const NO_COMMITS: &str =
    "The repository has no commits yet. Make the first one, then pick the target.";

/// The project's local branches, like `refs/heads/main`, to pick a target from, as the
/// desktop app offers them: the one named as a remote's default branch and tracking it first,
/// then others tracking it, then the rest by name, leaving out GitButler's own.
pub fn target_branches(path: &Path) -> Result<Vec<String>> {
    let ctx = open(path)?;
    let repo = ctx.repo.get()?;
    // Its first branch comes with its first commit, so there's nothing to list yet.
    anyhow::ensure!(!repo.head()?.is_unborn(), NO_COMMITS);
    let mut defaults = Vec::new();
    for reference in repo.references()?.remote_branches()?.filter_map(Result::ok) {
        if let gix::refs::TargetRef::Symbolic(default) = reference.target() {
            defaults.push(default.to_owned());
        }
    }
    let mut names: Vec<(u8, String)> = Vec::new();
    for reference in repo.references()?.local_branches()?.filter_map(Result::ok) {
        let name = reference.name();
        if name.as_bstr().starts_with(b"refs/heads/gitbutler/") {
            continue;
        }
        let upstream = repo.branch_remote_tracking_ref_name(name, gix::remote::Direction::Fetch);
        let rank = match upstream
            .and_then(Result::ok)
            .filter(|u| defaults.contains(u))
        {
            Some(default) if default.as_bstr().ends_with(name.shorten()) => 0,
            Some(_) => 1,
            None => 2,
        };
        names.push((rank, name.as_bstr().to_string()));
    }
    names.sort();
    Ok(names.into_iter().map(|(_, name)| name).collect())
}

/// Make `target` what the workspace integrates into: a local branch's full ref, or a remote
/// one that a single local branch tracks, as the CLI takes it. The branch's upstream is what
/// pulls bring in. Only the project's settings change: a new project stays on the branch it
/// has checked out, and branches rebase onto a changed target at the next update.
pub fn set_target(path: &Path, target: &str) -> Result<()> {
    let mut ctx = open(path)?;
    anyhow::ensure!(!ctx.repo.get()?.head()?.is_unborn(), NO_COMMITS);
    let local = but_core::ref_metadata::resolve_local_target(
        full_name(target)?.as_ref(),
        &*ctx.repo.get()?,
    )?;
    but_api::workspace::set_target_ref_and_init_project(
        &mut ctx,
        local.as_local_full_name_ref(),
        None,
    )
}

pub use but_api::ai::{AiConfiguration, AiConfigurationUpdate};

pub fn ai_configuration() -> Result<AiConfiguration> {
    but_api::ai::get_ai_configuration()
}

pub fn update_ai(update: AiConfigurationUpdate) -> Result<AiConfiguration> {
    but_api::ai::update_ai_configuration(update)
}

/// Where GitButler keeps its settings, for the build's channel.
pub fn config_dir() -> Result<PathBuf> {
    but_path::app_config_dir()
}

/// The build's channel, as About names it.
pub fn channel() -> &'static str {
    match but_path::AppChannel::new() {
        but_path::AppChannel::Release => "release",
        but_path::AppChannel::Nightly => "nightly",
        but_path::AppChannel::Dev => "development",
    }
}

/// The version the desktop app's update `feed` offers in place of this build's, or `None`
/// when this build is the latest, which the feed says with no content.
pub fn newer_version(feed: &str) -> Result<Option<String>> {
    #[derive(serde::Deserialize)]
    struct Release {
        version: String,
    }
    // With a timeout, so a check that hangs doesn't hold up the later ones; no content
    // means it's up to date.
    let body = super::accounts::fetch(feed)?;
    if body.is_empty() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_slice::<Release>(&body)?.version))
}

/// GitButler's app settings as they are on disk now. Read afresh each time: the other apps
/// change them too, and saving a copy read earlier would put their changes back.
pub(super) fn app_settings() -> Result<AppSettingsWithDiskSync> {
    let config_dir = config_dir()?;
    std::fs::create_dir_all(&config_dir)?;
    AppSettingsWithDiskSync::new_with_customization(config_dir, None)
}

#[cfg(test)]
mod tests {
    use crate::backend::ops::tests::{git, repo_with};

    #[test]
    fn the_default_branch_lists_first_and_picking_it_stays_on_the_branch() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            git(
                path,
                &["remote", "add", "origin", "https://example.com/repo.git"],
            );
            git(path, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
            git(
                path,
                &[
                    "symbolic-ref",
                    "refs/remotes/origin/HEAD",
                    "refs/remotes/origin/main",
                ],
            );
            git(path, &["update-ref", "refs/remotes/origin/feature", "HEAD"]);
            git(path, &["branch", "--set-upstream-to=origin/main", "main"]);
            git(path, &["branch", "another"]);
            git(path, &["branch", "gitbutler/edit"]);

            assert_eq!(
                super::target_branches(path).unwrap(),
                ["refs/heads/main", "refs/heads/another"],
                "the branch tracking the remote's default first, and none of GitButler's own"
            );
            super::set_target(path, "refs/remotes/origin/main").unwrap();
            assert_eq!(
                git(path, &["symbolic-ref", "HEAD"]).trim(),
                "refs/heads/main",
                "picking a target leaves the checked-out branch alone, as the desktop app does"
            );
            let workspace = crate::backend::load_workspace(path, Default::default()).unwrap();
            assert!(!workspace.managed);
            assert_eq!(
                workspace.target.map(|target| target.name).as_deref(),
                Some("origin/main"),
                "a remote ref picks the local branch tracking it, which goes by its upstream"
            );
        })
    }
}
