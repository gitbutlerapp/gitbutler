use std::{borrow::Cow, mem::ManuallyDrop, path::Path};

use anyhow::{Context as _, anyhow, bail};
use but_core::{
    DiffSpec, HunkHeader, UnifiedPatch, ref_metadata::StackId,
    worktree::checkout::UncommitedWorktreeChanges,
};
use but_db::poll::ItemKind;
use but_meta::VirtualBranchesTomlMetadata;
use but_settings::AppSettings;
use but_workspace::branch::{
    OnWorkspaceMergeConflict,
    apply::{WorkspaceMerge, WorkspaceReferenceNaming},
    create_reference::{Anchor, Position},
};
use gitbutler_project::{Project, ProjectId};
use gix::{
    bstr::{BString, ByteSlice},
    refs::Category,
};
use tokio::sync::mpsc::unbounded_channel;

pub(crate) const UI_CONTEXT_LINES: u32 = 3;

pub fn project_from_path(path: &Path) -> anyhow::Result<Project> {
    Project::from_path(path)
}

pub fn project_repo(path: &Path) -> anyhow::Result<gix::Repository> {
    let project = project_from_path(path)?;
    configured_repo(project.open_repo()?, RepositoryOpenMode::General)
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

fn ref_metadata_toml(project: &Project) -> anyhow::Result<VirtualBranchesTomlMetadata> {
    VirtualBranchesTomlMetadata::from_path(project.gb_dir().join("virtual_branches.toml"))
}

/// Operate like GitButler would in the future, on a Git repository and optionally with additional metadata as obtained
/// from the previously added project.
pub fn repo_and_maybe_project(
    args: &super::Args,
    mode: RepositoryOpenMode,
) -> anyhow::Result<(gix::Repository, Option<Project>)> {
    let repo = configured_repo(gix::discover(&args.current_dir)?, mode)?;
    let res = if let Some(work_dir) = repo.workdir() {
        let work_dir = gix::path::realpath(work_dir)?;
        (
            repo,
            gitbutler_project::Project::find_by_worktree_dir(&work_dir).ok(),
        )
    } else {
        (repo, None)
    };
    Ok(res)
}

pub fn repo_and_maybe_project_and_graph(
    args: &super::Args,
    mode: RepositoryOpenMode,
) -> anyhow::Result<(
    gix::Repository,
    Option<Project>,
    but_graph::Graph,
    ManuallyDrop<VirtualBranchesTomlMetadata>,
)> {
    let (repo, project) = repo_and_maybe_project(args, mode)?;
    let meta = meta_from_maybe_project(project.as_ref())?;
    let graph = but_graph::Graph::from_head(&repo, &*meta, Default::default())?;
    Ok((repo, project, graph, meta))
}

fn debug_print(this: impl std::fmt::Debug) -> anyhow::Result<()> {
    println!("{this:#?}");
    Ok(())
}

pub fn parse_diff_spec(arg: &Option<String>) -> Result<Option<Vec<DiffSpec>>, anyhow::Error> {
    arg.as_deref()
        .map(|value| {
            serde_json::from_str::<Vec<but_core::DiffSpec>>(value)
                .map(|diff_spec| diff_spec.into_iter().collect())
                .map_err(|e| anyhow!("Failed to parse diff_spec: {}", e))
        })
        .transpose()
}

mod commit;
use but_ctx::Context;
pub use commit::commit;
use gitbutler_branch_actions::BranchListingFilter;

use crate::command::discard_change::IndicesOrHeaders;

pub mod diff;
pub mod project;

pub mod assignment {
    use std::path::Path;

    use but_ctx::Context;
    use but_hunk_assignment::HunkAssignmentRequest;
    use but_settings::AppSettings;

    use crate::command::{debug_print, project_from_path};

    pub fn hunk_assignments(current_dir: &Path, use_json: bool) -> anyhow::Result<()> {
        let project = project_from_path(current_dir)?;
        let ctx =
            &mut Context::new_from_legacy_project_and_settings(&project, AppSettings::default());
        let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
            ctx,
            false,
            None::<Vec<but_core::TreeChange>>,
            None,
        )?;
        if use_json {
            let json = serde_json::to_string_pretty(&assignments)?;
            println!("{json}");
            Ok(())
        } else {
            debug_print(assignments)
        }
    }

    pub fn assign_hunk(
        current_dir: &Path,
        use_json: bool,
        assignment: HunkAssignmentRequest,
    ) -> anyhow::Result<()> {
        let project = project_from_path(current_dir)?;
        let ctx =
            &mut Context::new_from_legacy_project_and_settings(&project, AppSettings::default());
        let rejections = but_hunk_assignment::assign(ctx, vec![assignment], None)?;
        if use_json {
            let json = serde_json::to_string_pretty(&rejections)?;
            println!("{json}");
            Ok(())
        } else {
            debug_print(rejections)
        }
    }
}

pub mod stacks {
    use std::{path::Path, str::FromStr};

    use anyhow::Context as _;
    use but_ctx::Context;
    use but_settings::AppSettings;
    use but_workspace::legacy::{StacksFilter, stack_branches, ui};
    use gitbutler_reference::{Refname, RemoteRefname};
    use gitbutler_stack::StackId;
    use gix::bstr::ByteSlice;

    use crate::command::{debug_print, project_from_path, ref_metadata_toml};

    pub fn list(current_dir: &Path, use_json: bool, in_workspace: bool) -> anyhow::Result<()> {
        let project = project_from_path(current_dir)?;
        let ctx = Context::new_from_legacy_project_and_settings(&project, AppSettings::default());
        let repo = ctx.clone_repo_for_merging_non_persisting()?;
        let filter = if in_workspace {
            StacksFilter::InWorkspace
        } else {
            StacksFilter::All
        };
        let stacks = {
            let meta = ref_metadata_toml(&ctx.legacy_project)?;
            but_workspace::legacy::stacks_v3(&repo, &meta, filter, None)
        }?;
        if use_json {
            let json = serde_json::to_string_pretty(&stacks)?;
            println!("{json}");
            Ok(())
        } else {
            debug_print(stacks)
        }
    }

    pub fn details(id: Option<StackId>, current_dir: &Path) -> anyhow::Result<()> {
        let project = project_from_path(current_dir)?;
        let ctx = Context::new_from_legacy_project_and_settings(&project, AppSettings::default());
        let details = {
            let meta = ref_metadata_toml(&ctx.legacy_project)?;
            let repo = ctx.clone_repo_for_merging_non_persisting()?;
            but_workspace::legacy::stack_details_v3(id, &repo, &meta)
        }?;
        debug_print(details)
    }

    pub fn branch_details(ref_name: &str, current_dir: &Path) -> anyhow::Result<()> {
        let project = project_from_path(current_dir)?;
        let ctx = Context::new_from_legacy_project_and_settings(&project, AppSettings::default());
        let meta = ref_metadata_toml(&ctx.legacy_project)?;
        let repo = ctx.clone_repo_for_merging_non_persisting()?;
        let ref_name = repo.find_reference(ref_name)?.name().to_owned();

        let details = { but_workspace::branch_details(&repo, ref_name.as_ref(), &meta) }?;
        debug_print(details)
    }

    pub fn branches(id: StackId, current_dir: &Path, use_json: bool) -> anyhow::Result<()> {
        let project = project_from_path(current_dir)?;
        let ctx = Context::new_from_legacy_project_and_settings(&project, AppSettings::default());
        let branches = stack_branches(id, &ctx)?;
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
        ctx: &Context,
        name: &str,
        remote: bool,
        description: Option<&str>,
    ) -> anyhow::Result<ui::StackEntry> {
        let repo = ctx.repo.get()?;
        let remotes = repo.remote_names();
        if remote {
            let remote_name = remotes
                .first()
                .map(|r| r.to_str().unwrap())
                .context("No remote found in repository")?;

            let ref_name = Refname::from_str(&format!("refs/remotes/{remote_name}/{name}"))?;
            let remote_ref_name = RemoteRefname::new(remote_name, name);

            let (stack_id, _, _) = gitbutler_branch_actions::create_virtual_branch_from_branch(
                ctx,
                &ref_name,
                Some(remote_ref_name),
                None,
            )?;

            let meta = but_meta::VirtualBranchesTomlMetadata::from_path(
                ctx.legacy_project.gb_dir().join("virtual_branches.toml"),
            )?;
            let stack_entries =
                but_workspace::legacy::stacks_v3(&repo, &meta, Default::default(), None)?;
            let stack_entry = stack_entries
                .into_iter()
                .find(|entry| entry.id == Some(stack_id))
                .ok_or_else(|| {
                    anyhow::anyhow!("Failed to find newly created stack with ID: {stack_id}")
                })?;
            return Ok(stack_entry);
        };

        let creation_request = gitbutler_branch::BranchCreateRequest {
            name: Some(name.to_string()),
            ..Default::default()
        };
        let stack_entry = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &creation_request,
            ctx.exclusive_worktree_access().write_permission(),
        )?;

        if description.is_some() {
            gitbutler_branch_actions::stack::update_branch_description(
                ctx,
                stack_entry.id,
                name.to_string(),
                description.map(ToOwned::to_owned),
            )?;
        }

        Ok(stack_entry.into())
    }

    /// Add a branch to an existing stack.
    fn add_branch_to_stack(
        ctx: &Context,
        stack_id: StackId,
        name: &str,
        description: Option<&str>,
        repo: &gix::Repository,
    ) -> anyhow::Result<ui::StackEntry> {
        let creation_request = gitbutler_branch_actions::stack::CreateSeriesRequest {
            name: name.to_string(),
            description: None,
            target_patch: None,
            preceding_head: None,
        };

        gitbutler_branch_actions::stack::create_branch(ctx, stack_id, creation_request)?;
        let meta = but_meta::VirtualBranchesTomlMetadata::from_path(
            ctx.legacy_project.gb_dir().join("virtual_branches.toml"),
        )?;
        let stack_entries =
            but_workspace::legacy::stacks_v3(repo, &meta, Default::default(), None)?;

        let stack_entry = stack_entries
            .into_iter()
            .find(|entry| entry.id == Some(stack_id))
            .ok_or_else(|| anyhow::anyhow!("Failed to find stack with ID: {stack_id}"))?;

        if description.is_some() {
            gitbutler_branch_actions::stack::update_branch_description(
                ctx,
                stack_entry.id.context("BUG(opt-stack-id)")?,
                name.to_string(),
                description.map(ToOwned::to_owned),
            )?;
        }

        Ok(stack_entry)
    }

    /// Add a branch to an existing stack by looking up the stack by name.
    pub fn move_branch(
        subject_branch: &str,
        destination_branch: &str,
        current_dir: &Path,
    ) -> anyhow::Result<()> {
        let project = project_from_path(current_dir)?;
        // Enable v3 feature flags for the command context
        let app_settings = AppSettings {
            feature_flags: but_settings::app_settings::FeatureFlags {
                apply3: false,
                cv3: false,
                undo: false,
                rules: false,
                single_branch: false,
            },
            ..AppSettings::default()
        };

        let ctx = Context::new_from_legacy_project_and_settings(&project, app_settings);
        let repo = ctx.repo.get()?;

        let meta = but_meta::VirtualBranchesTomlMetadata::from_path(
            ctx.legacy_project.gb_dir().join("virtual_branches.toml"),
        )?;
        let stacks = but_workspace::legacy::stacks_v3(&repo, &meta, Default::default(), None)?;
        let subject_stack = stacks
            .clone()
            .into_iter()
            .find(|s| s.heads.iter().any(|h| h.name == subject_branch))
            .context(format!(
                "No stack branch found with name '{subject_branch}'"
            ))?;

        let destination_stack = stacks
            .into_iter()
            .find(|s| s.heads.iter().any(|h| h.name == destination_branch))
            .context(format!(
                "No stack branch found with name '{destination_branch}'"
            ))?;

        let outcome = gitbutler_branch_actions::move_branch(
            &ctx,
            destination_stack
                .id
                .context("BUG(opt-destination-stack-id)")?,
            destination_branch,
            subject_stack.id.context("BUG(opt-subject-stack-id)")?,
            subject_branch,
        )?;

        debug_print(outcome)
    }

    /// Create a new branch in the current project.
    ///
    /// If `id` is provided, it will be used to add the branch to an existing stack.
    /// If `id` is not provided, a new stack will be created with the branch.
    pub fn create_branch(
        id: Option<StackId>,
        name: &str,
        description: Option<&str>,
        current_dir: &Path,
        remote: bool,
        use_json: bool,
    ) -> anyhow::Result<()> {
        let project = project_from_path(current_dir)?;
        // Enable v3 feature flags for the command context
        let app_settings = AppSettings {
            feature_flags: but_settings::app_settings::FeatureFlags {
                apply3: false,
                cv3: false,
                undo: false,
                rules: false,
                single_branch: false,
            },
            ..AppSettings::default()
        };

        let ctx = Context::new_from_legacy_project_and_settings(&project, app_settings);
        let repo = ctx.repo.get()?;

        let stack_entry = match id {
            Some(id) => add_branch_to_stack(&ctx, id, name, description, &repo)?,
            None => create_stack_with_branch(&ctx, name, remote, description)?,
        };

        if use_json {
            let json = serde_json::to_string_pretty(&stack_entry)?;
            println!("{json}");
            Ok(())
        } else {
            debug_print(stack_entry)
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
    let spec = but_core::DiffSpec {
        previous_path,
        path,
        hunk_headers,
    };
    debug_print(but_workspace::discard_workspace_changes(
        &repo,
        Some(spec),
        UI_CONTEXT_LINES,
    )?)
}

pub async fn watch(args: &super::Args) -> anyhow::Result<()> {
    let (repo, project) = repo_and_maybe_project(args, RepositoryOpenMode::General)?;
    let (tx, mut rx) = unbounded_channel();
    let start = std::time::Instant::now();
    let workdir = repo
        .workdir()
        .context("really only want to watch workdirs")?;
    let _watcher = gitbutler_filemonitor::spawn(
        project.map(|p| p.id).unwrap_or(ProjectId::generate()),
        workdir,
        tx,
    )?;
    let elapsed = start.elapsed();
    eprintln!(
        "Started watching {workdir} in {elapsed:?}s - waiting for events",
        elapsed = elapsed.as_secs_f32(),
        workdir = workdir.display(),
    );

    while let Some(event) = rx.recv().await {
        debug_print(event).ok();
    }
    Ok(())
}
pub fn watch_db(args: &super::Args) -> anyhow::Result<()> {
    let (_repo, project) = repo_and_maybe_project(args, RepositoryOpenMode::General)?;
    let project = project.context("Couldn't find GitButler project in directory, needed here")?;
    let ctx = Context::new_from_legacy_project_and_settings(&project, AppSettings::default());
    let db = ctx.db.get()?;
    let rx = db.poll_changes(
        ItemKind::Actions | ItemKind::Assignments | ItemKind::Workflows,
        std::time::Duration::from_millis(500),
    )?;
    eprintln!("Press Ctrl + C to abort");
    for event in rx {
        eprintln!("{event:?} changed");
    }
    eprintln!("subscription stopped unexpectedly");
    Ok(())
}

pub fn operating_mode(args: &super::Args) -> anyhow::Result<()> {
    let (_repo, project) = repo_and_maybe_project(args, RepositoryOpenMode::General)?;
    let project = project.context("Couldn't find GitButler project in directory")?;
    let ctx = Context::new_from_legacy_project_and_settings(&project, AppSettings::default());

    debug_print(gitbutler_operating_modes::operating_mode(&ctx))
}

pub fn ref_info(args: &super::Args, ref_name: Option<&str>, expensive: bool) -> anyhow::Result<()> {
    let (repo, project) = repo_and_maybe_project(args, RepositoryOpenMode::Merge)?;
    let opts = but_workspace::ref_info::Options {
        expensive_commit_info: expensive,
        traversal: Default::default(),
    };

    let project = project.with_context(|| {
        format!(
            "Currently there must be an official project so we have metadata: {project_dir}",
            project_dir = args.current_dir.display()
        )
    })?;
    let meta = ref_metadata_toml(&project)?;
    debug_print(match ref_name {
        None => but_workspace::head_info(&repo, &meta, opts),
        Some(ref_name) => but_workspace::ref_info(repo.find_reference(ref_name)?, &meta, opts),
    }?)
}

pub mod graph;

/// NOTE: THis is a special case while vb.toml is used and while projects are somewhat special.
fn meta_from_maybe_project(
    project: Option<&Project>,
) -> anyhow::Result<ManuallyDrop<VirtualBranchesTomlMetadata>> {
    let meta = ManuallyDrop::new(match project {
        None => VirtualBranchesTomlMetadata::from_path("should-never-be-written-back.toml")?,
        Some(project) => ref_metadata_toml(project)?,
    });
    Ok(meta)
}

pub fn remove_reference(
    args: &super::Args,
    short_name: &str,
    opts: but_workspace::branch::remove_reference::Options,
) -> anyhow::Result<()> {
    let (repo, _project, graph, mut meta) =
        repo_and_maybe_project_and_graph(args, RepositoryOpenMode::General)?;

    let ref_name = Category::LocalBranch.to_full_name(short_name)?;
    let deleted = but_workspace::branch::remove_reference(
        ref_name.as_ref(),
        &repo,
        &graph.to_workspace()?,
        &mut *meta,
        opts,
    )?
    .is_some();
    if deleted {
        eprintln!("Deleted");
    } else {
        eprintln!("Nothing deleted");
    }
    // write metadata if there are projects - this is a special case while we use vb.toml.
    ManuallyDrop::into_inner(meta);
    Ok(())
}

pub fn apply(args: &super::Args, short_name: &str, order: Option<usize>) -> anyhow::Result<()> {
    let (repo, project, graph, mut meta) =
        repo_and_maybe_project_and_graph(args, RepositoryOpenMode::Merge)?;
    let branch = repo.find_reference(short_name)?;
    let ws = graph.to_workspace()?;
    let apply_outcome = but_workspace::branch::apply(
        branch.name(),
        &ws,
        &repo,
        &mut *meta,
        but_workspace::branch::apply::Options {
            workspace_merge: WorkspaceMerge::AlwaysMerge,
            on_workspace_conflict: OnWorkspaceMergeConflict::MaterializeAndReportConflictingStacks,
            workspace_reference_naming: WorkspaceReferenceNaming::Default,
            uncommitted_changes: UncommitedWorktreeChanges::KeepAndAbortOnConflict,
            order,
            new_stack_id: None,
        },
    )?;

    if project.is_some() {
        // write metadata if there are projects - this is a special case while we use vb.toml.
        ManuallyDrop::into_inner(meta);
    }
    debug_print(apply_outcome)
}

pub fn create_reference(
    args: &super::Args,
    short_name: &str,
    above: Option<&str>,
    below: Option<&str>,
) -> anyhow::Result<()> {
    let (repo, project, graph, mut meta) =
        repo_and_maybe_project_and_graph(args, RepositoryOpenMode::General)?;
    let resolve = |spec: &str, position: Position| -> anyhow::Result<Anchor<'_>> {
        Ok(match repo.try_find_reference(spec)? {
            None => Anchor::AtCommit {
                commit_id: repo.rev_parse_single(spec)?.detach(),
                position,
            },
            Some(rn) => Anchor::AtSegment {
                ref_name: Cow::Owned(rn.inner.name),
                position,
            },
        })
    };
    let anchor = above
        .map(|spec| resolve(spec, Position::Above))
        .or_else(|| below.map(|spec| resolve(spec, Position::Below)))
        .transpose()?;

    let new_ref = Category::LocalBranch.to_full_name(short_name)?;
    let ws = graph.to_workspace()?;
    _ = but_workspace::branch::create_reference(
        new_ref.as_ref(),
        anchor,
        &repo,
        &ws,
        &mut *meta,
        |_| StackId::generate(),
        None,
    )?;

    if project.is_some() {
        // write metadata if there are projects - this is a special case while we use vb.toml.
        ManuallyDrop::into_inner(meta);
    }
    Ok(())
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
                    change.path == *path && change.previous_path() == previous_path.as_ref().map(|p| p.as_bstr())
                })
                .with_context(|| {
                    format!("Couldn't find worktree change for file at '{path}' (previous-path: {previous_path:?}")
                })?;
            let Some(UnifiedPatch::Patch { hunks, .. }) =
                worktree_changes.unified_patch(repo, UI_CONTEXT_LINES)?
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

pub fn branch_list(project: Option<Project>) -> anyhow::Result<()> {
    let project = project.context("legacy code needs project")?;
    let ctx = Context::new_from_legacy_project_and_settings(&project, AppSettings::default());
    debug_print(gitbutler_branch_actions::list_branches(
        &ctx,
        Some(BranchListingFilter {
            local: None,
            applied: None,
        }),
        None,
    )?)
}
