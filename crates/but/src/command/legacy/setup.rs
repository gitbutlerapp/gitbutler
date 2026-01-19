use std::path::Path;

use but_core::RepositoryExt;
use colored::Colorize;
use gitbutler_project::Project;

use crate::utils::OutputChannel;

// setup a gitbutler project for the repository at `repo_path`
pub(crate) fn repo(repo_path: &Path, out: &mut OutputChannel) -> anyhow::Result<()> {
    // find or initialize the git repository
    let repo = match gix::open(repo_path) {
        Ok(repo) => repo,
        Err(_) => {
            // If for humans, try to set up a new repo interactively
            if out.for_human().is_some() {
                match setup_new_repo(repo_path, out) {
                    Ok(repo) => repo,
                    Err(e) => {
                        if let Some(out) = out.for_human() {
                            writeln!(
                                out,
                                "{}",
                                format!("Failed to initialize repository: {}", e).red()
                            )?;
                        }
                        anyhow::bail!(
                            "No git repository found - run `but setup` to initialize a new repository."
                        );
                    }
                }
            } else {
                anyhow::bail!("No git repository found.");
            }
        }
    };

    // find or setup the gitbutler project
    if let Some(out) = out.for_human() {
        writeln!(out, "{}", "Setting up GitButler project...".cyan())?;
        writeln!(out)?;
        writeln!(
            out,
            "{}",
            "→ Adding repository to GitButler project registry"
                .to_string()
                .dimmed()
        )?;
    }
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
            "The path {} is not a git repository.",
            repo_path.display()
        )),
    }?;

    if let Some(out) = out.for_human() {
        match &outcome {
            gitbutler_project::AddProjectOutcome::Added(_) => {
                writeln!(
                    out,
                    "  {}",
                    "✓ Repository added to project registry".green()
                )?;
            }
            gitbutler_project::AddProjectOutcome::AlreadyExists(_) => {
                writeln!(
                    out,
                    "  {}",
                    "✓ Repository already in project registry".green()
                )?;
            }
            _ => {}
        }
    }

    // Check if target branch is set
    let target = but_api::legacy::virtual_branches::get_base_branch_data(project.id)?;

    // If new or already exists but target is not set, set the target to be the remote's HEAD
    if (matches!(outcome, gitbutler_project::AddProjectOutcome::Added(_))
        || matches!(
            outcome,
            gitbutler_project::AddProjectOutcome::AlreadyExists(_)
        ))
        && target.is_none()
    {
        // Step 2: Determine remote
        if let Some(out) = out.for_human() {
            writeln!(out)?;
            writeln!(out, "{}", "→ Configuring default target branch".dimmed())?;
        }

        let remote_name = match repo.remote_default_name(gix::remote::Direction::Push) {
            Some(name) => {
                if let Some(out) = out.for_human() {
                    writeln!(
                        out,
                        "  {}",
                        format!("✓ Using existing push remote: {}", name).green()
                    )?;
                }
                name.to_string()
            }
            None => setup_local_remote(&repo, out)?,
        };

        // Try to find the remote HEAD, or fall back to detecting main/master
        let name = if let Ok(mut head_ref) =
            repo.find_reference(&format!("refs/remotes/{remote_name}/HEAD"))
        {
            head_ref.peel_to_commit().ok(); // Need this in order to "open" HEAD
            head_ref.name().shorten().to_string()
        } else {
            // No HEAD reference, try to find main or master
            let fallback_branch = ["main", "master"].into_iter().find(|branch| {
                repo.find_reference(&format!("refs/remotes/{remote_name}/{branch}"))
                    .is_ok()
            });
            match fallback_branch {
                Some(branch) => {
                    if let Some(out) = out.for_human() {
                        writeln!(
                            out,
                            "  {}",
                            format!("✓ No remote HEAD found, using {}/{}", remote_name, branch)
                                .yellow()
                        )?;
                    }
                    format!("{remote_name}/{branch}")
                }
                None => {
                    anyhow::bail!("No HEAD reference found for remote {}", remote_name);
                }
            }
        };

        but_api::legacy::virtual_branches::set_base_branch(
            project.id,
            name.clone(),
            Some(remote_name.clone()),
        )?;
        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "  {}",
                format!("✓ Set default target to: {}", name).green()
            )?;
            writeln!(out)?;
            writeln!(
                out,
                "{}",
                "GitButler project setup complete!".green().bold()
            )?;
            writeln!(out)?;
            writeln!(
                out,
                "{}",
                format!("Repository: {}", repo_path.display()).dimmed()
            )?;
            writeln!(out, "{}", format!("Default target: {}", name).dimmed())?;
            writeln!(out, "{}", format!("Remote: {}", remote_name).dimmed())?;
            writeln!(out)?;
        }
    } else if let Some(out) = out.for_human() {
        writeln!(out)?;
        writeln!(
            out,
            "{}",
            "GitButler project is already set up!".green().bold()
        )?;
        writeln!(out)?;
        writeln!(
            out,
            "{}",
            format!("Repository: {}", repo_path.display()).dimmed()
        )?;
        if let Some(target) = target {
            writeln!(
                out,
                "{}",
                format!("Default target: {}", target.branch_name).dimmed()
            )?;
        }
        writeln!(out)?;
    }

    // what branch is head() pointing to?
    let head = repo.head()?;
    let head_name = head
        .referent_name()
        .map(|n| n.shorten().to_string())
        .unwrap_or_default();
    if head_name != "gitbutler/workspace"
        && let Some(out) = out.for_human()
    {
        writeln!(
            out,
            "Currently on {}. Switching back to gitbutler/workspace branch...",
            head_name,
        )?;

        but_api::legacy::virtual_branches::switch_back_to_workspace(project.id)?;
    }

    Ok(())
}

/// Checks if a GitButler project is set up for the repository at `repo_path`.
/// If so, returns true
/// Otherwise, returns an error with a message indicating what is not setup.
/// It will check:
/// - if the repository exists
/// - if the project is registered in GitButler
/// - if there is a remote
/// - if there is a default target branch set
/// - if we're on gitbutler/workspace
pub fn check_project_setup(project: &Project) -> anyhow::Result<bool> {
    let target = but_api::legacy::virtual_branches::get_base_branch_data(project.id)?;
    if target.is_none() {
        anyhow::bail!("No default target branch set.");
    }

    let repo = gix::open(project.worktree_dir()?)?;

    // check if there is a remote
    let _remote_name = match repo.remote_default_name(gix::remote::Direction::Push) {
        Some(name) => name.to_string(),
        None => anyhow::bail!("No push remote found."),
    };

    // check if we're on gitbutler/workspace
    let head = repo.head()?;
    let head_name = head
        .referent_name()
        .map(|n| n.shorten().to_string())
        .unwrap_or_default();
    if head_name != "gitbutler/workspace" {
        anyhow::bail!("Not currently on gitbutler/workspace branch.");
    }

    Ok(true)
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
            "  {}",
            "No push remote found, creating gb-local remote...".yellow()
        )?;
    }

    let mut config = repo.local_common_config_for_editing()?;
    let mut section = config.section_mut_or_create_new("remote", Some("gb-local".into()))?;
    section.push("url".try_into()?, Some(repo_url.into()));
    repo.write_local_common_config(&config)?;

    // Figure out what local branch is probably the default target
    let mut head_ref = repo.head()?;
    if head_ref.id().is_none() {
        create_empty_initial_commit(repo)?;
        head_ref = repo.head()?;
    }

    let head_commit = head_ref.peel_to_commit()?;

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

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "  {}",
            format!("✓ Created gb-local remote tracking {}", default_branch_name).green()
        )?;
    }

    Ok("gb-local".to_string())
}

/// Sets up a new git repository and creates an initial empty commit.
fn setup_new_repo(current_dir: &Path, out: &mut OutputChannel) -> anyhow::Result<gix::Repository> {
    use std::fmt::Write as FmtWrite;

    let mut progress = out.progress_channel();
    if let Some(mut inout) = out.prepare_for_terminal_input() {
        writeln!(
            &mut progress as &mut dyn FmtWrite,
            "{}",
            "No git repository found.".red()
        )?;

        let input = inout.prompt(format!(
            "Would you like to initialize a new one?\n{}\n[y/N]",
            "(this will also create an empty first commit)".dimmed()
        ))?;
        if input.as_deref() == Some("y") {
            writeln!(
                &mut progress as &mut dyn FmtWrite,
                "{}",
                "Initializing new repository and creating an empty first commit...".dimmed()
            )?;
            let repo = gix::init(current_dir)?;

            create_empty_initial_commit(&repo)?;

            writeln!(
                &mut progress as &mut dyn FmtWrite,
                "{}",
                "Initialized a new repository and created an empty first commit.\n".green()
            )?;
            return Ok(repo);
        }
    }

    Err(anyhow::anyhow!("No git repository found."))
}

fn create_empty_initial_commit(repo: &gix::Repository) -> anyhow::Result<()> {
    // In an unborn repo, this returns the well-known empty-tree id.
    // (It works even if the empty tree object isn’t physically in the ODB.)
    let empty_tree = repo.head_tree_id_or_empty().expect("repo access failed"); // -> Id<'_>
    let empty_tree = empty_tree.detach(); // -> ObjectId (optional; commit() accepts Into<ObjectId> anyway)

    // No parents for the first commit. Update HEAD (writes through to refs/heads/main).
    repo.commit(
        "HEAD",
        "Initial empty commit\n",
        empty_tree,
        std::iter::empty::<gix::hash::ObjectId>(),
    )?;

    Ok(())
}
