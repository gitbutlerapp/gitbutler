//! `but _open --gui`: open what one ID names in the GitButler GUI rather than an editor, by the
//! `but://open` link that names it. IDs resolve as they do for the editor, and a hunk opens at the
//! line the editor would. A place with no CLI ID, as a project's graph, is given as the link's own
//! parameters, which the GUI reads, and an ID can take more of them.

use but_core::{ChangeId, SingleHunk, ref_metadata::StackId};
use but_ctx::Context;
use but_path::{
    AppChannel,
    link::{Changes, FileAt, OpenLink},
};
use itertools::{Either, Itertools as _};

use super::{Openable, compute_line_number_to_open_at};
use crate::{
    CliResult, IdMap,
    args::atoms::{CliIdArg, Purpose, ResolvedCliIdArg, anonymous_segment_error},
    bad_input,
    id::SegmentWithId,
    theme::Theme,
    utils::{
        CliOutput, CliOutputHuman, WriteWithUtils,
        change_source::{ChangeSourceId, InvokedFrom},
    },
};

/// Open what one ID names in the GitButler GUI, or with `dry_run`, return its link.
pub(crate) fn open(
    ctx: &Context,
    sources: Vec<CliIdArg>,
    dry_run: bool,
    invoked_from: &InvokedFrom,
) -> CliResult<Option<GuiOutcome>> {
    let link = resolve(ctx, sources, invoked_from)?;
    Ok(run(link, dry_run)?)
}

fn resolve(
    ctx: &Context,
    sources: Vec<CliIdArg>,
    invoked_from: &InvokedFrom,
) -> CliResult<OpenLink> {
    let guard = ctx.shared_worktree_access();
    let id_map = IdMap::new_from_context(ctx, guard.read_permission(), Some(invoked_from))?;
    let project = ctx.workdir_or_fail()?;
    let (repo, _ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;

    let (params, ids) = split_sources(&sources);
    let at = match ids.as_slice() {
        [] => params,
        [id] => {
            let (changes, file) = resolve_changes(&repo, &id_map, id)?;
            with_params(changes.at(file.as_ref()), params)?
        }
        _ => return Err(bad_input("The GUI opens one place at a time").into()),
    };
    Ok(OpenLink {
        path: project,
        new_window: false,
        at,
    })
}

fn split_sources(sources: &[CliIdArg]) -> (Vec<(String, String)>, Vec<&CliIdArg>) {
    sources
        .iter()
        .partition_map(|source| match source.0.split_once('=') {
            Some((key, value)) => Either::Left((key.to_owned(), value.to_owned())),
            None => Either::Right(source),
        })
}

/// `place`, as an ID names it, followed by `params`, none of which may say what the ID does.
fn with_params(
    mut place: Vec<(String, String)>,
    params: Vec<(String, String)>,
) -> CliResult<Vec<(String, String)>> {
    if let Some((key, _)) = params
        .iter()
        .find(|(key, _)| place.iter().any(|(named, _)| named == key))
    {
        return Err(bad_input(format!("The ID already says `{key}=`")).into());
    }
    place.extend(params);
    Ok(place)
}

/// IDs last one read of the workspace, so the link names refs, hashes and paths.
fn resolve_changes(
    repo: &gix::Repository,
    id_map: &IdMap,
    source: &CliIdArg,
) -> CliResult<(Changes, Option<FileAt>)> {
    let uncommitted = |source: &ChangeSourceId| Changes::Uncommitted {
        worktree: source.worktree_name().map(ToString::to_string),
    };
    let resolved = match source.resolve_in_workspace(repo, id_map, Purpose::Target, None)? {
        ResolvedCliIdArg::Branch(branch) => {
            let name = branch.resolve_local_branch_name()?.to_string();
            (Changes::Branch(name), None)
        }
        ResolvedCliIdArg::Stack { stack_id, .. } => (stack_changes(id_map, stack_id)?, None),
        ResolvedCliIdArg::Commit(commit) => (
            commit_changes(id_map, commit.commit_id, commit.change_id.as_ref()),
            None,
        ),
        ResolvedCliIdArg::CommittedFile(committed_file) => (
            commit_changes(
                id_map,
                committed_file.commit_id,
                committed_file.change_id.as_ref(),
            ),
            Some(FileAt {
                path: committed_file.path.to_string(),
                line: None,
            }),
        ),
        ResolvedCliIdArg::CommittedHunk(committed_hunk) => (
            commit_changes(
                id_map,
                committed_hunk.committed_file.commit_id,
                committed_hunk.committed_file.change_id.as_ref(),
            ),
            Some(file_at(repo, &committed_hunk.hunk, false)?),
        ),
        ResolvedCliIdArg::UncommittedHunkOrFile(changes) => (
            uncommitted(&changes.source),
            Some(file_at(
                repo,
                &changes.hunks.first().hunk,
                changes.is_entire_file,
            )?),
        ),
        ResolvedCliIdArg::Uncommitted(source) => (uncommitted(&source), None),
        ResolvedCliIdArg::AnonymousSegment(segment) => {
            return Err(anonymous_segment_error(&segment.id));
        }
        resolved_id @ ResolvedCliIdArg::PathPrefix { .. } => {
            return Err(bad_input(format!(
                "Expected a branch, stack, commit, file or hunk, got {}",
                resolved_id.kind_for_humans()
            ))
            .into());
        }
    };
    Ok(resolved)
}

/// The change ID finds a commit once it's amended or rebased.
fn commit_changes(id_map: &IdMap, id: gix::ObjectId, change: Option<&ChangeId>) -> Changes {
    match branch_of_remote_commit(id_map, id) {
        Some(branch) => Changes::Upstream {
            branch,
            commit: id.to_string(),
        },
        None => Changes::Commit {
            id: id.to_string(),
            change: change.map(ToString::to_string),
        },
    }
}

/// A stack by its bottom branch, as the app names one, or alone in it, that branch, as a stack of
/// one has no page.
fn stack_changes(id_map: &IdMap, stack_id: StackId) -> CliResult<Changes> {
    let segments = id_map
        .stack(stack_id)
        .map(|lane| lane.segments.as_slice())
        .unwrap_or_default();
    let [.., bottom] = segments else {
        return Err(bad_input("That stack has no branch to open").into());
    };
    let Some(name) = full_ref_name(bottom) else {
        return Err(anonymous_segment_error(&bottom.short_id));
    };
    Ok(match segments {
        [_] => Changes::Branch(name),
        _ => Changes::Stack(name),
    })
}

fn branch_of_remote_commit(id_map: &IdMap, id: gix::ObjectId) -> Option<String> {
    let segment = id_map
        .stacks()
        .iter()
        .chain(id_map.worktree_lanes())
        .flat_map(|lane| &lane.segments)
        .find(|segment| {
            segment
                .remote_commits
                .iter()
                .any(|commit| commit.commit_id() == id)
        })?;
    full_ref_name(segment)
}

fn full_ref_name(segment: &SegmentWithId) -> Option<String> {
    Some(segment.inner.ref_info.as_ref()?.ref_name.to_string())
}

/// At the line `_open` opens an editor at.
fn file_at(repo: &gix::Repository, hunk: &SingleHunk, whole: bool) -> anyhow::Result<FileAt> {
    let line = match Openable::try_from_hunk(repo, hunk, whole)? {
        Openable::Hunk(hunk) => Some(compute_line_number_to_open_at(&hunk)),
        Openable::File(_) | Openable::Files(_) => None,
    };
    Ok(FileAt {
        path: hunk.path.to_string(),
        line,
    })
}

fn run(link: OpenLink, dry_run: bool) -> anyhow::Result<Option<GuiOutcome>> {
    let channel = AppChannel::new();
    if dry_run {
        return Ok(Some(GuiOutcome(channel.link_url(&link)?.into())));
    }
    channel.open(&link)?;
    Ok(None)
}

pub(crate) struct GuiOutcome(String);

impl CliOutputHuman for GuiOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        writeln!(out, "{}", self.0)?;
        Ok(())
    }
}

impl CliOutput for GuiOutcome {
    fn on_json(self) -> impl serde::Serialize {
        #[derive(serde::Serialize)]
        struct Output {
            link: String,
        }

        Output { link: self.0 }
    }
}
