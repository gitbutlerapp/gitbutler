//! `but _open --gui`: open what one ID names in the GitButler GUI rather than an editor, by the
//! `but://open` link that names it. IDs resolve as they do for the editor, and a hunk opens at the
//! line the editor would. A place with no CLI ID, as a project's graph, is given as the link's own
//! parameters, which the GUI reads.

use but_core::SingleHunk;
use but_ctx::Context;
use but_path::{
    AppChannel,
    link::{Changes, FileAt, OpenLink},
};

use super::{Openable, compute_line_number_to_open_at};
use crate::{
    CliResult, IdMap,
    args::atoms::{CliIdArg, Purpose, ResolvedCliIdArg},
    bad_input,
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

    let at = match link_params(&sources)? {
        Some(params) => params,
        None => {
            let [source] = sources.as_slice() else {
                return Err(bad_input("The GUI opens one place at a time").into());
            };
            let (changes, file) = resolve_changes(&repo, &id_map, source)?;
            changes.at(file.as_ref())
        }
    };
    Ok(OpenLink {
        path: project,
        new_window: false,
        at,
    })
}

/// `sources` as a link's own parameters, in their order, when they're written `key=value`, or
/// `None` when they're CLI IDs.
fn link_params(sources: &[CliIdArg]) -> CliResult<Option<Vec<(String, String)>>> {
    if !sources.iter().any(|source| source.0.contains('=')) {
        return Ok(None);
    }
    let pair = |source: &CliIdArg| {
        let (key, value) = source.0.split_once('=')?;
        Some((key.to_owned(), value.to_owned()))
    };
    (sources.iter())
        .map(|source| {
            pair(source).ok_or_else(|| {
                bad_input("Give the GUI one CLI ID or a link's parameters, not both").into()
            })
        })
        .collect::<CliResult<_>>()
        .map(Some)
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
        ResolvedCliIdArg::Commit(commit) => (Changes::Commit(commit.commit_id.to_string()), None),
        ResolvedCliIdArg::CommittedFile(committed_file) => (
            Changes::Commit(committed_file.commit_id.to_string()),
            Some(FileAt {
                path: committed_file.path.to_string(),
                line: None,
            }),
        ),
        ResolvedCliIdArg::CommittedHunk(committed_hunk) => (
            Changes::Commit(committed_hunk.committed_file.commit_id.to_string()),
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
            return Err(crate::args::atoms::anonymous_segment_error(&segment.id));
        }
        resolved_id @ (ResolvedCliIdArg::PathPrefix { .. } | ResolvedCliIdArg::Stack { .. }) => {
            return Err(bad_input(format!(
                "Expected a branch, commit, file or hunk, got {}",
                resolved_id.kind_for_humans()
            ))
            .into());
        }
    };
    Ok(resolved)
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
