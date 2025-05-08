use anyhow::{Context, anyhow, bail};
use but_core::UnifiedDiff;
use but_workspace::commit_engine::{DiffSpec, HunkHeader};
use gitbutler_project::Project;
use gix::bstr::{BString, ByteSlice};
use std::path::Path;

pub(crate) const UI_CONTEXT_LINES: u32 = 3;

pub fn project_from_path(path: &Path) -> anyhow::Result<Project> {
    Project::from_path(path)
}

pub fn project_repo(path: &Path) -> anyhow::Result<gix::Repository> {
    let project = project_from_path(path)?;
    configured_repo(
        gix::open(project.worktree_path())?,
        RepositoryOpenMode::General,
    )
}

pub enum RepositoryOpenMode {
    Merge,
    General,
}

fn configured_repo(
    mut repo: gix::Repository,
    mode: RepositoryOpenMode,
) -> anyhow::Result<gix::Repository> {
    match mode {
        RepositoryOpenMode::Merge => {
            let bytes = repo.compute_object_cache_size_for_tree_diffs(&***repo.index_or_empty()?);
            repo.object_cache_size_if_unset(bytes);
        }
        RepositoryOpenMode::General => {
            repo.object_cache_size_if_unset(512 * 1024);
        }
    }
    Ok(repo)
}

/// Operate like GitButler would in the future, on a Git repository and optionally with additional metadata as obtained
/// from the previously added project.
pub fn repo_and_maybe_project(
    args: &super::Args,
    mode: RepositoryOpenMode,
) -> anyhow::Result<(gix::Repository, Option<Project>)> {
    let repo = configured_repo(gix::discover(&args.current_dir)?, mode)?;
    let res = if let Some((projects, work_dir)) =
        project_controller(args.app_suffix.as_deref(), args.app_data_dir.as_deref())
            .ok()
            .zip(repo.workdir())
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

pub fn parse_diff_spec(arg: &Option<String>) -> Result<Option<Vec<DiffSpec>>, anyhow::Error> {
    arg.as_deref()
        .map(|value| {
            serde_json::from_str::<Vec<but_workspace::commit_engine::ui::DiffSpec>>(value)
                .map(|diff_spec| diff_spec.into_iter().map(Into::into).collect())
                .map_err(|e| anyhow!("Failed to parse diff_spec: {}", e))
        })
        .transpose()
}

mod commit;
use crate::command::discard_change::IndicesOrHeaders;
pub use commit::commit;

pub mod diff;

pub mod stacks {
    use std::{path::Path, str::FromStr};

    use but_settings::AppSettings;
    use but_workspace::{
        stack_branch_local_and_remote_commits, stack_branch_upstream_only_commits, stack_branches,
        ui,
    };
    use gitbutler_command_context::CommandContext;
    use gitbutler_id::id::Id;

    use crate::command::{debug_print, project_from_path};

    /// A collection of all the commits that are part of a branch.
    #[derive(Debug, Clone, serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct BranchCommits {
        /// The commits that are local and optionally remote.
        pub local_and_remote: Vec<ui::Commit>,
        /// The commits that are only at the remote.
        pub upstream_commits: Vec<ui::UpstreamCommit>,
    }

    pub fn list(current_dir: &Path, use_json: bool) -> anyhow::Result<()> {
        let project = project_from_path(current_dir)?;
        let ctx = CommandContext::open(&project, AppSettings::default())?;
        let repo = ctx.gix_repo()?;
        let stacks = but_workspace::stacks(&ctx, &project.gb_dir(), &repo, Default::default())?;
        if use_json {
            let json = serde_json::to_string_pretty(&stacks)?;
            println!("{json}");
            Ok(())
        } else {
            debug_print(stacks)
        }
    }

    pub fn branches(id: &str, current_dir: &Path, use_json: bool) -> anyhow::Result<()> {
        let project = project_from_path(current_dir)?;
        let ctx = CommandContext::open(&project, AppSettings::default())?;
        let branches = stack_branches(id.to_string(), &ctx)?;
        if use_json {
            let json = serde_json::to_string_pretty(&branches)?;
            println!("{json}");
            Ok(())
        } else {
            debug_print(branches)
        }
    }

    /// Create a new stack containing only a branch with the given name.
    fn create_stack_with_branch(
        ctx: &CommandContext,
        name: &str,
        description: &Option<String>,
    ) -> anyhow::Result<ui::StackEntry> {
        let creation_request = gitbutler_branch::BranchCreateRequest {
            name: Some(name.to_string()),
            ..Default::default()
        };
        let stack_entry = gitbutler_branch_actions::create_virtual_branch(ctx, &creation_request)?;

        if description.is_some() {
            gitbutler_branch_actions::stack::update_branch_description(
                ctx,
                stack_entry.id,
                name.to_string(),
                description.clone(),
            )?;
        }

        Ok(stack_entry)
    }

    /// Add a branch to an existing stack.
    fn add_branch_to_stack(
        ctx: &CommandContext,
        id: &str,
        name: &str,
        description: &Option<String>,
        project: gitbutler_project::Project,
        repo: &gix::Repository,
    ) -> anyhow::Result<ui::StackEntry> {
        let creation_request = gitbutler_branch_actions::stack::CreateSeriesRequest {
            name: name.to_string(),
            description: None,
            target_patch: None,
            preceding_head: None,
        };

        let stack_id = Id::from_str(id)?;
        gitbutler_branch_actions::stack::create_branch(ctx, stack_id, creation_request)?;
        let stack_entries =
            but_workspace::stacks(ctx, &project.gb_dir(), repo, Default::default())?;

        let stack_entry = stack_entries
            .into_iter()
            .find(|entry| entry.id == stack_id)
            .ok_or_else(|| anyhow::anyhow!("Failed to find stack with ID: {id}"))?;

        if description.is_some() {
            gitbutler_branch_actions::stack::update_branch_description(
                ctx,
                stack_entry.id,
                name.to_string(),
                description.clone(),
            )?;
        }

        Ok(stack_entry)
    }

    /// Create a new branch in the current project.
    ///
    /// If `id` is provided, it will be used to add the branch to an existing stack.
    /// If `id` is not provided, a new stack will be created with the branch.
    pub fn create_branch(
        id: &Option<String>,
        name: &str,
        description: &Option<String>,
        current_dir: &Path,
        use_json: bool,
    ) -> anyhow::Result<()> {
        let project = project_from_path(current_dir)?;
        // Enable v3 feature flags for the command context
        let app_settings = AppSettings {
            feature_flags: but_settings::app_settings::FeatureFlags { v3: true },
            ..AppSettings::default()
        };

        let ctx = CommandContext::open(&project, app_settings)?;
        let repo = ctx.gix_repo()?;

        let stack_entry = match id {
            Some(id) => add_branch_to_stack(&ctx, id, name, description, project.clone(), &repo)?,
            None => create_stack_with_branch(&ctx, name, description)?,
        };

        if use_json {
            let json = serde_json::to_string_pretty(&stack_entry)?;
            println!("{json}");
            Ok(())
        } else {
            debug_print(stack_entry)
        }
    }

    pub fn branch_commits(
        id: &str,
        name: &str,
        current_dir: &Path,
        use_json: bool,
    ) -> anyhow::Result<()> {
        let project = project_from_path(current_dir)?;
        let ctx = CommandContext::open(&project, AppSettings::default())?;
        let repo = ctx.gix_repo()?;
        let local_and_remote =
            stack_branch_local_and_remote_commits(id.to_string(), name.to_string(), &ctx, &repo);
        let upstream_only =
            stack_branch_upstream_only_commits(id.to_string(), name.to_string(), &ctx, &repo);

        if use_json {
            let branch_commits = BranchCommits {
                local_and_remote: local_and_remote?,
                upstream_commits: upstream_only?,
            };

            let json = serde_json::to_string_pretty(&branch_commits)?;
            println!("{json}");
            Ok(())
        } else {
            debug_print(local_and_remote)?;
            debug_print(upstream_only)
        }
    }
}

pub(crate) mod discard_change {
    pub enum IndicesOrHeaders<'a> {
        Indices(&'a [usize]),
        Headers(&'a [u32]),
    }
}
pub(crate) fn discard_change(
    cwd: &Path,
    current_rela_path: &Path,
    previous_rela_path: Option<&Path>,
    indices_or_headers: Option<discard_change::IndicesOrHeaders<'_>>,
) -> anyhow::Result<()> {
    let repo = configured_repo(gix::discover(cwd)?, RepositoryOpenMode::Merge)?;

    let previous_path = previous_rela_path.map(path_to_rela_path).transpose()?;
    let path = path_to_rela_path(current_rela_path)?;
    let hunk_headers = indices_or_headers_to_hunk_headers(
        &repo,
        indices_or_headers,
        &path,
        previous_path.as_ref(),
    )?;
    let spec = but_workspace::commit_engine::DiffSpec {
        previous_path,
        path,
        hunk_headers,
    };
    debug_print(but_workspace::discard_workspace_changes(
        &repo,
        Some(spec.into()),
        UI_CONTEXT_LINES,
    )?)
}

fn indices_or_headers_to_hunk_headers(
    repo: &gix::Repository,
    indices_or_headers: Option<IndicesOrHeaders<'_>>,
    path: &BString,
    previous_path: Option<&BString>,
) -> anyhow::Result<Vec<HunkHeader>> {
    let headers = match indices_or_headers {
        None => vec![],
        Some(discard_change::IndicesOrHeaders::Headers(headers)) => headers
            .windows(4)
            .map(|n| HunkHeader {
                old_start: n[0],
                old_lines: n[1],
                new_start: n[2],
                new_lines: n[3],
            })
            .collect(),
        Some(discard_change::IndicesOrHeaders::Indices(hunk_indices)) => {
            let worktree_changes = but_core::diff::worktree_changes(repo)?
                .changes
                .into_iter()
                .find(|change| {
                    change.path == *path
                        && change.previous_path() == previous_path.as_ref().map(|p| p.as_bstr())
                }).with_context(|| format!("Couldn't find worktree change for file at '{path}' (previous-path: {previous_path:?}"))?;
            let UnifiedDiff::Patch { hunks, .. } =
                worktree_changes.unified_diff(repo, UI_CONTEXT_LINES)?
            else {
                bail!("No hunks available for given '{path}'")
            };

            hunk_indices
                .iter()
                .map(|idx| {
                    hunks.get(*idx).cloned().map(Into::into).ok_or_else(|| {
                        anyhow!(
                            "There was no hunk at index {idx} in '{path}' with {} hunks",
                            hunks.len()
                        )
                    })
                })
                .collect::<Result<Vec<HunkHeader>, _>>()?
        }
    };
    Ok(headers)
}

fn path_to_rela_path(path: &Path) -> anyhow::Result<BString> {
    if !path.is_relative() {
        bail!(
            "Can't currently convert absolute path to relative path (but this could be done via gix, just not as easily as I'd like right now"
        );
    }
    let rela_path =
        gix::path::to_unix_separators_on_windows(gix::path::os_str_into_bstr(path.as_os_str())?)
            .into_owned();
    Ok(rela_path)
}
