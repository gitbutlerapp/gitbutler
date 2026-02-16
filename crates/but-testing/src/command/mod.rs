use std::{borrow::Cow, path::Path};

use anyhow::{Context as _, anyhow, bail};
use but_core::{
    DiffSpec, HunkHeader, RepositoryExt, UnifiedPatch, ref_metadata::StackId,
    worktree::checkout::UncommitedWorktreeChanges,
};
use but_db::poll::ItemKind;
use but_workspace::branch::{
    OnWorkspaceMergeConflict,
    apply::{WorkspaceMerge, WorkspaceReferenceNaming},
    create_reference::{Anchor, Position},
};
use gitbutler_project::ProjectId;
use gix::{
    bstr::{BString, ByteSlice},
    refs::Category,
};
use tokio::sync::mpsc::unbounded_channel;

pub(crate) const UI_CONTEXT_LINES: u32 = 3;

fn debug_print(this: impl std::fmt::Debug) -> anyhow::Result<()> {
    println!("{this:#?}");
    Ok(())
}

pub fn parse_diff_spec(arg: &Option<String>) -> Result<Option<Vec<DiffSpec>>, anyhow::Error> {
    arg.as_deref()
        .map(|value| {
            serde_json::from_str::<Vec<but_core::DiffSpec>>(value)
                .map(|diff_spec| diff_spec.into_iter().collect())
                .map_err(|e| anyhow!("Failed to parse diff_spec: {e}"))
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

    use crate::command::debug_print;

    pub fn hunk_assignments(current_dir: &Path, use_json: bool) -> anyhow::Result<()> {
        let mut ctx = Context::discover(current_dir)?;
        let context_lines = ctx.settings.context_lines;
        let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
        let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
            db.hunk_assignments_mut()?,
            &repo,
            &ws,
            false,
            None::<Vec<but_core::TreeChange>>,
            None,
            context_lines,
        )?;
        if use_json {
            let json = serde_json::to_string_pretty(&assignments)?;
            println!("{json}");
            Ok(())
        } else {
            debug_print(assignments)
        }
    }

    pub fn assign_hunk(current_dir: &Path, use_json: bool, assignment: HunkAssignmentRequest) -> anyhow::Result<()> {
        let mut ctx = Context::discover(current_dir)?;
        let context_lines = ctx.settings.context_lines;
        let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
        let rejections = but_hunk_assignment::assign(
            db.hunk_assignments_mut()?,
            &repo,
            &ws,
            vec![assignment],
            None,
            context_lines,
        )?;
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
    use but_workspace::legacy::{StacksFilter, stack_branches, ui};
    use gitbutler_reference::{Refname, RemoteRefname};
    use gitbutler_stack::StackId;
    use gix::bstr::ByteSlice;

    use crate::command::debug_print;

    pub fn list(current_dir: &Path, use_json: bool, in_workspace: bool) -> anyhow::Result<()> {
        let ctx = Context::discover(current_dir)?;
        let repo = ctx.clone_repo_for_merging_non_persisting()?;
        let filter = if in_workspace {
            StacksFilter::InWorkspace
        } else {
            StacksFilter::All
        };
        let stacks = {
            let meta = ctx.legacy_meta()?;
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
        let ctx = Context::discover(current_dir)?;
        let details = {
            let meta = ctx.legacy_meta()?;
            let repo = ctx.clone_repo_for_merging_non_persisting()?;
            but_workspace::legacy::stack_details_v3(id, &repo, &meta)
        }?;
        debug_print(details)
    }

    pub fn branch_details(ref_name: &str, current_dir: &Path) -> anyhow::Result<()> {
        let ctx = Context::discover(current_dir)?;
        let meta = ctx.meta()?;
        let repo = ctx.clone_repo_for_merging_non_persisting()?;
        let ref_name = repo.find_reference(ref_name)?.name().to_owned();

        let details = but_workspace::branch_details(&repo, ref_name.as_ref(), &meta)?;
        debug_print(details)
    }

    pub fn branches(id: StackId, current_dir: &Path, use_json: bool) -> anyhow::Result<()> {
        let ctx = Context::discover(current_dir)?;
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
    fn create_stack_with_branch(ctx: &mut Context, name: &str, remote: bool) -> anyhow::Result<ui::StackEntry> {
        if remote {
            let repo = ctx.repo.get()?;
            let remotes = repo.remote_names();
            let remote_name = remotes
                .first()
                .map(|r| r.to_str().unwrap())
                .context("No remote found in repository")?;

            let ref_name = Refname::from_str(&format!("refs/remotes/{remote_name}/{name}"))?;
            let remote_ref_name = RemoteRefname::new(remote_name, name);
            drop(repo);

            let (stack_id, _, _) = gitbutler_branch_actions::create_virtual_branch_from_branch(
                ctx,
                &ref_name,
                Some(remote_ref_name),
                None,
            )?;

            let repo = ctx.repo.get()?;
            let meta =
                but_meta::VirtualBranchesTomlMetadata::from_path(ctx.project_data_dir().join("virtual_branches.toml"))?;
            let stack_entries = but_workspace::legacy::stacks_v3(&repo, &meta, Default::default(), None)?;
            let stack_entry = stack_entries
                .into_iter()
                .find(|entry| entry.id == Some(stack_id))
                .ok_or_else(|| anyhow::anyhow!("Failed to find newly created stack with ID: {stack_id}"))?;
            return Ok(stack_entry);
        };

        let creation_request = gitbutler_branch::BranchCreateRequest {
            name: Some(name.to_string()),
            ..Default::default()
        };
        let mut guard = ctx.exclusive_worktree_access();
        let stack_entry =
            gitbutler_branch_actions::create_virtual_branch(ctx, &creation_request, guard.write_permission())?;

        Ok(stack_entry.into())
    }

    /// Add a branch to an existing stack.
    fn add_branch_to_stack(ctx: &mut Context, stack_id: StackId, name: &str) -> anyhow::Result<ui::StackEntry> {
        let creation_request = gitbutler_branch_actions::stack::CreateSeriesRequest {
            name: name.to_string(),
            target_patch: None,
            preceding_head: None,
        };

        gitbutler_branch_actions::stack::create_branch(ctx, stack_id, creation_request)?;
        let repo = ctx.repo.get()?;
        let meta =
            but_meta::VirtualBranchesTomlMetadata::from_path(ctx.project_data_dir().join("virtual_branches.toml"))?;
        let stack_entries = but_workspace::legacy::stacks_v3(&repo, &meta, Default::default(), None)?;

        let stack_entry = stack_entries
            .into_iter()
            .find(|entry| entry.id == Some(stack_id))
            .ok_or_else(|| anyhow::anyhow!("Failed to find stack with ID: {stack_id}"))?;

        Ok(stack_entry)
    }

    /// Add a branch to an existing stack by looking up the stack by name.
    pub fn move_branch(subject_branch: &str, destination_branch: &str, current_dir: &Path) -> anyhow::Result<()> {
        let mut ctx = Context::discover(current_dir)?;
        let meta = ctx.legacy_meta()?;
        let stacks = but_workspace::legacy::stacks_v3(&*ctx.repo.get()?, &meta, Default::default(), None)?;
        let subject_stack = stacks
            .clone()
            .into_iter()
            .find(|s| s.heads.iter().any(|h| h.name == subject_branch))
            .context(format!("No stack branch found with name '{subject_branch}'"))?;

        let destination_stack = stacks
            .into_iter()
            .find(|s| s.heads.iter().any(|h| h.name == destination_branch))
            .context(format!("No stack branch found with name '{destination_branch}'"))?;

        let outcome = gitbutler_branch_actions::move_branch(
            &mut ctx,
            destination_stack.id.context("BUG(opt-destination-stack-id)")?,
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
        current_dir: &Path,
        remote: bool,
        use_json: bool,
    ) -> anyhow::Result<()> {
        let mut ctx = Context::discover(current_dir)?;
        let stack_entry = match id {
            Some(id) => add_branch_to_stack(&mut ctx, id, name)?,
            None => create_stack_with_branch(&mut ctx, name, remote)?,
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
    let repo = gix::discover(cwd)?.for_tree_diffing()?;

    let previous_path = previous_rela_path.map(path_to_rela_path).transpose()?;
    let path = path_to_rela_path(current_rela_path)?;
    let hunk_headers = indices_or_headers_to_hunk_headers(&repo, indices_or_headers, &path, previous_path.as_ref())?;
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

pub async fn watch(args: &super::Args, watch_mode: Option<&str>) -> anyhow::Result<()> {
    let ctx = Context::discover(&args.current_dir)?;
    let (tx, mut rx) = unbounded_channel();
    let start = std::time::Instant::now();
    let workdir = ctx.workdir_or_fail()?;
    let _monitor = gitbutler_filemonitor::spawn(
        ProjectId::generate(),
        &workdir,
        tx,
        watch_mode.and_then(|m| m.parse().ok()).unwrap_or_default(),
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
    let ctx = Context::discover(&args.current_dir)?;
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
    let ctx = Context::discover(&args.current_dir)?;
    debug_print(gitbutler_operating_modes::operating_mode(&ctx))
}

pub fn ref_info(args: &super::Args, ref_name: Option<&str>, expensive: bool) -> anyhow::Result<()> {
    let ctx = Context::discover(&args.current_dir)?;
    let opts = but_workspace::ref_info::Options {
        expensive_commit_info: expensive,
        traversal: Default::default(),
    };

    let _guard = ctx.shared_worktree_access();
    let meta = ctx.meta()?;
    let repo = &*ctx.repo.get()?;
    debug_print(match ref_name {
        None => but_workspace::head_info(repo, &meta, opts),
        Some(ref_name) => but_workspace::ref_info(repo.find_reference(ref_name)?, &meta, opts),
    }?)
}

pub mod graph;

pub fn remove_reference(
    args: &super::Args,
    short_name: &str,
    opts: but_workspace::branch::remove_reference::Options,
) -> anyhow::Result<()> {
    let mut ctx = Context::discover(&args.current_dir)?;
    let mut meta = ctx.meta()?;
    let (_guard, repo, mut ws, _) = ctx.workspace_mut_and_db()?;
    let ref_name = Category::LocalBranch.to_full_name(short_name)?;
    let deleted = but_workspace::branch::remove_reference(ref_name.as_ref(), &repo, &ws, &mut meta, opts)?;
    if let Some(new_ws) = deleted {
        *ws = new_ws;
        eprintln!("Deleted");
    } else {
        eprintln!("Nothing deleted");
    }
    Ok(())
}

pub fn apply(args: &super::Args, short_name: &str, order: Option<usize>) -> anyhow::Result<()> {
    let mut ctx = Context::discover(&args.current_dir)?;
    let mut meta = ctx.meta()?;
    let (_guard, repo, mut ws, _) = ctx.workspace_mut_and_db()?;
    let branch = repo.find_reference(short_name)?;
    let apply_outcome = but_workspace::branch::apply(
        branch.name(),
        &ws,
        &repo,
        &mut meta,
        but_workspace::branch::apply::Options {
            workspace_merge: WorkspaceMerge::AlwaysMerge,
            on_workspace_conflict: OnWorkspaceMergeConflict::MaterializeAndReportConflictingStacks,
            workspace_reference_naming: WorkspaceReferenceNaming::Default,
            uncommitted_changes: UncommitedWorktreeChanges::KeepAndAbortOnConflict,
            order,
            new_stack_id: None,
        },
    )?;

    let res = debug_print(&apply_outcome);
    if let Cow::Owned(new_ws) = apply_outcome.workspace {
        *ws = new_ws;
    }
    res
}

pub fn create_reference(
    args: &super::Args,
    short_name: &str,
    above: Option<&str>,
    below: Option<&str>,
) -> anyhow::Result<()> {
    let mut ctx = Context::discover(&args.current_dir)?;
    let mut meta = ctx.meta()?;
    let (_guard, repo, mut ws, _) = ctx.workspace_mut_and_db()?;
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
    let new_ws = but_workspace::branch::create_reference(
        new_ref.as_ref(),
        anchor,
        &repo,
        &ws,
        &mut meta,
        |_| StackId::generate(),
        None,
    )?;

    if let Cow::Owned(new_ws) = new_ws {
        *ws = new_ws;
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
            let Some(UnifiedPatch::Patch { hunks, .. }) = worktree_changes.unified_patch(repo, UI_CONTEXT_LINES)?
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
        gix::path::to_unix_separators_on_windows(gix::path::os_str_into_bstr(path.as_os_str())?).into_owned();
    Ok(rela_path)
}

pub fn branch_list(current_dir: &Path) -> anyhow::Result<()> {
    let ctx = Context::discover(current_dir)?;
    debug_print(gitbutler_branch_actions::list_branches(
        &ctx,
        Some(BranchListingFilter {
            local: None,
            applied: None,
        }),
        None,
    )?)
}
