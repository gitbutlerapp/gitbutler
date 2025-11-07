use std::io::Write;
use std::path::Path;

use anyhow::Context;

pub(crate) fn repo(repo_path: &Path, _json: bool, init_repo: bool) -> anyhow::Result<()> {
    let stdout = std::io::stdout();
    let repo = if init_repo
        && matches!(
            gix::open(repo_path),
            Err(gix::open::Error::NotARepository { .. })
        ) {
        gix::init(repo_path)?
    } else {
        gix::open(repo_path)
            .context("You can run `but init --repo` to initialize a new Git repository")?
    };
    let outcome = but_api::commands::projects::add_project(repo_path.to_path_buf())?;
    let project = match outcome.clone() {
        gitbutler_project::AddProjectOutcome::Added(project) => Ok(project),
        gitbutler_project::AddProjectOutcome::AlreadyExists(project) => Ok(project),
        gitbutler_project::AddProjectOutcome::PathNotFound => Err(anyhow::anyhow!(
            "The path {} does not exist",
            repo_path.display()
        )),
        gitbutler_project::AddProjectOutcome::NotADirectory => Err(anyhow::anyhow!(
            "The path {} is not a directory",
            repo_path.display()
        )),
        gitbutler_project::AddProjectOutcome::BareRepository => Err(anyhow::anyhow!(
            "The repository at {} is bare. GitButler requires a non-bare repository.",
            repo_path.display()
        )),
        gitbutler_project::AddProjectOutcome::NonMainWorktree => Err(anyhow::anyhow!(
            "The repository at {} is a non-main worktree. GitButler requires the main worktree.",
            repo_path.display()
        )),
        gitbutler_project::AddProjectOutcome::NoWorkdir => Err(anyhow::anyhow!(
            "The repository at {} has no working directory. GitButler requires a working directory.",
            repo_path.display()
        )),
        gitbutler_project::AddProjectOutcome::NoDotGitDirectory => Err(anyhow::anyhow!(
            "The repository at {} has no .git directory. GitButler requires a .git directory.",
            repo_path.display()
        )),
        gitbutler_project::AddProjectOutcome::NotAGitRepository(_) => Err(anyhow::anyhow!(
            "The path {} is not a git repository. You can run `but init --repo` to initialize a git repository.",
            repo_path.display()
        )),
    }?;
    let target = but_api::virtual_branches::get_base_branch_data(project.id)?;
    // If new or already exists but target is not set, set the target to be the remote's HEAD
    if (matches!(outcome, gitbutler_project::AddProjectOutcome::Added(_))
        || matches!(
            outcome,
            gitbutler_project::AddProjectOutcome::AlreadyExists(_)
        ))
        && target.is_none()
    {
        let remote_name = repo
            .remote_default_name(gix::remote::Direction::Push)
            .ok_or_else(|| anyhow::anyhow!("No push remote set"))?
            .to_string();
        let mut head_ref = repo
            .find_reference(&format!("refs/remotes/{remote_name}/HEAD"))
            .map_err(|_| anyhow::anyhow!("No HEAD reference found for remote {}", remote_name))?;
        head_ref.peel_to_commit().ok(); // Need this in order to "open" HEAD

        let name = head_ref.name().shorten().to_string();

        but_api::virtual_branches::set_base_branch(project.id, name.clone(), Some(remote_name))?;
        writeln!(
            stdout.lock(),
            "Initialized GitButler project from {}. The default target is {}",
            repo_path.display(),
            name
        )
        .ok();
    } else {
        writeln!(
            stdout.lock(),
            "The repository {} is already initialized as GitButler project. The default target is {:?}",
            repo_path.display(),
            target.map(|t| t.branch_name)
        )
        .ok();
        return Ok(());
    }
    Ok(())
}
