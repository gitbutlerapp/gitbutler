//! Settings the app shares with GitButler's other clients, a project's own settings, and
//! the editors and terminals files can open in.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use but_api::legacy::{config, projects};
use but_core::settings::git::ui::GitConfigSettings;
use but_settings::AppSettingsWithDiskSync;

use super::ops::full_name;
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
    but_api::open::terminal::get_terminal_options_for_platform("macos".into())
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
    use but_api::commit::json::ChangesSource;
    let mut ctx = open(path)?;
    let source = worktree.map_or(ChangesSource::Head, |name| {
        ChangesSource::Worktree(name.to_owned())
    });
    but_api::open::open_in_program(&mut ctx, editor.to_owned(), source, file.to_owned(), line)
}

pub fn open_in_terminal(path: &Path, terminal: &str) -> Result<()> {
    but_api::open::open_in_terminal(terminal.to_owned(), path.display().to_string())
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
    pub force_push_protection: bool,
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
        // The value a push checks.
        force_push_protection: ctx.legacy_project.force_push_protection,
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

/// Change the project's name, or whether it refuses force pushes that would drop others'
/// commits.
pub fn update_project(
    id: &str,
    name: Option<String>,
    force_push_protection: Option<bool>,
) -> Result<()> {
    projects::update_project_settings(
        id.parse().context("Invalid project id")?,
        projects::ProjectSettingsUpdate {
            title: name,
            description: None,
            force_push_protection,
            omit_certificate_check: None,
        },
    )
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

/// Sign a test commit with the project's settings, to tell whether signing works.
pub fn test_signing(path: &Path) -> Result<()> {
    but_api::legacy::repo::check_signing_settings(&open(path)?)?;
    Ok(())
}

/// The project's remote branches, like `refs/remotes/origin/main`, to pick a target from:
/// each remote's default branch first, then the rest by name.
pub fn remote_branches(path: &Path) -> Result<Vec<String>> {
    let ctx = open(path)?;
    let repo = ctx.repo.get()?;
    let (mut defaults, mut names) = (Vec::new(), Vec::new());
    for reference in repo.references()?.remote_branches()?.filter_map(Result::ok) {
        let name = reference.name().as_bstr().to_string();
        match reference.target() {
            gix::refs::TargetRef::Symbolic(default) if name.ends_with("/HEAD") => {
                defaults.push(default.as_bstr().to_string())
            }
            _ => names.push(name),
        }
    }
    names.sort();
    names.sort_by_key(|name| !defaults.contains(name));
    Ok(names)
}

/// Make `target` (a remote branch's full ref) what the workspace integrates into. Only the
/// project's settings change: a new project stays on the branch it has checked out, and
/// branches rebase onto a changed target at the next update.
pub fn set_target(path: &Path, target: &str) -> Result<()> {
    let mut ctx = open(path)?;
    anyhow::ensure!(
        !ctx.repo.get()?.head()?.is_unborn(),
        "The repository has no commits yet. Make the first one, then pick the target."
    );
    let name = full_name(target)?;
    but_api::workspace::set_target_ref_and_init_project(&mut ctx, name.as_ref(), None)
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
    use std::process::Command;

    #[test]
    fn the_default_branch_lists_first_and_picking_it_stays_on_the_branch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .current_dir(path)
                .args(args)
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?}: {out:?}");
            String::from_utf8(out.stdout).unwrap()
        };
        git(&["init", "-q", "-b", "main"]);
        git(&["config", "user.name", "T"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "commit.gpgsign", "false"]);
        git(&["commit", "-q", "--allow-empty", "-m", "first"]);
        git(&["remote", "add", "origin", "https://example.com/repo.git"]);
        git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
        git(&[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/main",
        ]);
        git(&["update-ref", "refs/remotes/origin/feature", "HEAD"]);

        assert_eq!(
            super::remote_branches(path).unwrap(),
            ["refs/remotes/origin/main", "refs/remotes/origin/feature"],
            "the remote's default branch first"
        );
        super::set_target(path, "refs/remotes/origin/main").unwrap();
        assert_eq!(
            git(&["symbolic-ref", "HEAD"]).trim(),
            "refs/heads/main",
            "picking a target leaves the checked-out branch alone, as the desktop app does"
        );
        assert!(
            !crate::backend::load_workspace(path, Default::default())
                .unwrap()
                .managed
        );
    }
}
