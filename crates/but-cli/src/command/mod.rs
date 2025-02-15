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
    let repo = gix::discover(&args.current_dir)?;
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

mod commit {
    use crate::command::debug_print;
    use anyhow::bail;
    use but_core::TreeChange;
    use but_workspace::commit_engine::DiffSpec;
    use gitbutler_oxidize::OidExt;
    use gitbutler_project::Project;
    use gitbutler_stack::{VirtualBranchesHandle, VirtualBranchesState};
    use gix::prelude::ObjectIdExt;
    use gix::revision::walk::Sorting;

    pub fn commit(
        repo: gix::Repository,
        project: Option<Project>,
        message: Option<&str>,
        amend: bool,
        parent_revspec: Option<&str>,
    ) -> anyhow::Result<()> {
        if message.is_none() && !amend {
            bail!("Need a message when creating a new commit");
        }
        let mut parent_id = parent_revspec
            .map(|revspec| repo.rev_parse_single(revspec).map_err(anyhow::Error::from))
            .unwrap_or_else(|| Ok(repo.head_id()?))?
            .detach();
        #[allow(unused_assignments)]
        let mut vbs = None;
        let mut frame = None;
        let mut project = if let Some(project) = project {
            let guard = project.exclusive_worktree_access();
            vbs = Some(VirtualBranchesHandle::new(project.gb_dir()).read_file()?);
            let reference_frame =
                project_to_reference_frame(&repo, vbs.as_mut().unwrap(), parent_id)?;
            // This might be the default set earlier, but we never want to push on top of the workspace commit.
            if repo.head_id().ok().map(|id| id.detach()) == Some(parent_id) {
                parent_id = reference_frame
                    .branch_tip
                    .expect("set as we need the parent to be part of a stack");
            }
            frame = Some(reference_frame);
            Some((project, guard))
        } else {
            None
        };
        debug_print(
            but_workspace::commit_engine::create_commit_and_update_refs_with_project(
                &repo,
                project
                    .as_mut()
                    .zip(frame)
                    .map(|((_project, guard), frame)| (frame, guard.write_permission())),
                if amend {
                    but_workspace::commit_engine::Destination::AmendCommit(parent_id)
                } else {
                    but_workspace::commit_engine::Destination::NewCommit {
                        parent_commit_id: Some(parent_id),
                        message: message.unwrap_or_default().to_owned(),
                    }
                },
                None,
                to_whole_file_diffspec(but_core::diff::worktree_changes(&repo)?.changes),
                0, /* context-lines */
            )?,
        )?;

        if let Some((vbs, (project, _guard))) = vbs.zip(project) {
            VirtualBranchesHandle::new(project.gb_dir()).write_file(&vbs)?;
        }
        Ok(())
    }

    /// Find the tip of the stack that will contain the `parent_id`, and the workspace merge commit as well.
    fn project_to_reference_frame<'a>(
        repo: &gix::Repository,
        vb: &'a mut VirtualBranchesState,
        parent_id: gix::ObjectId,
    ) -> anyhow::Result<but_workspace::commit_engine::ReferenceFrame<'a>> {
        let head_id = repo.head_id()?;
        let workspace_commit = head_id.object()?.into_commit().decode()?.to_owned();
        if workspace_commit.parents.len() < 2 {
            return Ok(but_workspace::commit_engine::ReferenceFrame {
                workspace_tip: Some(head_id.detach()),
                // The workspace commit is never the tip
                branch_tip: Some(workspace_commit.parents[0]),
                vb,
            });
        }

        let merge_base = repo.merge_base_octopus(workspace_commit.parents)?;
        for stack in vb.branches.values() {
            let stack_tip = stack.head.to_gix();
            if stack_tip
                .attach(repo)
                .ancestors()
                .with_boundary(Some(merge_base))
                .sorting(Sorting::BreadthFirst)
                .all()?
                .filter_map(Result::ok)
                .any(|info| info.id == parent_id)
            {
                return Ok(but_workspace::commit_engine::ReferenceFrame {
                    workspace_tip: Some(head_id.detach()),
                    branch_tip: Some(stack_tip),
                    vb,
                });
            }
        }
        bail!("Could not find stack that includes parent-id at {parent_id}")
    }

    fn to_whole_file_diffspec(changes: Vec<TreeChange>) -> Vec<DiffSpec> {
        changes
            .into_iter()
            .map(|change| DiffSpec {
                previous_path: change.previous_path().map(ToOwned::to_owned),
                path: change.path,
                hunk_headers: Vec::new(),
            })
            .collect()
    }
}
pub use commit::commit;

pub mod diff {
    use crate::command::{debug_print, project_from_path, project_repo};
    use gix::bstr::BString;
    use itertools::Itertools;
    use std::path::Path;

    pub fn commit_changes(
        current_dir: &Path,
        current_commit: &str,
        previous_commit: Option<&str>,
        unified_diff: bool,
    ) -> anyhow::Result<()> {
        let repo = project_repo(current_dir)?;
        let previous_commit = previous_commit
            .map(|revspec| repo.rev_parse_single(revspec))
            .transpose()?;
        let commit = repo.rev_parse_single(current_commit)?;
        let changes =
            but_core::diff::commit_changes(&repo, previous_commit.map(Into::into), commit.into())?;

        if unified_diff {
            debug_print(unified_diff_for_changes(&repo, changes)?)
        } else {
            debug_print(changes)
        }
    }

    pub fn status(current_dir: &Path, unified_diff: bool) -> anyhow::Result<()> {
        let repo = project_repo(current_dir)?;
        let worktree = but_core::diff::worktree_changes(&repo)?;
        if unified_diff {
            debug_print((
                unified_diff_for_changes(&repo, worktree.changes)?,
                worktree.ignored_changes,
            ))
        } else {
            debug_print(worktree)
        }
    }

    pub fn locks(current_dir: &Path) -> anyhow::Result<()> {
        let project = project_from_path(current_dir)?;
        let repo = gix::open(project.worktree_path())?;
        let worktree_changes = but_core::diff::worktree_changes(&repo)?;
        let input_stacks = but_hunk_dependency::workspace_stacks_to_input_stacks(
            &repo,
            &but_workspace::stacks(&project.gb_dir())?,
            but_workspace::common_merge_base_with_target_branch(&project.gb_dir())?,
        )?;
        let ranges = but_hunk_dependency::WorkspaceRanges::try_from_stacks(input_stacks)?;
        debug_print(intersect_workspace_ranges(
            &repo,
            ranges,
            worktree_changes.changes,
        )?)
    }

    fn unified_diff_for_changes(
        repo: &gix::Repository,
        changes: Vec<but_core::TreeChange>,
    ) -> anyhow::Result<Vec<(but_core::TreeChange, but_core::UnifiedDiff)>> {
        changes
            .into_iter()
            .map(|tree_change| {
                tree_change
                    .unified_diff(repo, 3)
                    .map(|diff| (tree_change, diff))
            })
            .collect::<Result<Vec<_>, _>>()
    }

    fn intersect_workspace_ranges(
        repo: &gix::Repository,
        ranges: but_hunk_dependency::WorkspaceRanges,
        worktree_changes: Vec<but_core::TreeChange>,
    ) -> anyhow::Result<LockInfo> {
        let mut intersections_by_path = Vec::new();
        let mut missed_hunks = Vec::new();
        for change in worktree_changes {
            let unidiff = change.unified_diff(repo, 0)?;
            let but_core::UnifiedDiff::Patch { hunks } = unidiff else {
                continue;
            };
            let mut intersections = Vec::new();
            for hunk in hunks {
                if let Some(hunk_ranges) =
                    ranges.intersection(&change.path, hunk.old_start, hunk.old_lines)
                {
                    intersections.push(HunkIntersection {
                        hunk,
                        commit_intersections: hunk_ranges.into_iter().copied().collect(),
                    });
                } else {
                    missed_hunks.push((change.path.clone(), hunk));
                }
            }
            if !intersections.is_empty() {
                intersections_by_path.push((change.path, intersections));
            }
        }

        Ok(LockInfo {
            intersections_by_path,
            missed_hunks,
            ranges_by_path: ranges
                .ranges_by_path_map()
                .iter()
                .sorted_by(|a, b| a.0.cmp(b.0))
                .map(|(path, ranges)| (path.to_owned(), ranges.to_vec()))
                .collect(),
        })
    }

    /// A structure that has stable content so it can be asserted on, showing the hunk-ranges that intersect with each of the input ranges.
    #[derive(Debug)]
    #[allow(dead_code)]
    pub struct LockInfo {
        /// All available ranges for a tracked path, basically all changes seen over a set of commits.
        pub ranges_by_path: Vec<(BString, Vec<but_hunk_dependency::HunkRange>)>,
        /// The ranges that intersected with an input hunk.
        pub intersections_by_path: Vec<(BString, Vec<HunkIntersection>)>,
        /// Hunks that didn't have a matching intersection, with the filepath mentioned per hunk as well.
        pub missed_hunks: Vec<(BString, but_core::unified_diff::DiffHunk)>,
    }

    #[derive(Debug)]
    #[allow(dead_code)]
    pub struct HunkIntersection {
        /// The hunk that was used for the intersection.
        pub hunk: but_core::unified_diff::DiffHunk,
        /// The hunks that touch `hunk` in the commit-diffs.
        pub commit_intersections: Vec<but_hunk_dependency::HunkRange>,
    }
}

pub mod stacks {
    use std::path::Path;

    use but_workspace::stack_branches;
    use gitbutler_command_context::CommandContext;
    use gitbutler_settings::AppSettings;

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
