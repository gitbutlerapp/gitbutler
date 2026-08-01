use std::path::{Path, PathBuf};

use anyhow::Context as _;
use bstr::BStr;
use but_api::open::{
    list_program_specs, list_program_specs_for_file,
    program::{OpenSpec, ProgramSpec, open_in_program_unchecked},
};
use but_ctx::Context;
use gix::utils::AsBStr;
use itertools::Itertools as _;
use nonempty::NonEmpty;

use crate::{
    CliError, CliResult, IdMap,
    args::atoms::{CliIdArg, Purpose, ResolvedCliIdArg},
    bad_input,
    id::UncommittedHunkOrFile,
    utils::IntermediateChannel,
};

#[derive(Debug)]
pub(crate) struct Hunk {
    /// The line numbers that were added in this hunk.
    pub line_nums_added: Vec<u32>,
    /// The line numbers that were removed in this hunk.
    pub line_nums_removed: Vec<u32>,
    /// The start position of the hunk in the old version.
    pub old_start: u32,
    /// The start position of the hunk in the new version.
    pub new_start: u32,
    /// Path to the file containing the hunk.
    pub path: PathBuf,
}

#[derive(Debug)]
pub(crate) enum Openable {
    File(PathBuf),
    Files(NonEmpty<PathBuf>),
    Hunk(Hunk),
}

impl Openable {
    /// Try to create an [`Openable`] from an [`UncommittedHunkOrFile`].
    pub fn try_from_uncommitted(
        repo: &gix::Repository,
        uncommitted: &UncommittedHunkOrFile,
    ) -> anyhow::Result<Self> {
        let hunk = &uncommitted.hunks.first().hunk;

        let path = repo
            .workdir_path(hunk.path.as_bstr())
            .ok_or_else(|| anyhow::anyhow!("Failed to resolve path to hunk"))?;

        let openable = match (
            uncommitted.is_entire_file,
            &hunk.hunk_header,
            hunk.line_nums(),
        ) {
            (false, Some(hunk_header), Some((line_nums_added, line_nums_removed))) => {
                Openable::Hunk(Hunk {
                    // Truncate line numbers - the probability of exceeding a u32 is infinitesimally small.
                    line_nums_added: line_nums_added
                        .iter()
                        .map(|n| (*n).min(u32::MAX as usize) as u32)
                        .collect(),
                    line_nums_removed: line_nums_removed
                        .iter()
                        .map(|n| (*n).min(u32::MAX as usize) as u32)
                        .collect(),
                    old_start: hunk_header.old_start,
                    new_start: hunk_header.new_start,
                    path,
                })
            }
            _ => Openable::File(path),
        };

        Ok(openable)
    }

    /// Try to create an [`Openable`] from a repository-relative path.
    ///
    /// Does NOT validate the path exists in the repository.
    pub fn try_from_relpath(repo: &gix::Repository, relpath: &BStr) -> anyhow::Result<Self> {
        repo.workdir_path(relpath)
            .map(Openable::File)
            .context("Failed to resolve path")
    }

    /// Try to create an [`Openable`] from a list of repository-relative path.
    ///
    /// Does NOT validate the paths exist in the repository.
    pub fn try_from_relpaths(
        repo: &gix::Repository,
        relpaths: NonEmpty<&BStr>,
    ) -> anyhow::Result<Self> {
        let repo_paths =
            relpaths.try_map(|path| repo.workdir_path(path).context("Failed to resolve path"))?;
        Ok(Openable::Files(repo_paths))
    }
}

pub(crate) fn open(
    ctx: &Context,
    mut out: IntermediateChannel<'_>,
    sources: Vec<CliIdArg>,
    program_id: Option<String>,
) -> CliResult<()> {
    let guard = ctx.shared_worktree_access();
    let id_map = IdMap::new_from_context(ctx, guard.read_permission())?;
    let (repo, _ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;

    let to_open = match sources.as_slice() {
        [] => return Err(bad_input("At least one source is required").into()),
        [source] => resolve_source(&repo, &id_map, source)?,
        [first_source, tail @ ..] => {
            let mut paths = NonEmpty::new(resolve_file_source(&repo, &id_map, first_source)?);
            for source in tail {
                paths.push(resolve_file_source(&repo, &id_map, source)?);
            }
            Openable::Files(paths)
        }
    };

    let program = match program_id {
        Some(program_id) => {
            let program_specs = list_program_specs();
            program_specs
                .into_iter()
                .find(|ps| ps.id == program_id)
                .ok_or_else(|| {
                    CliError::from(
                        bad_input("No such program found")
                            .arg_name("--program-id")
                            .arg_value(program_id),
                    )
                })?
        }
        None => {
            let program_specs = list_program_specs_for_openable(&to_open);
            match TryInto::<[ProgramSpec; 1]>::try_into(program_specs) {
                Ok([program_spec]) => program_spec,
                Err(mut programs) => {
                    if !programs.is_empty() {
                        if let Some(mut input) = out.prepare_for_terminal_input() {
                            let options = NonEmpty::from_vec(
                                programs
                                    .iter()
                                    .enumerate()
                                    .map(|(idx, program)| (&program.id, idx))
                                    .collect::<Vec<_>>(),
                            )
                            .expect("programs was just checked to be non-empty");

                            let Some(selection) = input.prompt_select(
                                "Could not automatically choose program. Choose one to open with",
                                &options,
                            )?.copied()
                            else {
                                return Err(bad_input("No program picked").into());
                            };

                            programs.swap_remove(selection)
                        } else {
                            let program_ids =
                                programs.into_iter().map(|program| program.id).join(", ");
                            return Err(bad_input(format!(
                                "Could not automatically choose program. Found {program_ids}"
                            ))
                            .hint("Specify a program with `--program-id`")
                            .into());
                        }
                    } else {
                        return Err(bad_input("Could not automatically choose program")
                            .hint("Specify a program with `--program-id`")
                            .into());
                    }
                }
            }
        }
    };

    run(&program, to_open)?;

    Ok(())
}

pub(crate) fn list_program_specs_for_openable(openable: &Openable) -> Vec<ProgramSpec> {
    let path: &Path = match openable {
        Openable::File(path) => path,
        Openable::Files(paths) => paths.first(),
        Openable::Hunk(hunk) => &hunk.path,
    };

    list_program_specs_for_file(path)
}

fn resolve_source(
    repo: &gix::Repository,
    id_map: &IdMap,
    source: &CliIdArg,
) -> CliResult<Openable> {
    match source.resolve_in_workspace(repo, id_map, Purpose::Uncommitted, None)? {
        ResolvedCliIdArg::UncommittedHunkOrFile(uncommitted) => {
            Ok(Openable::try_from_uncommitted(repo, &uncommitted)?)
        }
        resolved_id => Err(unexpected_source_kind(resolved_id)),
    }
}

fn resolve_file_source(
    repo: &gix::Repository,
    id_map: &IdMap,
    source: &CliIdArg,
) -> CliResult<PathBuf> {
    match source.resolve_in_workspace(repo, id_map, Purpose::Uncommitted, None)? {
        ResolvedCliIdArg::UncommittedHunkOrFile(uncommitted) => {
            if !uncommitted.is_entire_file {
                return Err(bad_input(format!(
                    "Only entire files can be opened when multiple sources are provided; \
                     '{source}' is a hunk"
                ))
                .into());
            }

            match Openable::try_from_uncommitted(repo, &uncommitted)? {
                Openable::File(path) => Ok(path),
                Openable::Hunk(_) | Openable::Files(_) => {
                    unreachable!("entire-file source must resolve to one file")
                }
            }
        }
        resolved_id => Err(unexpected_source_kind(resolved_id)),
    }
}

fn unexpected_source_kind(resolved_id: ResolvedCliIdArg) -> CliError {
    bad_input(format!(
        "Expected uncommitted file or hunk, got {}",
        resolved_id.kind_for_humans()
    ))
    .into()
}

pub(crate) fn run(program: &ProgramSpec, to_open: Openable) -> anyhow::Result<()> {
    let open_spec = match to_open {
        Openable::Hunk(hunk) => {
            let line_number = compute_line_number_to_open_at(&hunk);
            OpenSpec::FileAtLine(hunk.path, line_number)
        }
        Openable::File(path) => OpenSpec::File(path),
        Openable::Files(paths) => OpenSpec::Files(paths),
    };

    open_in_program_unchecked(program, open_spec)
}

/// Compute the line to place the cursor at, going through a priority order of additions ->
/// deletions -> hunk header start, falling through to the next thing if the prior one is absent.
fn compute_line_number_to_open_at(hunk: &Hunk) -> u32 {
    match (
        hunk.line_nums_added.as_slice(),
        hunk.line_nums_removed.as_slice(),
    ) {
        ([first_added, ..], _) => *first_added,
        (_, [first_removed, ..]) => {
            let leading_context = first_removed.saturating_sub(hunk.old_start);
            (hunk.new_start + leading_context).saturating_sub(1).max(1)
        }
        _ => hunk.new_start,
    }
}
