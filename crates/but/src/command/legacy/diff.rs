use std::path::Path;

use bstr::BString;
use but_core::{TreeStatusKind, diff::LineStats};
use but_ctx::Context;
use gix::refs::FullName;
use itertools::Itertools as _;
use nonempty::NonEmpty;
use ratatui::text::Line;
use serde::Serialize;

use crate::{
    CliResult, IdMap,
    args::{
        atoms::{CliIdArg, Purpose, ResolvedCliIdArg},
        diff::Platform,
    },
    bad_input,
    id::{CommitId, CommittedFileId, CommittedHunk, IdAndHunk, UncommittedHunkOrFile},
    theme::{Paint as _, Theme},
    utils::{
        CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils,
        change_source::{ChangeSourceId, ChangeSourceRepo, InvokedFrom, conflicted_worktree_paths},
        diff_rendering::{
            self, DetailsLine, DiffLineWriter, IdGen, WithSyntaxHighlighting, load_syntax_set,
        },
        string_interning::Strings,
    },
};

const CLEAR_TO_END_OF_LINE: &str = "\x1b[0K";

#[derive(Debug)]
pub struct DiffOutcome<'a> {
    ctx: &'a mut Context,
    targets: Vec<DiffOperation>,
    stat: bool,
}

impl CliOutputHuman for DiffOutcome<'_> {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        theme: &'static Theme,
    ) -> anyhow::Result<()> {
        let Self { ctx, targets, stat } = self;

        if stat {
            return write_stat(ctx, &targets, theme, &mut DiffWriter { out, theme });
        }

        let syntax_set = load_syntax_set();
        let syntax_theme = theme.load_syntax_highlighting_theme()?;

        let strings = Strings::default();
        let writer = DiffWriter { out, theme };
        let mut writer =
            WithSyntaxHighlighting::new(writer, strings.clone(), &syntax_set, &syntax_theme);
        let mut id_gen = IdGen::new(strings);

        let options = diff_rendering::Options {
            skip_commit_header: true,
            skip_line_stats: true,
        };

        for (index, target) in targets.into_iter().enumerate() {
            if index > 0 {
                writer.write_section_separator()?;
            }
            match target {
                DiffOperation::Uncommitted(source) => {
                    diff_rendering::render_uncommitted_source(
                        ctx,
                        source,
                        theme,
                        &mut id_gen,
                        options,
                        &mut writer,
                    )?;
                }
                DiffOperation::Commit { commit } => {
                    diff_rendering::render_commit(
                        commit.commit_id,
                        commit.change_id,
                        ctx,
                        theme,
                        &mut id_gen,
                        options,
                        &mut writer,
                    )?;
                }
                DiffOperation::Branch { branch } => {
                    let branch = branch.shorten().to_string();
                    diff_rendering::render_branch(
                        branch,
                        ctx,
                        theme,
                        &mut id_gen,
                        options,
                        &mut writer,
                    )?;
                }
                DiffOperation::UncommittedHunkOrFile { hunk } => {
                    diff_rendering::render_uncommitted_hunk(
                        *hunk,
                        theme,
                        &mut id_gen,
                        options,
                        &mut writer,
                    )?;
                }
                DiffOperation::CommittedFile { commit, path } => {
                    diff_rendering::render_committed_file(
                        commit.commit_id,
                        path,
                        ctx,
                        theme,
                        &mut id_gen,
                        options,
                        &mut writer,
                    )?;
                }
                DiffOperation::CommittedHunk { hunk } => {
                    diff_rendering::render_committed_hunk(*hunk, theme, &mut id_gen, &mut writer)?;
                }
                DiffOperation::PathPrefix { id, hunks } => {
                    diff_rendering::render_path_prefix(
                        &id,
                        hunks,
                        ctx,
                        theme,
                        &mut id_gen,
                        options,
                        &mut writer,
                    )?;
                }
                DiffOperation::CleanPath { path } => {
                    writer.write_non_selectable_text(clean_path_line(&path, theme))?;
                }
            }
        }

        Ok(())
    }
}

impl CliOutput for DiffOutcome<'_> {
    fn on_json(self) -> impl Serialize {
        struct DeferredOutput<'a> {
            ctx: &'a Context,
            targets: Vec<DiffOperation>,
            stat: bool,
        }

        #[derive(Serialize)]
        struct StatOutput {
            files: Vec<FileStat>,
        }

        impl Serialize for DeferredOutput<'_> {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                use serde::ser::Error as _;

                if self.stat {
                    let files = file_stats(self.ctx, &self.targets).map_err(S::Error::custom)?;
                    StatOutput { files }.serialize(serializer)
                } else {
                    let output = build_output(self.ctx, &self.targets).map_err(S::Error::custom)?;
                    output.serialize(serializer)
                }
            }
        }

        let Self { ctx, targets, stat } = self;
        DeferredOutput { ctx, targets, stat }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Output {
    changes: Vec<Change>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Change {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    path: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    old_path: Option<String>,
    diff: Diff,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Diff {
    Binary,
    TooLarge {
        size_in_bytes: u64,
    },
    Patch {
        hunks: Vec<Hunk>,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        is_binary_to_text: bool,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Hunk {
    old_start: u32,
    old_lines: u32,
    new_start: u32,
    new_lines: u32,
    diff: String,
}

fn hunk_to_json_hunk(hunk: &but_core::unified_diff::DiffHunk) -> Hunk {
    use bstr::ByteSlice as _;

    Hunk {
        old_start: hunk.old_start,
        old_lines: hunk.old_lines,
        new_start: hunk.new_start,
        new_lines: hunk.new_lines,
        diff: hunk.diff.to_str_lossy().into_owned(),
    }
}

fn hunk_to_change(id: Option<&str>, hunk: &but_core::SingleHunk, status: TreeStatusKind) -> Change {
    use bstr::ByteSlice as _;

    let diff = if let (Some(diff), Some(header)) = (&hunk.diff, &hunk.hunk_header) {
        Diff::Patch {
            hunks: vec![hunk_to_json_hunk(&but_core::unified_diff::DiffHunk {
                old_start: header.old_start,
                old_lines: header.old_lines,
                new_start: header.new_start,
                new_lines: header.new_lines,
                diff: diff.clone(),
            })],
            is_binary_to_text: false,
        }
    } else {
        Diff::Patch {
            hunks: vec![],
            is_binary_to_text: false,
        }
    };

    Change {
        id: id.map(str::to_owned),
        path: hunk.path.to_str_lossy().into_owned(),
        status: match status {
            TreeStatusKind::Addition => "added",
            TreeStatusKind::Deletion => "deleted",
            TreeStatusKind::Modification => "modified",
            TreeStatusKind::Rename => "renamed",
        }
        .to_owned(),
        old_path: None,
        diff,
    }
}

fn hunk_changes(mut hunks: Vec<(&str, &but_core::SingleHunk, TreeStatusKind)>) -> Vec<Change> {
    hunks.sort_by(|(_, a_hunk, _), (_, b_hunk, _)| {
        a_hunk
            .path
            .cmp(&b_hunk.path)
            .then_with(|| a_hunk.hunk_header.cmp(&b_hunk.hunk_header))
    });
    hunks
        .into_iter()
        .map(|(id, hunk, status)| hunk_to_change(Some(id), hunk, status))
        .collect()
}

fn tree_change_to_change(ctx: &Context, change: but_core::ui::TreeChange) -> Change {
    use but_core::{UnifiedPatch, ui::TreeStatus};

    let (status, old_path) = match &change.status {
        TreeStatus::Addition { .. } => ("added", None),
        TreeStatus::Deletion { .. } => ("deleted", None),
        TreeStatus::Modification { .. } => ("modified", None),
        TreeStatus::Rename { previous_path, .. } => ("renamed", Some(previous_path.to_string())),
    };

    let patch = but_api::diff::tree_change_diffs(ctx, change.clone())
        .ok()
        .flatten();
    let diff = match patch {
        Some(UnifiedPatch::Binary) => Diff::Binary,
        Some(UnifiedPatch::TooLarge { size_in_bytes }) => Diff::TooLarge { size_in_bytes },
        Some(UnifiedPatch::Patch {
            hunks,
            is_result_of_binary_to_text_conversion,
            ..
        }) => Diff::Patch {
            hunks: hunks.iter().map(hunk_to_json_hunk).collect(),
            is_binary_to_text: is_result_of_binary_to_text_conversion,
        },
        None => Diff::Patch {
            hunks: vec![],
            is_binary_to_text: false,
        },
    };

    Change {
        id: None,
        path: change.path_bytes.to_string(),
        status: status.to_owned(),
        old_path,
        diff,
    }
}

fn commit_changes(
    ctx: &Context,
    commit: gix::ObjectId,
    path: Option<&BString>,
) -> anyhow::Result<Vec<Change>> {
    let details = but_api::diff::commit_details(ctx, commit, but_api::diff::ComputeLineStats::No)?;
    Ok(details
        .diff_with_first_parent
        .into_iter()
        .filter(|change| path.is_none_or(|path| path == &change.path))
        .map(|change| tree_change_to_change(ctx, change.into()))
        .collect())
}

fn build_output(ctx: &Context, targets: &[DiffOperation]) -> anyhow::Result<Output> {
    let mut changes = Vec::new();
    for target in targets {
        changes.extend(target_changes(ctx, target)?);
    }
    Ok(Output { changes })
}

fn target_changes(ctx: &Context, target: &DiffOperation) -> anyhow::Result<Vec<Change>> {
    Ok(match target {
        DiffOperation::Uncommitted(source) => {
            let id_map = IdMap::legacy_new_from_context(ctx)?;
            hunk_changes(
                id_map
                    .uncommitted_hunks
                    .iter()
                    .filter(|(_, hunk)| hunk.source == *source)
                    .map(|(id, hunk)| (id.as_str(), &hunk.hunk, hunk.tree_status))
                    .collect(),
            )
        }
        DiffOperation::Commit { commit } => commit_changes(ctx, commit.commit_id, None)?,
        DiffOperation::Branch { branch } => {
            let branch = branch.shorten().to_string();
            let branch_diff = but_api::branch::branch_diff(ctx, branch)?;
            branch_diff
                .changes
                .into_iter()
                .map(|change| tree_change_to_change(ctx, change))
                .collect()
        }
        DiffOperation::UncommittedHunkOrFile { hunk } => hunk_changes(
            hunk.hunks
                .iter()
                .map(|hunk| (hunk.id.as_str(), &hunk.hunk, hunk.tree_status))
                .collect(),
        ),
        DiffOperation::CommittedFile { commit, path } => {
            commit_changes(ctx, commit.commit_id, Some(path))?
        }
        DiffOperation::CommittedHunk { hunk } => {
            hunk_changes(vec![(hunk.id.as_str(), &hunk.hunk, hunk.tree_status)])
        }
        DiffOperation::PathPrefix { hunks, .. } => hunk_changes(
            hunks
                .iter()
                .map(|hunk| (hunk.id.as_str(), &hunk.hunk, hunk.tree_status))
                .collect(),
        ),
        DiffOperation::CleanPath { .. } => Vec::new(),
    })
}

/// A file and its line counts. Binary and too large files have no lines to count.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileStat {
    path: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    old_path: Option<String>,
    insertions: u64,
    deletions: u64,
}

impl FileStat {
    /// `changes` must be of one file, which is more than one change if it has several hunks.
    fn new(changes: &[Change]) -> Self {
        let count_lines_starting_with = |marker| {
            changes
                .iter()
                .filter_map(|change| match &change.diff {
                    Diff::Patch { hunks, .. } => Some(hunks),
                    Diff::Binary | Diff::TooLarge { .. } => None,
                })
                .flatten()
                .flat_map(|hunk| hunk.diff.lines())
                .filter(|line| line.starts_with(marker))
                .count() as u64
        };
        let Change {
            path,
            status,
            old_path,
            ..
        } = &changes[0];
        FileStat {
            path: path.clone(),
            status: status.clone(),
            old_path: old_path.clone(),
            insertions: count_lines_starting_with('+'),
            deletions: count_lines_starting_with('-'),
        }
    }
}

/// Group by target, as one path in two targets can be different files: all changes of a target are
/// from one checkout or commit.
fn file_stats(ctx: &Context, targets: &[DiffOperation]) -> anyhow::Result<Vec<FileStat>> {
    let mut files = Vec::new();
    for target in targets {
        let changes = target_changes(ctx, target)?;
        files.extend(changes.chunk_by(|a, b| a.path == b.path).map(FileStat::new));
    }
    Ok(files)
}

fn write_stat(
    ctx: &Context,
    targets: &[DiffOperation],
    theme: &'static Theme,
    writer: &mut dyn DiffLineWriter,
) -> anyhow::Result<()> {
    let files = file_stats(ctx, targets)?;
    let names = files
        .iter()
        .map(|file| match &file.old_path {
            Some(old_path) => format!("{old_path} => {}", file.path),
            None => file.path.clone(),
        })
        .collect::<Vec<_>>();
    let name_width = names
        .iter()
        .map(|name| name.chars().count())
        .max()
        .unwrap_or(0);

    let mut total = LineStats {
        files_changed: files.len() as u64,
        ..LineStats::default()
    };
    for (file, name) in files.iter().zip(&names) {
        total.lines_added += file.insertions;
        total.lines_removed += file.deletions;
        writer.write_non_selectable_text(diff_rendering::render_file_line_stats(
            name,
            name_width,
            file.insertions,
            file.deletions,
        ))?;
    }
    if !files.is_empty() {
        writer.write_non_selectable_text(diff_rendering::render_line_stats(total))?;
    }

    for target in targets {
        if let DiffOperation::CleanPath { path } = target {
            writer.write_non_selectable_text(clean_path_line(path, theme))?;
        }
    }
    Ok(())
}

fn clean_path_line(path: &str, theme: &'static Theme) -> Line<'static> {
    Line::from(format!("'{path}' has no uncommitted changes")).style(theme.hint)
}

struct DiffWriter<'a> {
    out: &'a mut dyn WriteWithUtils,
    theme: &'static Theme,
}

impl DiffLineWriter for DiffWriter<'_> {
    fn write(&mut self, line: DetailsLine) -> anyhow::Result<()> {
        match line {
            DetailsLine::Text { line, .. } => {
                let line_style = line.style;
                for span in line.spans {
                    let rendered = line_style.patch(span.style).paint(&span.content);
                    write!(self.out, "{rendered}")?;
                }
                writeln!(self.out)?;
            }
            DetailsLine::TextToWrap { id: _, text } => {
                writeln!(self.out, "{text}")?;
            }
            DetailsLine::Code(code_line) => {
                let syntax_highlighted_line = code_line.syntax_highlighted_line.borrow();
                let syntax_highlighted_line = syntax_highlighted_line
                    .as_ref()
                    .expect("WithSyntaxHighlighting ensures the line is highlighted");

                let line_style = syntax_highlighted_line.style;
                for span in syntax_highlighted_line {
                    let rendered = line_style.patch(span.style).paint(&span.content);
                    write!(self.out, "{rendered}")?;
                }
                if line_style.bg.is_some() && colored::control::SHOULD_COLORIZE.should_colorize() {
                    write!(self.out, "{}", line_style.paint(CLEAR_TO_END_OF_LINE))?;
                }
                writeln!(self.out)?;
            }
            DetailsLine::SectionSeparator => {
                writeln!(self.out)?;
            }
            DetailsLine::HunkHeader { width, line, .. } => {
                for _ in 0..width {
                    write!(self.out, "{}", self.theme.border.paint("─"))?;
                }
                writeln!(self.out, "{}", self.theme.border.paint("╮"))?;

                for span in line {
                    let rendered = span.style.paint(&span.content);
                    write!(self.out, "{rendered}")?;
                }
                writeln!(self.out)?;

                for _ in 0..width {
                    write!(self.out, "{}", self.theme.border.paint("─"))?;
                }
                writeln!(self.out, "{}", self.theme.border.paint("╯"))?;

                writeln!(self.out)?;
            }
        }

        Ok(())
    }
}

pub fn diff<'a>(
    ctx: &'a mut Context,
    _out: IntermediateChannel<'_>,
    args: Platform,
    invoked_from: &InvokedFrom,
) -> CliResult<DiffOutcome<'a>> {
    let guard = ctx.shared_worktree_access();
    let id_map = IdMap::new_from_context(ctx, guard.read_permission(), Some(invoked_from))?;

    let Platform { targets, stat } = args;
    let targets = resolve(ctx, &id_map, targets, invoked_from)?;
    Ok(run(ctx, targets, stat)?)
}

fn resolve(
    ctx: &Context,
    id_map: &IdMap,
    targets: Vec<CliIdArg>,
    invoked_from: &InvokedFrom,
) -> CliResult<Vec<DiffOperation>> {
    if targets.is_empty() {
        let source = invoked_from.managed_source(id_map)?;
        return Ok(vec![DiffOperation::Uncommitted(source)]);
    }

    let repo = ctx.repo.get()?;
    let operations = targets
        .iter()
        .map(|target| resolve_target(ctx, target, &targets, &repo, id_map, invoked_from))
        .collect::<CliResult<Vec<_>>>()?;

    // `git diff <a> <b>` compares the two, so showing each in turn would read like a comparison.
    let has_revision = operations.iter().any(|operation| {
        matches!(
            operation,
            DiffOperation::Commit { .. } | DiffOperation::Branch { .. }
        )
    });
    if has_revision && operations.len() > 1 {
        return Err(
            bad_input("A commit or branch must be the only target of `but diff`")
                .hint(
                    "To compare two revisions, run `git diff <a> <b>`. To see each target on its own, run `but diff <target>` once per target.",
                )
                .into(),
        );
    }
    Ok(operations)
}

fn resolve_target(
    ctx: &Context,
    target: &CliIdArg,
    targets: &[CliIdArg],
    repo: &gix::Repository,
    id_map: &IdMap,
    invoked_from: &InvokedFrom,
) -> CliResult<DiffOperation> {
    let Some(resolved_target) = target.try_resolve(repo, id_map, Purpose::Target, None)? else {
        return resolve_missing_target(ctx, target, targets, repo, id_map, invoked_from);
    };

    match resolved_target {
        ResolvedCliIdArg::Uncommitted(source) => Ok(DiffOperation::Uncommitted(source)),
        ResolvedCliIdArg::Commit(commit) => Ok(DiffOperation::Commit { commit }),
        ResolvedCliIdArg::Branch(branch) => {
            let branch = branch.resolve_local_branch_name()?;
            Ok(DiffOperation::Branch { branch })
        }
        ResolvedCliIdArg::AnonymousSegment(segment) => {
            Err(crate::args::atoms::anonymous_segment_error(&segment.id))
        }
        ResolvedCliIdArg::UncommittedHunkOrFile(hunk) => {
            Ok(DiffOperation::UncommittedHunkOrFile { hunk })
        }
        ResolvedCliIdArg::CommittedFile(CommittedFileId {
            commit_id,
            path,
            change_id,
        }) => Ok(DiffOperation::CommittedFile {
            commit: CommitId {
                commit_id,
                change_id,
            },
            path,
        }),
        ResolvedCliIdArg::CommittedHunk(hunk) => Ok(DiffOperation::CommittedHunk { hunk }),
        ResolvedCliIdArg::PathPrefix {
            id,
            hunks,
            source: _,
        } => Ok(DiffOperation::PathPrefix { id, hunks }),
        ResolvedCliIdArg::Stack { .. } => {
            Err(bad_input("viewing diffs for stack assignments is not supported").into())
        }
    }
}

/// Agents often pass Git revisions like `HEAD~1` or `main...HEAD`, which are not CLI IDs.
/// When a target fails to resolve but Git can parse it, point at `git diff` in addition
/// to the generic missing-target hint.
///
/// A path that exists resolves to nothing when it has no changes. That is an empty diff, not a
/// mistake.
fn resolve_missing_target(
    ctx: &Context,
    target: &CliIdArg,
    targets: &[CliIdArg],
    repo: &gix::Repository,
    id_map: &IdMap,
    invoked_from: &InvokedFrom,
) -> CliResult<DiffOperation> {
    let err = bad_input(format!("Could not find target: '{target}'"));
    if repo.rev_parse(target.0.as_str()).is_ok() {
        let targets = targets.iter().join(" ");
        return Err(err
            .hint(format!(
                "To diff it as a Git revision, run `git diff {targets}`. {}",
                CliIdArg::TARGET_MISSING_HINT
            ))
            .into());
    }

    if is_clean_path(ctx, target, repo, id_map, invoked_from)? {
        return Ok(DiffOperation::CleanPath {
            path: target.to_string(),
        });
    }
    Err(err
        .hint(target.missing_hint(CliIdArg::TARGET_MISSING_HINT, repo, id_map))
        .into())
}

/// Whether `target`, which resolved to nothing, is a path in the checkout `but` runs in without
/// changes. A checkout GitButler doesn't manage has no known changes, and a file with unresolved
/// conflicts has no CLI ID.
fn is_clean_path(
    ctx: &Context,
    target: &CliIdArg,
    repo: &gix::Repository,
    id_map: &IdMap,
    invoked_from: &InvokedFrom,
) -> CliResult<bool> {
    let Some(path) = invoked_from
        .directory
        .as_ref()
        .and_then(|directory| directory.existing_path_from_root(&target.0))
    else {
        return Ok(false);
    };
    let source = invoked_from.managed_source(id_map)?;
    let checkout = ChangeSourceRepo::open(ctx, &source)?;
    let conflicted = conflicted_worktree_paths(checkout.repo(repo))?;
    Ok(!conflicted
        .iter()
        .any(|file| Path::new(file).starts_with(&path)))
}

fn run(
    ctx: &mut Context,
    targets: Vec<DiffOperation>,
    stat: bool,
) -> anyhow::Result<DiffOutcome<'_>> {
    Ok(DiffOutcome { ctx, targets, stat })
}

#[derive(Debug)]
enum DiffOperation {
    Uncommitted(ChangeSourceId),
    Commit {
        commit: CommitId,
    },
    Branch {
        branch: FullName,
    },
    UncommittedHunkOrFile {
        hunk: Box<UncommittedHunkOrFile>,
    },
    CommittedFile {
        commit: CommitId,
        path: BString,
    },
    CommittedHunk {
        hunk: Box<CommittedHunk>,
    },
    PathPrefix {
        id: String,
        hunks: NonEmpty<IdAndHunk>,
    },
    /// A path that exists, as named on the command line, without uncommitted changes.
    CleanPath {
        path: String,
    },
}
