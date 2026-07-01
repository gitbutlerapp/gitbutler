use std::{
    collections::HashSet,
    iter::{once, repeat_n},
    sync::Arc,
};

use anyhow::bail;
use bstr::{BStr, BString, ByteSlice as _};
use but_core::{
    UnifiedPatch,
    diff::LineStats,
    ui::{TreeChange, TreeStatus},
    unified_diff::DiffHunk,
};
use but_ctx::Context;
use but_hunk_assignment::HunkAssignment;
use gix::{ObjectId, actor::Signature};
use itertools::{Itertools, Position};
use ratatui::{
    style::Stylize as _,
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr as _;

use crate::{
    CliId, IdMap,
    command::legacy::status::tui::details2::{
        CodeLineNumbers, IdGen, LineWriter, SectionId, num_digits,
    },
    id::{ShortId, UncommittedHunk, UncommittedHunkOrFile},
    theme::Theme,
};

pub fn render_commit(
    commit: ObjectId,
    ctx: &mut Context,
    theme: &'static Theme,
    id_gen: &mut IdGen<'_>,
    out: &mut dyn LineWriter,
) -> anyhow::Result<()> {
    let mut id_gen = id_gen.scoped("commits");
    let mut id_gen = id_gen.scoped(commit);

    let commit_details =
        but_api::diff::commit_details(ctx, commit, but_api::diff::ComputeLineStats::No)?;

    let header_id = id_gen.new_id("header");

    out.push_selectable_text(
        header_id,
        Line::from_iter([
            Span::raw(format!("{:<11}", "Commit ID:")),
            Span::styled(commit.to_hex().to_string(), theme.commit_id),
        ]),
    )?;
    out.push_selectable_text(
        header_id,
        Line::from_iter(
            once(Span::raw(format!("{:<11}", "Author:")))
                .chain(render_signature(&commit_details.commit.author, theme)),
        ),
    )?;
    out.push_selectable_text(
        header_id,
        Line::from_iter(
            once(Span::raw(format!("{:<11}", "Committer:")))
                .chain(render_signature(&commit_details.commit.committer, theme)),
        ),
    )?;

    out.push_empty_line(header_id)?;

    let message = commit_details.commit.message.to_string();
    if message.is_empty() {
        out.push_selectable_text(
            header_id,
            Line::from("(no commit message)").style(theme.hint),
        )?;
    } else {
        out.push_text_to_wrap(header_id, message)?;
    }
    out.push_empty_line(header_id)?;

    let tree_changes = commit_details
        .diff_with_first_parent
        .iter()
        .map(|change| TreeChange::from(change.clone()))
        .collect::<Vec<_>>();

    let tree_changes = tree_changes_with_patches(ctx, tree_changes);

    let mut line_stats = LineStats::default();
    compute_line_stats_from_tree_changes(&tree_changes, &mut line_stats);
    out.push_selectable_text(header_id, render_line_stats(line_stats))?;
    out.push_section_separator()?;

    render_tree_changes(tree_changes, theme, &mut id_gen, out)?;

    Ok(())
}

pub fn render_branch(
    name: String,
    ctx: &mut Context,
    theme: &'static Theme,
    id_gen: &mut IdGen<'_>,
    out: &mut dyn LineWriter,
) -> anyhow::Result<()> {
    let mut id_gen = id_gen.scoped("branches");
    let mut id_gen = id_gen.scoped(&name);

    let tree_changes = but_api::branch::branch_diff(ctx, name.clone())?;
    let tree_changes = tree_changes_with_patches(ctx, tree_changes.changes);

    let mut line_stats = LineStats::default();
    compute_line_stats_from_tree_changes(&tree_changes, &mut line_stats);
    out.push_selectable_text(id_gen.new_id("line_stats"), render_line_stats(line_stats))?;
    out.push_section_separator()?;

    render_tree_changes(tree_changes, theme, &mut id_gen, out)?;

    Ok(())
}

pub fn render_uncommitted(
    ctx: &mut Context,
    theme: &'static Theme,
    id_gen: &mut IdGen<'_>,
    out: &mut dyn LineWriter,
) -> anyhow::Result<()> {
    let mut id_gen = id_gen.scoped("uncommitted");

    let wt_changes = but_api::diff::changes_in_worktree(ctx)?;
    let id_map = IdMap::legacy_new_from_context(ctx, Some(wt_changes.assignments))?;
    let uncommitted_hunks = filter_uncommitted_hunks(ctx, &id_map, |hunk_assignment| {
        hunk_assignment.stack_id.is_none()
    })?;

    let line_stats = render_line_stats(compute_line_stats_from_uncommitted_hunks(
        &uncommitted_hunks,
    ));
    out.push_selectable_text(id_gen.new_id("line_stats"), line_stats)?;
    out.push_section_separator()?;

    for (pos, (raw_id, _cli_id, UncommittedHunk { hunk_assignment })) in
        uncommitted_hunks.into_iter().with_position()
    {
        let id = id_gen.new_id(raw_id);

        render_hunk_path_header(
            id,
            hunk_assignment.path_bytes.as_ref(),
            Some(ShortIdOrTreeStatus::ShortId(raw_id)),
            out,
            theme,
        )?;

        out.push_empty_line(id)?;

        render_hunk_assignment(id, hunk_assignment, theme, out)?;

        if pos.needs_padding_below() {
            out.push_section_separator()?;
        }
    }

    Ok(())
}

pub fn render_uncommitted_hunk(
    hunk: UncommittedHunkOrFile,
    ctx: &mut Context,
    theme: &'static Theme,
    id_gen: &mut IdGen<'_>,
    out: &mut dyn LineWriter,
) -> anyhow::Result<()> {
    let mut id_gen = id_gen.scoped("hunk");
    let mut id_gen = id_gen.scoped(&hunk.id);

    let wt_changes = but_api::diff::changes_in_worktree(ctx)?;
    let id_map = IdMap::legacy_new_from_context(ctx, Some(wt_changes.assignments))?;
    let uncommitted_hunks = filter_uncommitted_hunks(ctx, &id_map, |hunk_assignment| {
        uncommitted_hunk_matches_selection(hunk_assignment, &hunk)
    })?;

    let line_stats = render_line_stats(compute_line_stats_from_uncommitted_hunks(
        &uncommitted_hunks,
    ));
    out.push_selectable_text(id_gen.new_id("line_stats"), line_stats)?;
    out.push_section_separator()?;

    for (pos, (raw_id, _cli_id, UncommittedHunk { hunk_assignment })) in
        uncommitted_hunks.into_iter().with_position()
    {
        let id = id_gen.new_id(raw_id);

        render_hunk_path_header(
            id,
            hunk_assignment.path_bytes.as_ref(),
            Some(ShortIdOrTreeStatus::ShortId(raw_id)),
            out,
            theme,
        )?;

        out.push_empty_line(id)?;

        render_hunk_assignment(id, hunk_assignment, theme, out)?;

        if pos.needs_padding_below() {
            out.push_section_separator()?;
        }
    }

    Ok(())
}

pub fn render_committed_file(
    commit: ObjectId,
    path: BString,
    id: ShortId,
    ctx: &mut Context,
    theme: &'static Theme,
    id_gen: &mut IdGen<'_>,
    out: &mut dyn LineWriter,
) -> anyhow::Result<()> {
    let mut id_gen = id_gen.scoped("committed_file");
    let mut id_gen = id_gen.scoped(commit);
    let mut id_gen = id_gen.scoped(id);

    let commit_details =
        but_api::diff::commit_details(ctx, commit, but_api::diff::ComputeLineStats::No)?;

    let tree_changes = commit_details
        .diff_with_first_parent
        .iter()
        .filter(|change| change.path == path)
        .map(|change| TreeChange::from(change.clone()))
        .collect::<Vec<_>>();
    let tree_changes = tree_changes_with_patches(ctx, tree_changes);

    let mut line_stats = LineStats::default();
    compute_line_stats_from_tree_changes(&tree_changes, &mut line_stats);
    out.push_selectable_text(id_gen.new_id("line_stats"), render_line_stats(line_stats))?;
    out.push_section_separator()?;

    render_tree_changes(tree_changes, theme, &mut id_gen, out)?;

    Ok(())
}

fn tree_changes_with_patches(
    ctx: &Context,
    tree_changes: Vec<TreeChange>,
) -> Vec<(TreeChange, UnifiedPatch)> {
    tree_changes
        .into_iter()
        .filter_map(|tree_change| {
            let patch = but_api::diff::tree_change_diffs(ctx, tree_change.clone())
                .ok()
                .flatten()?;
            Some((tree_change, patch))
        })
        .collect::<Vec<_>>()
}

fn render_hunk_assignment(
    id: SectionId,
    hunk_assignment: &HunkAssignment,
    theme: &'static Theme,
    out: &mut dyn LineWriter,
) -> anyhow::Result<()> {
    if let Some(hunk_header) = hunk_assignment.hunk_header {
        if let Some(diff) = hunk_assignment.diff.clone() {
            let hunk = DiffHunk {
                old_start: hunk_header.old_start,
                old_lines: hunk_header.old_lines,
                new_start: hunk_header.new_start,
                new_lines: hunk_header.new_lines,
                diff,
            };

            let is_result_of_binary_to_text_conversion = false;

            let path = Arc::new(hunk_assignment.path_bytes.clone());

            render_unified_patch(
                id,
                &path,
                hunk,
                is_result_of_binary_to_text_conversion,
                theme,
                out,
            )?;
        } else {
            out.push_selectable_text(id, "No diff available".into())?;
        }
    } else {
        out.push_selectable_text(
            id,
            "No diff available - file is either empty, binary, or too large".into(),
        )?;
    }

    Ok(())
}

fn compute_line_stats_from_tree_changes(
    tree_changes: &[(TreeChange, UnifiedPatch)],
    line_stats: &mut LineStats,
) {
    for (_tree_change, patch) in tree_changes {
        line_stats.files_changed += 1;
        match patch {
            UnifiedPatch::Patch {
                hunks: _,
                is_result_of_binary_to_text_conversion: _,
                lines_added,
                lines_removed,
            } => {
                line_stats.lines_added += (*lines_added) as u64;
                line_stats.lines_removed += (*lines_removed) as u64;
            }
            UnifiedPatch::Binary | UnifiedPatch::TooLarge { .. } => {}
        }
    }
}

fn render_tree_changes(
    tree_changes: Vec<(TreeChange, UnifiedPatch)>,
    theme: &'static Theme,
    id_gen: &mut IdGen<'_>,
    out: &mut dyn LineWriter,
) -> anyhow::Result<()> {
    let mut id_gen = id_gen.scoped("tree_changes");

    for (tree_change_pos, (i, (tree_change, patch))) in
        tree_changes.into_iter().enumerate().with_position()
    {
        let mut id_gen = id_gen.scoped(i);
        let path = Arc::new(tree_change.path_bytes.clone());
        match patch {
            UnifiedPatch::Patch {
                hunks,
                is_result_of_binary_to_text_conversion,
                lines_added: _,
                lines_removed: _,
            } => {
                let mut first_hunk = true;
                let mut id_gen = id_gen.scoped("hunks");
                for (hunk_pos, (j, hunk)) in hunks.into_iter().enumerate().with_position() {
                    let hunk_id = id_gen.new_id(j);

                    if std::mem::take(&mut first_hunk) {
                        render_hunk_path_header(
                            hunk_id,
                            tree_change.path.as_ref(),
                            Some(ShortIdOrTreeStatus::TreeStatus(&tree_change.status)),
                            out,
                            theme,
                        )?;
                    }

                    render_unified_patch(
                        hunk_id,
                        &path,
                        hunk,
                        is_result_of_binary_to_text_conversion,
                        theme,
                        out,
                    )?;

                    if tree_change_pos.needs_padding_below() || hunk_pos.needs_padding_below() {
                        out.push_section_separator()?;
                    }
                }
            }
            UnifiedPatch::Binary => {
                let patch_id = id_gen.new_id("binary");

                render_hunk_path_header(
                    patch_id,
                    tree_change.path.as_ref(),
                    Some(ShortIdOrTreeStatus::TreeStatus(&tree_change.status)),
                    out,
                    theme,
                )?;

                out.push_selectable_text(patch_id, "Binary file - no diff available".into())?;

                if tree_change_pos.needs_padding_below() {
                    out.push_section_separator()?;
                }
            }
            UnifiedPatch::TooLarge { size_in_bytes } => {
                let patch_id = id_gen.new_id("too_large");

                render_hunk_path_header(
                    patch_id,
                    tree_change.path.as_ref(),
                    Some(ShortIdOrTreeStatus::TreeStatus(&tree_change.status)),
                    out,
                    theme,
                )?;

                out.push_selectable_text(
                    patch_id,
                    format!("File too large ({size_in_bytes} bytes) - no diff available").into(),
                )?;

                if tree_change_pos.needs_padding_below() {
                    out.push_section_separator()?;
                }
            }
        }
    }

    Ok(())
}

fn render_signature(
    sig: &Signature,
    theme: &'static Theme,
) -> impl IntoIterator<Item = Span<'static>> {
    [
        Span::styled(sig.name.to_string(), theme.user),
        Span::raw(" <"),
        Span::styled(sig.email.to_string(), theme.user),
        Span::raw(">"),
        Span::raw(" ("),
        Span::styled(
            sig.time.format_or_unix(gix::date::time::format::DEFAULT),
            theme.time,
        ),
        Span::raw(")"),
    ]
    .into_iter()
}

enum ShortIdOrTreeStatus<'a> {
    ShortId(&'a str),
    TreeStatus(&'a TreeStatus),
}

fn render_hunk_path_header(
    id: SectionId,
    path: &BStr,
    status: Option<ShortIdOrTreeStatus<'_>>,
    out: &mut dyn LineWriter,
    theme: &'static Theme,
) -> anyhow::Result<()> {
    let status = status.map(|id_or_status| match id_or_status {
        ShortIdOrTreeStatus::ShortId(id) => Span::styled(id.to_owned(), theme.cli_id),
        ShortIdOrTreeStatus::TreeStatus(status) => change_status(status, theme),
    });
    let path = path.to_string();
    let path_line = Line::from_iter(
        [Span::raw(" ")]
            .into_iter()
            .chain(
                status
                    .into_iter()
                    .flat_map(|status| [status, Span::raw(" ")]),
            )
            .chain([Span::raw(path)]),
    );
    bordered_line_top_right_bottom(id, path_line, out, theme)?;
    Ok(())
}

fn change_status(status: &TreeStatus, theme: &'static Theme) -> Span<'static> {
    match status {
        TreeStatus::Addition { .. } => Span::styled("added", theme.addition),
        TreeStatus::Deletion { .. } => Span::styled("deleted", theme.deletion),
        TreeStatus::Modification { .. } => Span::styled("modified", theme.modification),
        TreeStatus::Rename { .. } => Span::styled("renamed", theme.renaming),
    }
}

fn bordered_line_top_right_bottom(
    id: SectionId,
    mut text: Line<'static>,
    out: &mut dyn LineWriter,
    theme: &'static Theme,
) -> anyhow::Result<()> {
    let width_including_padding = text.width() + 1;

    out.push_hunk_header(
        id,
        Line::from_iter(repeat_n("─", width_including_padding).chain(once("╮")))
            .style(theme.border),
    )?;

    text.spans
        .extend([Span::raw(" "), Span::styled("│", theme.border)]);
    out.push_hunk_header(id, text)?;

    out.push_hunk_header(
        id,
        Line::from_iter(repeat_n("─", width_including_padding).chain(once("╯")))
            .style(theme.border),
    )?;

    out.push_hunk_header(id, " ".into())?;

    Ok(())
}

fn render_unified_patch(
    id: SectionId,
    path: &Arc<BString>,
    hunk: DiffHunk,
    is_result_of_binary_to_text_conversion: bool,
    theme: &'static Theme,
    out: &mut dyn LineWriter,
) -> anyhow::Result<()> {
    let DiffHunk {
        old_start,
        new_start,
        diff,
        old_lines: _,
        new_lines: _,
    } = hunk;

    if is_result_of_binary_to_text_conversion {
        out.push_selectable_text(id, "(diff generated from binary-to-text conversion)".into())?;
    }

    if let Some(headers) = diff.lines().next() {
        out.push_selectable_text(
            id,
            Span::styled(headers.to_str_lossy().to_string(), theme.hint).into(),
        )?;

        out.push_hunk_header(
            id,
            Line::from_iter(repeat_n("─", headers.to_str_lossy().width())).style(theme.border),
        )?;
    }

    let (old_width, new_width) = {
        let mut old_line = old_start;
        let mut new_line = new_start;
        for line in diff.lines().skip(1) {
            if line.starts_with(b"+") {
                new_line += 1;
            } else if line.starts_with(b"-") {
                old_line += 1;
            } else {
                old_line += 1;
                new_line += 1;
            }
        }
        (num_digits(old_line), num_digits(new_line))
    };

    let mut old_line_num = old_start;
    let mut new_line_num = new_start;

    let diff = Arc::new(diff);

    let mut first_line = true;
    let mut i = 0;
    for line in diff.lines_with_terminator() {
        if std::mem::take(&mut first_line) {
            i += line.len();
            continue;
        }

        let (line_numbers, line_start_end) = if let Some(rest) = line.strip_prefix(b"+") {
            let start = i + 1;
            let end = start + rest.len();
            let line_numbers = CodeLineNumbers::addition(old_width, new_width, new_line_num);
            new_line_num += 1;
            (line_numbers, (start, end))
        } else if let Some(rest) = line.strip_prefix(b"-") {
            let start = i + 1;
            let end = start + rest.len();
            let line_numbers = CodeLineNumbers::deletion(old_width, new_width, old_line_num);
            old_line_num += 1;
            (line_numbers, (start, end))
        } else {
            let (start, end) = if let Some(rest) = line.strip_prefix(b" ") {
                let start = i + 1;
                (start, start + rest.len())
            } else {
                (i, i + line.len())
            };
            let line_numbers =
                CodeLineNumbers::context(old_width, new_width, old_line_num, new_line_num);
            old_line_num += 1;
            new_line_num += 1;
            (line_numbers, (start, end))
        };

        out.push_code(
            id,
            line_numbers,
            line_start_end,
            Arc::clone(&diff),
            Arc::clone(path),
        )?;

        i += line.len();
    }

    Ok(())
}

fn compute_line_stats_from_uncommitted_hunks(
    uncommitted_hunks: &[(&str, Arc<CliId>, &UncommittedHunk)],
) -> LineStats {
    let mut line_stats = LineStats::default();
    let mut unique_paths = HashSet::new();
    for (_, _, hunk) in uncommitted_hunks {
        let hunk_assignment = &hunk.hunk_assignment;
        unique_paths.insert(&hunk_assignment.path_bytes);
        line_stats.lines_added += hunk_assignment
            .line_nums_added
            .as_ref()
            .map_or(0, |lines| lines.len() as u64);
        line_stats.lines_removed += hunk_assignment
            .line_nums_removed
            .as_ref()
            .map_or(0, |lines| lines.len() as u64);
    }
    line_stats.files_changed = unique_paths.len() as u64;
    line_stats
}

fn filter_uncommitted_hunks<'a, F>(
    ctx: &'a mut Context,
    id_map: &'a IdMap,
    mut filter: F,
) -> anyhow::Result<Vec<(&'a str, Arc<CliId>, &'a UncommittedHunk)>>
where
    F: FnMut(&HunkAssignment) -> bool,
{
    let mut uncommitted_hunks = id_map
        .uncommitted_hunks
        .iter()
        .filter(move |(_, hunk)| filter(&hunk.hunk_assignment))
        .map(|(raw_id, hunk)| {
            let mut cli_ids = id_map.parse_using_context(raw_id, ctx)?;
            if cli_ids.len() == 1 {
                Ok((&**raw_id, Arc::new(cli_ids.remove(0)), hunk))
            } else if cli_ids.is_empty() {
                bail!("'{raw_id}' no found")
            } else {
                bail!(
                    "'{raw_id}' resolved to more than one hunk ({})",
                    cli_ids.len()
                )
            }
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    uncommitted_hunks.sort_by(|(id_a, _, hunk_a), (id_b, _, hunk_b)| {
        (
            &hunk_a.hunk_assignment.path_bytes,
            hunk_a
                .hunk_assignment
                .hunk_header
                .as_ref()
                .map(|header| header.old_start),
            id_a,
        )
            .cmp(&(
                &hunk_b.hunk_assignment.path_bytes,
                hunk_b
                    .hunk_assignment
                    .hunk_header
                    .as_ref()
                    .map(|header| header.old_start),
                id_b,
            ))
    });

    Ok(uncommitted_hunks)
}

/// Returns true if `hunk_assignment` is part of the selected uncommitted entity.
fn uncommitted_hunk_matches_selection(
    hunk_assignment: &HunkAssignment,
    hunk: &UncommittedHunkOrFile,
) -> bool {
    let selected_hunk = hunk.hunk_assignments.first();

    if hunk.is_entire_file {
        hunk_assignment.path_bytes == selected_hunk.path_bytes
            && hunk_assignment.stack_id == selected_hunk.stack_id
    } else {
        hunk_assignment == selected_hunk && hunk_assignment.stack_id == selected_hunk.stack_id
    }
}

trait PositionExt {
    fn needs_padding_below(self) -> bool;
}

impl PositionExt for Position {
    fn needs_padding_below(self) -> bool {
        match self {
            Position::First | Position::Middle => true,
            Position::Last | Position::Only => false,
        }
    }
}

fn render_line_stats(line_stats: LineStats) -> Line<'static> {
    let LineStats {
        lines_added,
        lines_removed,
        files_changed,
    } = line_stats;

    Line::from_iter([
        if files_changed == 1 {
            Span::raw(format!("{files_changed} file changed"))
        } else {
            Span::raw(format!("{files_changed} files changed"))
        },
        Span::raw(", "),
        Span::raw(format!("+{lines_added}")).green(),
        Span::raw(" "),
        Span::raw(format!("-{lines_removed}")).red(),
    ])
}
