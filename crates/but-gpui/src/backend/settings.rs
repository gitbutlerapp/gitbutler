//! Settings the app shares with GitButler's other clients, a project's own settings, and
//! the editors and terminals files can open in.

use std::path::Path;

use anyhow::{Context as _, Result};
use but_api::legacy::{config, projects};

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
pub fn open_in_editor(path: &Path, editor: &str, file: &str, line: Option<u32>) -> Result<()> {
    let mut ctx = open(path)?;
    but_api::open::open_in_program(
        &mut ctx,
        editor.to_owned(),
        but_api::commit::json::ChangesSource::Head,
        file.to_owned(),
        line,
    )
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

pub fn set_sign_commits(path: &Path, sign: bool) -> Result<()> {
    let ctx = open(path)?;
    let mut git = config::get_gb_config(&ctx)?;
    git.gitbutler_sign_commits = Some(sign);
    config::set_gb_config(&ctx, git)
}

/// Where pull requests list the stack, and whether GitHub shows them as a native stack.
pub fn set_stacking(
    path: &Path,
    info: Option<StackInfo>,
    github: Option<GitHubStacks>,
) -> Result<()> {
    let ctx = open(path)?;
    let mut git = config::get_gb_config(&ctx)?;
    if let Some(info) = info {
        git.gitbutler_review_stacking_description = Some(info);
    }
    if let Some(github) = github {
        git.gitbutler_github_stacking_mode = Some(github);
    }
    config::set_gb_config(&ctx, git)
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
