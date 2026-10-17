//! Settings the app shares with GitButler's other clients, a project's own settings, and
//! the editors and terminals files can open in.

use std::path::Path;

use anyhow::{Context as _, Result};
use but_api::legacy::{config, projects};
use but_core::settings::git::ui::GitConfigSettings;

use super::open;

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

/// Open `file` (relative to the project) in the editor `editor`, at `line` if given.
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
    let settings = super::watch::settings().ok()?;
    let minutes = settings.get().ok()?.fetch.auto_fetch_interval_minutes;
    u64::try_from(minutes).ok().filter(|minutes| *minutes > 0)
}

pub fn set_fetch_interval(minutes: Option<u64>) -> Result<()> {
    let minutes = minutes.map_or(-1, |minutes| minutes as isize);
    super::watch::settings()?.update_fetch(but_settings::api::FetchUpdate {
        auto_fetch_interval_minutes: Some(minutes),
    })
}

/// Whether GitButler's experimental support for linked worktrees is on.
pub fn worktrees_enabled() -> bool {
    (super::watch::settings().ok()).is_some_and(|settings| {
        settings
            .get()
            .is_ok_and(|settings| settings.feature_flags.worktree_manipulation)
    })
}

pub fn set_worktrees_enabled(on: bool) -> Result<()> {
    super::watch::settings()?.update_feature_flags(but_settings::api::FeatureFlagsUpdate {
        worktree_manipulation: Some(on),
    })
}

/// What the Project section of the settings shows.
#[derive(Debug, Clone)]
pub struct ProjectSettings {
    pub force_push_protection: bool,
    pub sign_commits: bool,
    /// Where pull requests list the stack's other pull requests.
    pub stack_info: StackInfo,
    pub github_stacks: GitHubStacks,
}

pub use but_core::settings::git::ui::{
    GitHubStackingMode as GitHubStacks, ReviewStackingDescription as StackInfo,
};

pub fn project_settings(id: &str, path: &Path) -> Result<ProjectSettings> {
    let project = gitbutler_project::get(id.parse().context("Invalid project id")?)?;
    let git = config::get_gb_config(&open(path)?)?;
    Ok(ProjectSettings {
        force_push_protection: project.force_push_protection,
        sign_commits: git.gitbutler_sign_commits.unwrap_or(false),
        stack_info: git
            .gitbutler_review_stacking_description
            .unwrap_or(StackInfo::Bottom),
        github_stacks: git
            .gitbutler_github_stacking_mode
            .unwrap_or(GitHubStacks::Auto),
    })
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

/// Only the fields set in `git` are written to the repository's own config; what it reads
/// from the user's global config stays there.
fn set_git_config(path: &Path, git: GitConfigSettings) -> Result<()> {
    config::set_gb_config(&open(path)?, git)
}

pub fn set_sign_commits(path: &Path, sign: bool) -> Result<()> {
    set_git_config(
        path,
        GitConfigSettings {
            gitbutler_sign_commits: Some(sign),
            ..Default::default()
        },
    )
}

/// Where pull requests list the stack, and whether GitHub shows them as a native stack.
pub fn set_stacking(
    path: &Path,
    info: Option<StackInfo>,
    github: Option<GitHubStacks>,
) -> Result<()> {
    set_git_config(
        path,
        GitConfigSettings {
            gitbutler_review_stacking_description: info,
            gitbutler_github_stacking_mode: github,
            ..Default::default()
        },
    )
}

/// Sign a test commit with the project's settings, to tell whether signing works.
pub fn test_signing(path: &Path) -> Result<()> {
    but_api::legacy::repo::check_signing_settings(&open(path)?)?;
    Ok(())
}

/// Take the project out of GitButler. The repository stays as it is on disk.
pub fn remove_project(id: &str) -> Result<()> {
    projects::delete_project(id.parse().context("Invalid project id")?)
}

/// The project's remote branches, like `refs/remotes/origin/main`, to pick a target from.
pub fn remote_branches(path: &Path) -> Result<Vec<String>> {
    let ctx = open(path)?;
    let repo = ctx.repo.get()?;
    let mut names: Vec<String> = repo
        .references()?
        .remote_branches()?
        .filter_map(Result::ok)
        .map(|reference| reference.name().as_bstr().to_string())
        .filter(|name| !name.ends_with("/HEAD"))
        .collect();
    names.sort();
    Ok(names)
}

/// Make `target` (a remote branch's full ref) what the workspace integrates into. Only the
/// project's settings change; branches rebase onto it at the next update.
pub fn set_target(path: &Path, target: &str) -> Result<()> {
    let mut ctx = open(path)?;
    let name = gix::refs::FullName::try_from(target)?;
    but_api::workspace::set_target_ref_and_init_project(&mut ctx, name.as_ref(), None)
}

pub use but_api::ai::{AiConfiguration, AiConfigurationUpdate};

pub fn ai_configuration() -> Result<AiConfiguration> {
    but_api::ai::get_ai_configuration()
}

pub fn update_ai(update: AiConfigurationUpdate) -> Result<AiConfiguration> {
    but_api::ai::update_ai_configuration(update)
}

/// Clone `url` into the folder `into`, which shouldn't exist yet.
pub fn clone(url: &str, into: &Path) -> Result<()> {
    super::runtime()
        .handle()
        .block_on(but_api::legacy::repo::git_clone_repository(
            url.to_owned(),
            into.to_owned(),
        ))
}

/// Tell GitButler the project `id` now lives at `path`.
pub fn relocate_project(id: &str, path: &Path) -> Result<()> {
    gitbutler_project::update(gitbutler_project::UpdateRequest {
        path: Some(path.to_owned()),
        git_dir: Some(path.join(".git")),
        ..gitbutler_project::UpdateRequest::default_with_id(
            id.parse().context("Invalid project id")?,
        )
    })?;
    Ok(())
}
