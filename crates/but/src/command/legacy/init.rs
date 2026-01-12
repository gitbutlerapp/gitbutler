use std::path::Path;

use but_core::RepositoryExt;
use colored::Colorize;

use crate::utils::OutputChannel;

pub(crate) fn repo(repo_path: &Path, out: &mut OutputChannel) -> anyhow::Result<()> {
    let repo = gix::open(repo_path)
        .map_err(|e| anyhow::anyhow!("Could not open the git repository: {}", e))?;

    let outcome = but_api::legacy::projects::add_project(repo_path.to_path_buf())?;
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

    let target = but_api::legacy::virtual_branches::get_base_branch_data(project.id)?;

    // If new or already exists but target is not set, set the target to be the remote's HEAD
    if (matches!(outcome, gitbutler_project::AddProjectOutcome::Added(_))
        || matches!(
            outcome,
            gitbutler_project::AddProjectOutcome::AlreadyExists(_)
        ))
        && target.is_none()
    {
        let remote_name = match repo.remote_default_name(gix::remote::Direction::Push) {
            Some(name) => name.to_string(),
            None => setup_local_remote(&repo, out)?,
        };
        let mut head_ref = repo
            .find_reference(&format!("refs/remotes/{remote_name}/HEAD"))
            .map_err(|_| anyhow::anyhow!("No HEAD reference found for remote {}", remote_name))?;
        head_ref.peel_to_commit().ok(); // Need this in order to "open" HEAD
        let name = head_ref.name().shorten().to_string();
        but_api::legacy::virtual_branches::set_base_branch(
            project.id,
            name.clone(),
            Some(remote_name),
        )?;
        if let Some(out) = out.for_human() {
            writeln!(out, "------------------------------------------------")?;
            writeln!(
                out,
                "Initialized GitButler project from {}\nThe default target is {}",
                repo_path.display(),
                name.to_string().green()
            )?;
            writeln!(out, "------------------------------------------------")?;
            writeln!(out)?;
        }
    } else if let Some(out) = out.for_human() {
        writeln!(out, "------------------------------------------------")?;
        writeln!(
            out,
            "The repository {} is already initialized as GitButler project.\nThe default target is {:?}",
            repo_path.display(),
            target.map(|t| t.branch_name.to_string())
        )?;
        writeln!(out, "------------------------------------------------")?;
        writeln!(out)?;
    }
    Ok(())
}

/// Creates a 'gb-local' remote pointing to this repository and creates tracking refs for the default branch.
fn setup_local_remote(repo: &gix::Repository, out: &mut OutputChannel) -> anyhow::Result<String> {
    let repo_url = repo
        .workdir()
        .ok_or_else(|| anyhow::anyhow!("Repository has no working directory"))?
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Repository path is not valid UTF-8"))?;

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "{}",
            format!(
                "Setting up local remote for repository {}",
                repo.workdir().unwrap().display()
            )
            .dimmed()
        )?;
    }

    let mut config = repo.local_common_config_for_editing()?;
    let mut section = config.section_mut_or_create_new("remote", Some("gb-local".into()))?;
    section.push("url".try_into()?, Some(repo_url.into()));
    repo.write_local_common_config(&config)?;

    // Figure out what local branch is probably the default target
    let head_commit = repo.head()?.peel_to_commit()?;
    let default_branch_name = repo
        .head()?
        .referent_name()
        .map(|n| n.shorten().to_string())
        .unwrap_or_else(|| "main".to_string());

    // Create refs/remotes/gb-local/{branch_name} pointing to the HEAD commit
    let branch_ref_name: gix::refs::FullName =
        format!("refs/remotes/gb-local/{default_branch_name}").try_into()?;
    repo.reference(
        branch_ref_name.clone(),
        head_commit.id(),
        gix::refs::transaction::PreviousValue::Any,
        "GitButler local remote setup",
    )?;

    // Create refs/remotes/gb-local/HEAD as a symbolic reference
    let head_ref_name: gix::refs::FullName = "refs/remotes/gb-local/HEAD".try_into()?;
    repo.edit_reference(gix::refs::transaction::RefEdit {
        change: gix::refs::transaction::Change::Update {
            log: gix::refs::transaction::LogChange {
                mode: gix::refs::transaction::RefLog::AndReference,
                force_create_reflog: false,
                message: "GitButler local remote HEAD".into(),
            },
            expected: gix::refs::transaction::PreviousValue::Any,
            new: gix::refs::Target::Symbolic(branch_ref_name),
        },
        name: head_ref_name,
        deref: false,
    })?;

    Ok("gb-local".to_string())
}
