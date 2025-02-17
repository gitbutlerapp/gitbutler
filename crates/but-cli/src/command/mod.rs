use anyhow::{bail, Context};
use gitbutler_project::Project;
use std::path::Path;

pub fn project_from_path(path: &Path) -> anyhow::Result<Project> {
    Project::from_path(path)
}

pub fn project_repo(path: &Path) -> anyhow::Result<gix::Repository> {
    let project = project_from_path(path)?;
    Ok(gix::open(project.worktree_path())?)
}

/// Operate like GitButler would in the future, on a Git repository and optionally with additional metadata as obtained
/// from the previously added project.
pub fn repo_and_maybe_project(
    args: &super::Args,
) -> anyhow::Result<(gix::Repository, Option<Project>)> {
    let mut repo = gix::discover(&args.current_dir)?;
    repo.object_cache_size_if_unset(512 * 1024);
    let res = if let Some((projects, work_dir)) =
        project_controller(args.app_suffix.as_deref(), args.app_data_dir.as_deref())
            .ok()
            .zip(repo.work_dir())
    {
        let work_dir = gix::path::realpath(work_dir)?;
        (
            repo,
            projects.list()?.into_iter().find(|p| p.path == work_dir),
        )
    } else {
        (repo, None)
    };
    Ok(res)
}

fn debug_print(this: impl std::fmt::Debug) -> anyhow::Result<()> {
    println!("{:#?}", this);
    Ok(())
}

fn project_controller(
    app_suffix: Option<&str>,
    app_data_dir: Option<&Path>,
) -> anyhow::Result<gitbutler_project::Controller> {
    let path = if let Some(dir) = app_data_dir {
        std::fs::create_dir_all(dir).context("Failed to assure the designated data-dir exists")?;
        dir.to_owned()
    } else {
        dirs_next::data_dir()
            .map(|dir| {
                dir.join(format!(
                    "com.gitbutler.app{}",
                    app_suffix
                        .map(|suffix| {
                            let mut suffix = suffix.to_owned();
                            suffix.insert(0, '.');
                            suffix
                        })
                        .unwrap_or_default()
                ))
            })
            .context("no data-directory available on this platform")?
    };
    if !path.is_dir() {
        bail!("Path '{}' must be a valid directory", path.display());
    }
    tracing::debug!("Using projects from '{}'", path.display());
    Ok(gitbutler_project::Controller::from_path(path))
}

mod commit;
pub use commit::commit;

pub mod diff;

pub mod stacks {
    use std::path::Path;

    use but_settings::AppSettings;
    use but_workspace::stack_branches;
    use gitbutler_command_context::CommandContext;

    use crate::command::{debug_print, project_from_path};

    pub fn list(current_dir: &Path) -> anyhow::Result<()> {
        let project = project_from_path(current_dir)?;
        debug_print(but_workspace::stacks(&project.gb_dir()))
    }

    pub fn branches(id: &str, current_dir: &Path) -> anyhow::Result<()> {
        let project = project_from_path(current_dir)?;
        let ctx = CommandContext::open(&project, AppSettings::default())?;
        debug_print(stack_branches(id.to_string(), &ctx))
    }
}
