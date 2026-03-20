use std::path::{self, Path};

use anyhow::Context as _;
use but_core::{
    RepositoryExt,
    sync::{RepoExclusive, RepoShared},
};
use but_ctx::Context;
use colored::Colorize;
use serde::Serialize;

use crate::utils::OutputChannel;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupResult {
    /// The repository path
    repository_path: String,
    /// Whether the project was newly added or already existed
    project_status: ProjectStatus,
    /// The target branch configuration
    target: Option<TargetInfo>,
    /// Information about an external hook manager, if one was detected
    hook_manager: Option<HookManagerInfo>,
}

/// Details about an external hook manager detected during setup.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HookManagerInfo {
    /// The name of the detected hook manager (e.g. "prek", "lefthook")
    name: String,
    /// Whether GitButler installed its own managed hooks
    hooks_installed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
enum ProjectStatus {
    /// Project was newly added to registry
    Added,
    /// Project already existed in registry
    AlreadyExists,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TargetInfo {
    /// The target branch name (e.g., "origin/main")
    branch_name: String,
    /// The remote name (e.g., "origin" or "gb-local")
    remote_name: String,
    /// Whether the target was newly set or already existed
    newly_set: bool,
}

/// Display a colorful splash screen with GitButler branding and helpful commands
fn display_splash_screen(out: &mut dyn std::fmt::Write) -> anyhow::Result<()> {
    writeln!(out)?;
    writeln!(
        out,
        "{}",
        r#"
 █████      █████    ██████╗ ██╗   ██╗████████╗
   █████  █████      ██╔══██╗██║   ██║╚══██╔══╝
     ████████        ██████╔╝██║   ██║   ██║
   █████  █████      ██╔══██╗██║   ██║   ██║
 █████      █████    ██████╔╝╚██████╔╝   ██║
"#
        .cyan()
        .bold()
    )?;

    writeln!(
        out,
        "{}",
        "The command-line interface for GitButler".dimmed()
    )?;
    writeln!(out)?;

    writeln!(
        out,
        "{:<45} {}",
        "$ but branch new <name>".bright_blue(),
        "Create a new branch".dimmed()
    )?;
    writeln!(
        out,
        "{:<45} {}",
        "$ but status".bright_blue(),
        "View workspace status".dimmed()
    )?;
    writeln!(
        out,
        "{:<45} {}",
        "$ but commit -m <message>".bright_blue(),
        "Commit changes to current branch".dimmed()
    )?;
    writeln!(
        out,
        "{:<45} {}",
        "$ but push".bright_blue(),
        "Push all branches".dimmed()
    )?;
    writeln!(
        out,
        "{:<45} {}",
        "$ but teardown".bright_blue(),
        "Return to normal Git mode".dimmed()
    )?;
    writeln!(out)?;

    writeln!(
        out,
        "{}",
        "Learn more at https://docs.gitbutler.com/cli-overview".dimmed()
    )?;
    writeln!(out)?;

    Ok(())
}

/// Finds an existing git repository at `repo_path`, or initializes a new one if `init` is true.
pub fn find_or_initialize_repo(
    repo_path: &Path,
    out: &mut OutputChannel,
    init: bool,
) -> anyhow::Result<gix::Repository> {
    match gix::open(repo_path) {
        Ok(repo) => Ok(repo),
        Err(_) => {
            // If --init flag is passed, initialize a new repo non-interactively
            if init {
                if let Some(out) = out.for_human() {
                    writeln!(
                        out,
                        "{}",
                        "No git repository found. Initializing new repository...".dimmed()
                    )?;
                }
                let repo = gix::init(repo_path)?;
                create_empty_initial_commit(&repo)?;
                if let Some(out) = out.for_human() {
                    writeln!(
                        out,
                        "{}",
                        "✓ Initialized repository with empty commit".green()
                    )?;
                    writeln!(out)?;
                }
                Ok(repo)
            }
            // If for humans, try to set up a new repo interactively
            else if out.for_human().is_some() {
                match setup_new_repo(repo_path, out) {
                    Ok(repo) => Ok(repo),
                    Err(e) => {
                        if let Some(out) = out.for_human() {
                            writeln!(
                                out,
                                "{}",
                                format!("Failed to initialize repository: {e}").red()
                            )?;
                        }
                        anyhow::bail!(
                            "No git repository found - run `but setup --init` to initialize a new repository."
                        );
                    }
                }
            } else {
                anyhow::bail!("No git repository found.");
            }
        }
    }
}

// setup a gitbutler project for the repository at `repo_path`
pub(crate) fn repo(
    ctx: &mut Context,
    repo_path: &Path,
    out: &mut OutputChannel,
    perm: &mut RepoExclusive,
    no_hooks: bool,
    force_hooks: bool,
) -> anyhow::Result<()> {
    let mut target_info: Option<TargetInfo> = None;

    // what branch is head() pointing to?
    let pre_head_name = {
        let repo = ctx.repo.get()?;
        let pre_head = repo.head()?;
        pre_head
            .referent_name()
            .map(|n| n.shorten().to_string())
            .unwrap_or_default()
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

    let outcome = but_api::legacy::projects::add_project_best_effort(repo_path.to_path_buf())?;

    // Track project status for JSON output
    let project_status = match outcome {
        gitbutler_project::AddProjectOutcome::Added(_) => {
            if let Some(out) = out.for_human() {
                writeln!(
                    out,
                    "  {}",
                    "✓ Repository added to project registry".green()
                )?;
            }
            Ok(ProjectStatus::Added)
        }
        gitbutler_project::AddProjectOutcome::AlreadyExists(_) => {
            if let Some(out) = out.for_human() {
                writeln!(
                    out,
                    "  {}",
                    "✓ Repository already in project registry".green()
                )?;
            }
            Ok(ProjectStatus::AlreadyExists)
        }
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

    {
        let repo = ctx.repo.get()?;
        if no_hooks {
            gitbutler_repo::managed_hooks::set_install_managed_hooks_enabled(&repo, false)?;
        }
        // NOTE: --force-hooks config persistence is deferred to the hook section
        // below, because switch_back_to_workspace_with_perm() may call
        // ensure_managed_hooks(force=false) which would overwrite it.
    }

    // Check if target branch is set
    let target = but_api::legacy::virtual_branches::get_base_branch_data(ctx)?;

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

        let repo = ctx.repo.get()?;
        let remote_name = match repo.remote_default_name(gix::remote::Direction::Push) {
            Some(name) => {
                if let Some(out) = out.for_human() {
                    writeln!(
                        out,
                        "  {}",
                        format!("✓ Using existing push remote: {name}").green()
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
                            format!("✓ No remote HEAD found, using {remote_name}/{branch}")
                                .yellow()
                        )?;
                    }
                    format!("{remote_name}/{branch}")
                }
                None => {
                    anyhow::bail!("No HEAD reference found for remote {remote_name}");
                }
            }
        };

        drop(repo);
        but_api::legacy::virtual_branches::set_base_branch_with_perm(
            ctx,
            name.clone(),
            Some(remote_name.clone()),
            perm,
        )?;

        // Track target info for JSON output
        target_info = Some(TargetInfo {
            branch_name: name.clone(),
            remote_name: remote_name.clone(),
            newly_set: true,
        });

        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "  {}",
                format!("✓ Set default target to: {name}").green()
            )?;
            writeln!(out)?;
            writeln!(
                out,
                "{}",
                "GitButler project setup complete!".green().bold()
            )?;
            writeln!(out, "{}", format!("Target branch: {name}").dimmed())?;
            writeln!(out, "{}", format!("Remote: {remote_name}").dimmed())?;
            writeln!(out)?;
        }
    } else {
        // Target already exists
        if let Some(target) = &target {
            target_info = Some(TargetInfo {
                branch_name: target.branch_name.clone(),
                remote_name: target.remote_name.clone(),
                newly_set: false,
            });
        }

        if let Some(out) = out.for_human() {
            writeln!(out)?;
            writeln!(
                out,
                "{}",
                "GitButler project is already set up!".green().bold()
            )?;
            if let Some(target) = target {
                writeln!(
                    out,
                    "{}",
                    format!("Target branch: {}", target.branch_name).dimmed()
                )?;
            }
            writeln!(out)?;
        }
    };

    let head_name = {
        let repo = ctx.repo.get()?;
        let head = repo.head()?;
        head.referent_name()
            .map(|n| n.shorten().to_owned())
            .unwrap_or_default()
    };

    // switch to gitbutler/workspace if not already there
    if !head_name.starts_with(b"gitbutler/") {
        but_api::legacy::virtual_branches::switch_back_to_workspace_with_perm(ctx, perm)?;
    }

    // Install managed hooks to prevent accidental git commits.
    //
    // --no-hooks is a CLI-level bypass: persist opt-out, clean up, and skip.
    // --force-hooks and the default case delegate to the shared
    // ensure_managed_hooks() which handles detection, config persistence,
    // and cleanup consistently with the app flow.
    let mut hook_manager_info = None;
    let mut hooks_installed = false;

    if no_hooks {
        // Explicit CLI opt-out: persist and clean up.
        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "  {}",
                "Skipping hook installation (--no-hooks)".dimmed()
            )?;
        }

        let repo = ctx.repo.get()?;
        let hooks_dir = gitbutler_repo::managed_hooks::resolve_hooks_dir(&repo);
        if let Err(e) = gitbutler_repo::managed_hooks::uninstall_managed_hooks(&hooks_dir).context(
            "Failed to remove GitButler managed hooks while entering externally-managed hook mode",
        ) && let Some(out) = out.for_human()
        {
            writeln!(
                out,
                "  {}",
                format!("Warning: Failed to remove GitButler managed hooks: {e}").yellow()
            )?;
        }
    } else {
        // --force-hooks: re-enable in config before calling the shared function.
        // This is done here (after the workspace switch) rather than earlier,
        // because switch_back_to_workspace_with_perm() may call
        // ensure_managed_hooks(force=false) which would overwrite the setting.
        if force_hooks {
            let repo = ctx.repo.get()?;
            gitbutler_repo::managed_hooks::set_install_managed_hooks_enabled(&repo, true)?;
            if let Some(out) = out.for_human() {
                writeln!(
                    out,
                    "  {}",
                    "Forcing hook installation (--force-hooks), skipping detection.".dimmed()
                )?;
            }
        }

        let repo = ctx.repo.get()?;
        let hooks_dir = gitbutler_repo::managed_hooks::resolve_hooks_dir(&repo);
        let outcome =
            gitbutler_repo::managed_hooks::ensure_managed_hooks(&repo, &hooks_dir, force_hooks);

        match outcome {
            gitbutler_repo::managed_hooks::HookSetupOutcome::Installed => {
                hooks_installed = true;
            }
            gitbutler_repo::managed_hooks::HookSetupOutcome::DisabledByConfig => {
                // Previously configured opt-out (not --no-hooks, handled above).
                if let Some(out) = out.for_human() {
                    writeln!(
                        out,
                        "  {}",
                        "Skipping hook installation (--no-hooks is configured for this repository)"
                            .dimmed()
                    )?;
                    writeln!(
                        out,
                        "  {}",
                        "To switch back to GitButler-managed hooks: but setup --force-hooks"
                            .dimmed()
                    )?;
                }
            }
            gitbutler_repo::managed_hooks::HookSetupOutcome::ExternalManagerDetected {
                ref manager_name,
                ref instructions,
            } => {
                hook_manager_info = Some(HookManagerInfo {
                    name: manager_name.clone(),
                    hooks_installed: false,
                });
                if let Some(out) = out.for_human() {
                    writeln!(out)?;
                    writeln!(
                        out,
                        "  {}",
                        format!("Detected {manager_name} managing your git hooks.").cyan()
                    )?;
                    writeln!(
                        out,
                        "  {}",
                        "GitButler will not overwrite hooks owned by your hook manager.".dimmed()
                    )?;
                    writeln!(
                        out,
                        "  {}",
                        "This repository is now configured for externally-managed hooks.".dimmed()
                    )?;
                    writeln!(out)?;
                    writeln!(
                        out,
                        "  {}",
                        "To integrate GitButler's workspace guard with your hook manager:".yellow()
                    )?;
                    for line in instructions.lines() {
                        if line.is_empty() {
                            writeln!(out)?;
                        } else {
                            writeln!(out, "  {}", line.dimmed())?;
                        }
                    }
                    writeln!(out)?;
                    writeln!(
                        out,
                        "  {}",
                        "To switch back to GitButler-managed hooks: but setup --force-hooks"
                            .dimmed()
                    )?;
                    writeln!(out)?;
                }
            }
            gitbutler_repo::managed_hooks::HookSetupOutcome::HookSkipped { ref hook_name } => {
                if let Some(out) = out.for_human() {
                    writeln!(
                        out,
                        "  {}",
                        format!(
                            "Warning: Skipped {hook_name} — hook exists and is not GitButler-managed. \
                             Use --force-hooks to override."
                        )
                        .yellow()
                    )?;
                }
            }
            gitbutler_repo::managed_hooks::HookSetupOutcome::Failed { ref error } => {
                if let Some(out) = out.for_human() {
                    writeln!(
                        out,
                        "  {}",
                        format!("Warning: Failed to install GitButler managed hooks: {error}")
                            .yellow()
                    )?;
                }
            }
        }

        // Warn about orphaned hooks if core.hooksPath redirects to a different directory.
        // Only relevant when hooks were installed or attempted (not when an external
        // manager was detected, since those hooks are intentionally elsewhere).
        if matches!(
            outcome,
            gitbutler_repo::managed_hooks::HookSetupOutcome::Installed
                | gitbutler_repo::managed_hooks::HookSetupOutcome::HookSkipped { .. }
        ) {
            let default_hooks_dir = repo.git_dir().join("hooks");
            if hooks_dir != default_hooks_dir
                && gitbutler_repo::managed_hooks::has_managed_hooks_in(&default_hooks_dir)
                && let Some(out) = out.for_human()
            {
                writeln!(
                    out,
                    "  {}",
                    format!(
                        "Warning: GitButler-managed hooks found in old hooks directory ({}).",
                        default_hooks_dir.display()
                    )
                    .yellow()
                )?;
                writeln!(
                    out,
                    "  {}",
                    format!(
                        "core.hooksPath is now set to {}, so those hooks are orphaned.",
                        hooks_dir.display()
                    )
                    .dimmed()
                )?;
                writeln!(
                    out,
                    "  {}",
                    "You can safely remove them with: rm .git/hooks/pre-commit .git/hooks/post-checkout .git/hooks/pre-push"
                        .dimmed()
                )?;
            }
        }
    }

    // if we switched - tell the user what this is all about
    if pre_head_name != "gitbutler/workspace"
        && let Some(out) = out.for_human()
    {
        let hooks_note = if hooks_installed {
            "\n- Installing Git hooks to help manage commits on the workspace branch"
        } else {
            ""
        };
        writeln!(
            out,
            "{}",
            format!(
                r#"
Setting up your project for GitButler tooling. Some things to note:

- Switching you to a special `gitbutler/workspace` branch to enable parallel branches{hooks_note}

To undo these changes and return to normal Git mode, either:

    - Directly checkout a branch (`git checkout {pre_head_name}`)
    - Run `but teardown`

More info: https://docs.gitbutler.com/workspace-branch
"#
            )
            .yellow()
        )?;
    }

    // Display splash screen for human output
    if let Some(out) = out.for_human() {
        display_splash_screen(out)?;
    }

    // Output JSON if requested
    if let Some(json_out) = out.for_json() {
        let result = SetupResult {
            repository_path: path::absolute(repo_path)?.display().to_string(),
            project_status,
            target: target_info,
            hook_manager: hook_manager_info,
        };
        json_out.write_value(&result)?;
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
/// - if we're on a gitbutler/* branch
pub fn check_project_setup(ctx: &Context, perm: &RepoShared) -> anyhow::Result<bool> {
    let (repo, ws, _) = ctx.workspace_and_db_with_perm(perm)?;

    // check if we're on a gitbutler/* branch
    let head = repo.head()?;
    let head_name = head
        .referent_name()
        .map(|n| n.shorten().to_owned())
        .unwrap_or_default();
    if !head_name.starts_with(b"gitbutler/") {
        anyhow::bail!("Not currently on a gitbutler/* branch.");
    }

    // When on gitbutler/edit, the project was already set up when entering edit mode.
    // The workspace graph built from gitbutler/edit doesn't expose the target ref or
    // remote configuration, but both are still configured in virtual_branches.toml
    // and will be accessible when returning to gitbutler/workspace.
    if head_name == b"gitbutler/edit" {
        return Ok(true);
    }

    // TODO(legacy): it's fine to have no target.
    if ws.target_ref.is_none() {
        anyhow::bail!("No default target branch set.");
    }

    // check if there is a remote
    if ws.remote_name().is_none()
        && repo
            .remote_default_name(gix::remote::Direction::Push)
            .is_none()
    {
        anyhow::bail!(
            "Neither found push remote found in workspace nor unambiguously in the Git repository configuration."
        )
    };

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
            "No push remote found, creating gb-local remote...".blue()
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
            format!("✓ Created gb-local remote tracking {default_branch_name}").green()
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
