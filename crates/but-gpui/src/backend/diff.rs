//! Diffs of a worktree, a commit, a range of commits, a branch or an operation, parsed into
//! hunks of lines and, for the first files, highlighted.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context as _, Result};
use bstr::{BStr, BString, ByteSlice};

use super::{
    CommitId, Conflict, DiffBody, DiffLine, FileChange, FileDiff, Hunk, HunkKey, LineKind, Picture,
    SnapshotId, annotate, checkout, columns, file_change, highlight, markers, open,
    present_first_parent, worktree_files,
};

/// The files each of `commits` changes, from its first parent, by their paths.
pub fn commit_paths(path: &Path, commits: &[CommitId]) -> Result<Vec<(CommitId, Vec<String>)>> {
    let ctx = open(path)?;
    let repo = checkout(&ctx, None)?;
    (commits.iter())
        .map(|id| {
            // One gone, as a deleted branch's once pruned, changes nothing that can be read.
            let Ok(commit) = repo.find_commit(id.0) else {
                return Ok((*id, Vec::new()));
            };
            let base = present_first_parent(&commit);
            let changes = but_core::diff::tree_changes(&repo, base, id.0)?;
            let paths = changes
                .into_iter()
                .map(|c| c.path.to_str_lossy().into_owned());
            Ok((*id, paths.collect()))
        })
        .collect()
}

/// The commits that last changed `lines` (1-based) of `file` as commit `tip` has it, following
/// it through renames, each with the file's path in that commit.
pub fn blame_lines(
    path: &Path,
    tip: CommitId,
    file: &BStr,
    lines: &[u32],
) -> Result<HashMap<CommitId, String>> {
    let ctx = open(path)?;
    let repo = checkout(&ctx, None)?;
    let options = gix::repository::blame_file::Options {
        ranges: gix::blame::BlameRanges::from_one_based_inclusive_ranges(
            lines.iter().map(|&line| line..=line).collect(),
        )?,
        rewrites: Some(Default::default()),
        ..Default::default()
    };
    let blame = repo.blame_file(file, tip.0, options)?;
    Ok((blame.entries.iter())
        .map(|e| {
            let named = e.source_file_name.as_ref().map(|name| name.to_string());
            (
                CommitId(e.commit_id),
                named.unwrap_or_else(|| file.to_string()),
            )
        })
        .collect())
}

/// What a diff is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffSource {
    Worktree,
    /// The uncommitted changes of the linked worktree with this name, like an agent's.
    Linked(BString),
    Commit(CommitId),
    /// Everything from `oldest` (inclusive) up to `newest`, like a whole branch.
    Range {
        oldest: CommitId,
        newest: CommitId,
    },
    /// Each of these commits' own changes in turn, oldest first: for commits picked with
    /// others between them, whose one range would bring in the others' changes too.
    Commits(std::sync::Arc<[CommitId]>),
    /// What a branch outside the workspace changes: from where it forked off the target to
    /// its tip `tip`, as its pull request would show. `ahead` is how many commits it has that
    /// the target doesn't: a fetch can land some on the target without moving the tip.
    Branch {
        tip: CommitId,
        ahead: Option<usize>,
    },
    /// What a pull brings in from the target: from where what's checked out parts from it to
    /// its tip `tip`. `behind` is how many commits it has that what's checked out doesn't: a
    /// branch applied with some of them moves where they part without moving the tip.
    Incoming {
        tip: CommitId,
        behind: usize,
    },
    /// What the operation that snapshot `id` was taken for changed: up to the snapshot
    /// after it, `newer`, or to now.
    Snapshot {
        id: SnapshotId,
        newer: Option<SnapshotId>,
    },
}

impl DiffSource {
    /// Whether the diff follows a worktree, so it's loaded again as files change.
    pub fn live(&self) -> bool {
        matches!(self, DiffSource::Worktree | DiffSource::Linked(_))
    }
}

/// The diff of `source`. A worktree diff takes the `worktree` files the workspace just
/// listed, rather than looking at the worktree a second time. Files whose patch is the same
/// as in `previous` keep their highlighting, which is most of what loading costs. With
/// `annotate_first`, only that many files, those shown first, are highlighted and given scope
/// names, and only the short ones, so a load with `None` does the rest. A hunk's scope adds at
/// most `scope_lines` unchanged lines around it, and isn't read with `None`.
pub fn load_diff(
    path: &Path,
    source: DiffSource,
    worktree: Option<Vec<FileChange>>,
    previous: Option<Arc<Vec<FileDiff>>>,
    annotate_first: Option<usize>,
    scope_lines: Option<usize>,
) -> Result<Vec<FileDiff>> {
    let started = std::time::Instant::now();
    let ctx = open(path)?;
    // A linked worktree's changes are read and diffed in its own checkout.
    let linked = match &source {
        DiffSource::Linked(name) => Some(name.as_ref()),
        _ => None,
    };
    let repo = checkout(&ctx, linked)?;
    let workdir = repo.workdir().map(Path::to_path_buf);
    let tree_changes = |base, tip| -> Result<Vec<FileChange>> {
        let changes = but_core::diff::tree_changes(&repo, base, tip)?;
        Ok(changes.into_iter().map(file_change).collect())
    };
    // Which commit each file is from, for commits shown one after another.
    let mut owners = Vec::new();
    let changes: Vec<FileChange> = match &source {
        // With modification times, as the workspace's file list has them.
        DiffSource::Worktree | DiffSource::Linked(_) => match worktree {
            Some(files) => files,
            None => worktree_files(&repo, &[])?,
        },
        // At the edge of a shallow clone a commit's parent isn't here; Git shows it as a
        // root then, everything in it added.
        DiffSource::Commit(id) => {
            tree_changes(present_first_parent(&repo.find_commit(id.0)?), id.0)?
        }
        DiffSource::Range { oldest, newest } => {
            tree_changes(present_first_parent(&repo.find_commit(oldest.0)?), newest.0)?
        }
        DiffSource::Commits(ids) => {
            let mut all = Vec::new();
            for id in ids.iter() {
                let changes = tree_changes(present_first_parent(&repo.find_commit(id.0)?), id.0)?;
                owners.extend(std::iter::repeat_n(*id, changes.len()));
                all.extend(changes);
            }
            all
        }
        DiffSource::Branch { tip, .. } => {
            let target = super::target_source(&repo, &ctx.project_meta()?)
                .and_then(|name| super::ref_tip(&repo, name.as_ref()));
            let base = target
                .and_then(|target| repo.merge_base(tip.0, target).ok().flatten())
                .map(|id| id.detach());
            tree_changes(base, tip.0)?
        }
        DiffSource::Incoming { tip, .. } => {
            let head = repo.head_id()?.detach();
            let base = (repo.merge_base(head, tip.0).ok().flatten()).map(|id| id.detach());
            tree_changes(base, tip.0)?
        }
        DiffSource::Snapshot { id, newer } => {
            but_api::legacy::oplog::snapshot_diff(&ctx, id.0, newer.map(|newer| newer.0))?
                .into_iter()
                .map(|change| file_change(change.into()))
                .collect()
        }
    };
    let context_lines = ctx.settings.context_lines;
    let previous: HashMap<&BStr, &FileDiff> = previous
        .iter()
        .flat_map(|files| files.iter())
        .map(|file| (file.file.change.path.as_bstr(), file))
        .collect();
    // Files past the first stay plain unless reused, and a long one among the first waits too.
    let (first, limit) = match annotate_first {
        Some(first) => (first, FIRST_HIGHLIGHT_LIMIT),
        None => (usize::MAX, HIGHLIGHT_LIMIT),
    };
    // Drawing both sides of hundreds of SVGs, as snapshot tests leave, takes seconds and
    // gigabytes; that many show as text.
    let draw_svgs = changes.iter().filter(|file| is_svg(&file.path)).count() <= SVGS_DRAWN;
    let repo = repo.into_sync();
    let mut diffs = parallel_map(
        changes,
        || repo.to_thread_local(),
        |repo, index, file| {
            let previous = previous.get(file.change.path.as_bstr()).copied();
            load_file(
                repo,
                file,
                context_lines,
                previous,
                (index < first).then_some(limit),
                scope_lines,
                draw_svgs,
            )
        },
    )
    .into_iter()
    .collect::<Result<Vec<_>>>()?;
    for (diff, owner) in diffs.iter_mut().zip(owners) {
        diff.commit = Some(owner);
    }
    match (&source, workdir) {
        (DiffSource::Commit(id), _) => add_conflicts(&ctx, *id, scope_lines, &mut diffs),
        (DiffSource::Worktree | DiffSource::Linked(_), Some(workdir)) => {
            add_marker_conflicts(&workdir, &mut diffs)
        }
        _ => {}
    }
    tracing::debug!(elapsed = ?started.elapsed(), ?source, "loaded diff");
    Ok(diffs)
}

/// Give `diffs` the conflicts of `commit`, when it's conflicted: on the files its diff has,
/// and as files of their own where the diff has nothing else of them. Conflicted files go
/// first, as they're what the commit needs.
fn add_conflicts(
    ctx: &but_ctx::Context,
    commit: CommitId,
    scope_lines: Option<usize>,
    diffs: &mut Vec<FileDiff>,
) {
    let found = match but_api::resolve::commit_conflicts(ctx, commit.0) {
        Ok(found) => found,
        Err(err) => return tracing::warn!("couldn't read {commit:?}'s conflicts: {err:#}"),
    };
    for file in &found.files {
        let ix = row(diffs, &file.path, scope_lines, || {
            Some(file.change.clone().into())
        });
        if let Some(diff) = ix.and_then(|ix| diffs.get_mut(ix)) {
            set_conflicts(diff, conflicts_of(file));
        }
    }
    for manual in found.manual {
        // A deletion that conflicts keeps the file, so the commit's tree may not change it.
        let change = || {
            (commit_side(ctx, commit, &manual.path))
                .inspect_err(|err| tracing::warn!("couldn't read {}: {err:#}", manual.path))
                .ok()
                .flatten()
        };
        let ix = row(diffs, &manual.path, scope_lines, change);
        if let Some(diff) = ix.and_then(|ix| diffs.get_mut(ix)) {
            diff.file.conflicted = true;
            // The reason is a sentence of its own; the command names the commit, as the
            // branch it's on may hold several.
            diff.body = DiffBody::Unresolvable(format!(
                "{}. Resolve it in a terminal, with `but resolve {}`.",
                manual.reason.trim_end_matches('.'),
                commit.short()
            ));
        }
    }
    diffs.sort_by_key(|diff| !diff.file.conflicted);
}

/// Where `path` is in `diffs`, added as `change` when the diff has nothing of it.
fn row(
    diffs: &mut Vec<FileDiff>,
    path: &str,
    scope_lines: Option<usize>,
    change: impl FnOnce() -> Option<but_core::TreeChange>,
) -> Option<usize> {
    if let Some(ix) = diffs.iter().position(|diff| diff.file.path == path) {
        return Some(ix);
    }
    diffs.push(FileDiff {
        file: file_change(change()?),
        body: DiffBody::Empty,
        fingerprint: 0,
        annotated: true,
        scope_lines,
        drawing: None,
        commit: None,
    });
    Some(diffs.len() - 1)
}

/// What conflicted `commit` does to `path`: from its parent's version to its own side's, as
/// its other conflicted files show, which also gives a rename's new path a row.
fn commit_side(
    ctx: &but_ctx::Context,
    commit: CommitId,
    path: &str,
) -> Result<Option<but_core::TreeChange>> {
    use but_core::{ChangeState, TreeStatus, commit::TreeKind};
    use gix::prelude::ObjectIdExt as _;
    let repo = ctx.repo.get()?;
    let commit = but_core::Commit::from_id(commit.0.attach(&repo))?;
    let state = |kind| -> Result<Option<ChangeState>> {
        let tree = repo.find_tree(commit.tree_id_or_kind(kind)?)?;
        Ok(tree.lookup_entry_by_path(path)?.map(|entry| ChangeState {
            id: entry.object_id(),
            kind: entry.mode().kind(),
        }))
    };
    let status = match (state(TreeKind::Ours)?, state(TreeKind::Theirs)?) {
        (None, Some(state)) => TreeStatus::Addition {
            state,
            is_untracked: false,
        },
        (Some(previous_state), None) => TreeStatus::Deletion { previous_state },
        (Some(previous_state), Some(state)) => TreeStatus::Modification {
            previous_state,
            state,
            flags: None,
        },
        (None, None) => return Ok(None),
    };
    Ok(Some(but_core::TreeChange {
        path: path.into(),
        status,
    }))
}

/// Give the conflicted files among the uncommitted `diffs` their conflicts, read from the
/// markers in each file under `workdir`. The conflicts stand in for the file's diff, which
/// would only show the markers as added lines.
fn add_marker_conflicts(workdir: &Path, diffs: &mut [FileDiff]) {
    for diff in diffs.iter_mut().filter(|diff| diff.file.conflicted) {
        let Ok(path) = gix::path::from_bstr(diff.file.change.path.as_bstr()) else {
            continue;
        };
        let Ok(text) = std::fs::read(workdir.join(path)) else {
            continue;
        };
        let conflicts = marker_conflicts(&diff.file.path, &text);
        if !conflicts.is_empty() {
            diff.body = DiffBody::Empty;
            set_conflicts(diff, conflicts);
        }
    }
}

/// Mark `diff` conflicted with `conflicts`, among its hunks if it has any.
fn set_conflicts(diff: &mut FileDiff, conflicts: Vec<Conflict>) {
    diff.file.conflicted = true;
    let lines = (conflicts.iter()).flat_map(|c| [&c.new_base, &c.commit, &c.before, &c.after]);
    let conflict_columns = (lines.flatten())
        .map(|line| columns(&line.text))
        .max()
        .unwrap_or(0);
    match &mut diff.body {
        DiffBody::Hunks {
            widest,
            conflicts: slot,
            ..
        } => {
            *widest = widest.map(|side| side.max(conflict_columns));
            *slot = conflicts;
        }
        body => {
            *body = DiffBody::Hunks {
                hunks: Vec::new(),
                added: 0,
                removed: 0,
                widest: [conflict_columns; 2],
                conflicts,
            }
        }
    }
}

/// The conflicts a merge left in `text`, the file at `path`, numbered by its lines: each
/// side from the line its markers start at, with up to three lines around it.
fn marker_conflicts(path: &str, text: &[u8]) -> Vec<Conflict> {
    let lines = markers::lines(text);
    let blocks = markers::blocks(&lines);
    let ids = markers::ids(&lines, &blocks);
    let numbered = |range: std::ops::Range<usize>, from: usize| -> Vec<DiffLine> {
        (lines
            .get(range)
            .unwrap_or_default()
            .iter()
            .zip(from as u32..))
        .map(|(raw, new)| {
            context_line(
                new,
                &display_text(raw.to_str_lossy().trim_end_matches('\n')),
            )
        })
        .collect()
    };
    (blocks.iter().enumerate().zip(ids))
        .map(|((index, block), id)| {
            let (start, end) = (block.lines.start, block.lines.end);
            let previous =
                (index.checked_sub(1).and_then(|ix| blocks.get(ix))).map_or(0, |b| b.lines.end);
            let next = blocks.get(index + 1).map_or(lines.len(), |b| b.lines.start);
            let before = start.saturating_sub(3).max(previous)..start;
            let after = end..(end + 3).min(next);
            let mut conflict = Conflict {
                id,
                index,
                at: start as u32 + 1,
                before: numbered(before.clone(), before.start + 1),
                new_base: numbered(block.ours.clone(), start + 1),
                commit: numbered(block.theirs.clone(), start + 1),
                ancestor: (block.ancestor.clone()).map(|range| numbered(range, start + 1)),
                after: numbered(after.clone(), after.start + 1),
                new_base_text: markers::text(&lines, block.ours.clone()),
                commit_text: markers::text(&lines, block.theirs.clone()),
                labels: block.labels.clone(),
            };
            annotate_conflict(path, &mut conflict);
            conflict
        })
        .collect()
}

/// A file's conflicts, numbered as its diff numbers the new side. The diff is of the commit
/// with every conflict left as its new base has it, so each conflict starts as far before
/// where the commit's own lines would put it as the earlier conflicts' lines differ.
fn conflicts_of(file: &but_api::resolve::ConflictedFile) -> Vec<Conflict> {
    let count = |text: &str| text.lines().count() as u32;
    let lines = |text: &str, from: u32| -> Vec<DiffLine> {
        (text.lines().zip(from..))
            .map(|(raw, new)| context_line(new, &display_text(raw)))
            .collect()
    };
    let mut shift = 0i64;
    (file.hunks.iter().enumerate())
        .map(|(index, hunk)| {
            let at = u32::try_from(i64::from(hunk.line) - shift)
                .unwrap_or(1)
                .max(1);
            shift += i64::from(count(&hunk.theirs)) - i64::from(count(&hunk.ours));
            let mut conflict = Conflict {
                id: hunk.id.clone(),
                index,
                at,
                before: lines(
                    &hunk.context_before,
                    at.saturating_sub(count(&hunk.context_before)),
                ),
                new_base: lines(&hunk.ours, at),
                commit: lines(&hunk.theirs, at),
                ancestor: hunk.base.as_deref().map(|base| lines(base, at)),
                after: lines(&hunk.context_after, at + count(&hunk.ours)),
                new_base_text: hunk.ours.as_str().into(),
                commit_text: hunk.theirs.as_str().into(),
                labels: Default::default(),
            };
            annotate_conflict(&file.path, &mut conflict);
            conflict
        })
        .collect()
}

/// Highlight a conflict's lines, each side reading on from the lines before it, and mark
/// the words the two sides differ in.
fn annotate_conflict(path: &str, conflict: &mut Conflict) {
    let Conflict {
        before,
        new_base,
        commit,
        ancestor,
        after,
        ..
    } = conflict;
    let highlight = |lead: &[&[DiffLine]], lines: &mut [DiffLine]| {
        let Some(mut highlighter) = highlight::Highlighter::for_path(path) else {
            return;
        };
        for line in lead.iter().flat_map(|lines| lines.iter()) {
            highlighter.line(&line.text);
        }
        for line in lines {
            line.spans = highlighter.line(&line.text);
        }
    };
    highlight(&[], before);
    highlight(&[before], new_base);
    highlight(&[before], commit);
    if let Some(ancestor) = ancestor {
        highlight(&[before], ancestor);
    }
    highlight(&[before, new_base], after);
    // The sides as a removal and an addition, which marks the words that differ.
    let mut pair: Vec<DiffLine> = (new_base.iter().map(|line| (line, LineKind::Removed)))
        .chain(commit.iter().map(|line| (line, LineKind::Added)))
        .map(|(line, kind)| DiffLine {
            kind,
            ..line.clone()
        })
        .collect();
    annotate::emphasize_run(&mut pair);
    let (base_emph, commit_emph) = pair.split_at(new_base.len());
    for (line, marked) in new_base.iter_mut().zip(base_emph) {
        line.emph = marked.emph.clone();
    }
    for (line, marked) in commit.iter_mut().zip(commit_emph) {
        line.emph = marked.emph.clone();
    }
}

/// One file's diff, reused from `previous` when its patch is the same, else parsed and,
/// with `annotate`, highlighted and given scope names while it changes no more lines than
/// that, and with `draw_svgs`, drawn when it's an SVG.
fn load_file(
    repo: &gix::Repository,
    file: FileChange,
    context_lines: u32,
    previous: Option<&FileDiff>,
    annotate: Option<u32>,
    scope_lines: Option<usize>,
    draw_svgs: bool,
) -> Result<FileDiff> {
    // Annotated as this load would, its scopes no wider or narrower.
    let up_to_date = |old: &FileDiff| old.annotated && old.scope_lines == scope_lines;
    let reuse = |old: &FileDiff, file| FileDiff {
        file: FileChange {
            converted: old.file.converted,
            ..file
        },
        body: old.body.clone(),
        fingerprint: old.fingerprint,
        annotated: up_to_date(old),
        scope_lines,
        drawing: old.drawing.clone(),
        commit: None,
    };
    // An uncommitted file last written at the same moment, to the same size, against the
    // same base, has the same patch, so it isn't computed again, much as git skips files
    // whose stat is unchanged.
    if let Some(old) = previous.filter(|old| {
        file.modified.is_some()
            && (old.file.modified, old.file.size) == (file.modified, file.size)
            && old.file.kind == file.kind
            && old.file.previous_path == file.previous_path
            && old.file.base() == file.base()
            && (up_to_date(old) || annotate.is_none())
            // Conflicts replace the diff, which a file resolved since needs back.
            && !old.file.conflicted
    }) {
        return Ok(reuse(old, file));
    }
    let mut filter = but_core::unified_diff::filter_from_state(
        repo,
        file.change.status.state(),
        but_core::UnifiedPatch::CONVERSION_MODE,
    )?;
    let patch = file
        .change
        .unified_patch_with_filter(repo, context_lines, &mut filter)?;
    let fingerprint = fingerprint(&file, &patch);
    let converted = matches!(
        patch,
        Some(but_core::UnifiedPatch::Patch {
            is_result_of_binary_to_text_conversion: true,
            ..
        })
    );
    if let Some(old) = (previous.filter(|old| old.fingerprint == fingerprint && up_to_date(old)))
        .filter(|old| !old.file.conflicted)
    {
        return Ok(reuse(old, file));
    }
    let mut body = match patch {
        None => DiffBody::Empty,
        // Images are decoded with the highlighting, so a long diff's first files show sooner.
        Some(but_core::UnifiedPatch::Binary) if annotate.is_some() => {
            image_body(repo, &file).unwrap_or(DiffBody::Binary)
        }
        Some(but_core::UnifiedPatch::Binary) => DiffBody::Binary,
        Some(but_core::UnifiedPatch::TooLarge { size_in_bytes }) => DiffBody::TooLarge {
            bytes: size_in_bytes,
        },
        Some(but_core::UnifiedPatch::Patch {
            hunks,
            lines_added,
            lines_removed,
            ..
        }) => {
            let hunks: Vec<Hunk> = hunks.iter().map(parse_hunk).collect();
            DiffBody::Hunks {
                widest: widest(&hunks),
                hunks,
                added: lines_added,
                removed: lines_removed,
                conflicts: Vec::new(),
            }
        }
    };
    let lines = match &body {
        DiffBody::Hunks { added, removed, .. } => added + removed,
        _ => 0,
    };
    if let Some(limit) = annotate
        && let DiffBody::Hunks { hunks, widest, .. } = &mut body
        && lines <= limit
    {
        // First, so the lines a scope adds around a hunk are highlighted with it, in order.
        if let Some(scope_lines) = scope_lines
            && let Some((old, new)) = scope_sides(&filter)
        {
            annotate::label_scopes(hunks, &old, &new, &file.path, scope_lines);
            *widest = self::widest(hunks);
        }
        for hunk in hunks.iter_mut() {
            annotate::emphasize(hunk);
            let Hunk {
                lead, lines, tail, ..
            } = hunk;
            highlight_lines(&file.path, lead.iter_mut().chain(lines).chain(tail));
        }
    }
    // Only lines and images are filled in later, a file the first files' limit left plain
    // among them; other bodies are as they'll stay.
    let annotated = annotate.is_some_and(|limit| lines <= limit || limit >= HIGHLIGHT_LIMIT)
        || !matches!(body, DiffBody::Hunks { .. } | DiffBody::Binary);
    // An SVG is drawn as images are decoded, with the highlighting.
    let drawable = matches!(body, DiffBody::Hunks { .. } | DiffBody::Binary);
    let drawing = (draw_svgs && annotate.is_some() && drawable)
        .then(|| svg_drawing(repo, &file))
        .flatten();
    Ok(FileDiff {
        file: FileChange { converted, ..file },
        body,
        fingerprint,
        annotated,
        scope_lines,
        drawing,
        commit: None,
    })
}

/// `bytes` turned into an image by `decode`, once for the images seen last: one keeps its
/// id, so the GPU texture gpui makes for it the first time it's drawn serves later diffs
/// too. Decoded here rather than by gpui as it's drawn: an image gpui is still decoding
/// stays blank in the cached diff until something else redraws it.
fn decoded(bytes: &[u8], decode: impl FnOnce(&[u8]) -> Option<Picture>) -> Option<Picture> {
    use std::hash::{Hash as _, Hasher as _};
    use std::sync::{LazyLock, Mutex};
    // About this many bytes of pixels, the oldest going first; one image can be 2.8 MB.
    const KEPT_BYTES: usize = 32 << 20;
    type Kept = std::collections::VecDeque<(u64, Picture)>;
    static DECODED: LazyLock<Mutex<Kept>> = LazyLock::new(Default::default);
    let mut hasher = std::hash::DefaultHasher::new();
    bytes.hash(&mut hasher);
    let key = hasher.finish();
    if let Some((_, found)) = DECODED.lock().ok()?.iter().find(|(k, _)| *k == key) {
        return Some(found.clone());
    }
    let render = decode(bytes)?;
    let mut decoded = DECODED.lock().ok()?;
    decoded.push_back((key, render.clone()));
    let bytes = |kept: &Kept| -> usize {
        let sizes = kept.iter().map(|(_, picture)| picture.image.size(0));
        sizes
            .map(|size| (size.width.0 * size.height.0 * 4) as usize)
            .sum()
    };
    while decoded.len() > 1 && bytes(&decoded) > KEPT_BYTES {
        decoded.pop_front();
    }
    Some(render)
}

/// `bytes` decoded as `format` and shrunk to the most a diff shows of it.
fn raster(format: image::ImageFormat, bytes: &[u8]) -> Option<Picture> {
    let mut image = image::load_from_memory_with_format(bytes, format).ok()?;
    // A diff draws an image at most 220 points tall, 440 pixels on a Retina screen, and half
    // the pane wide; more pixels would only cost memory.
    if image.width() > 1600 || image.height() > 440 {
        image = image.thumbnail(1600, 440);
    }
    let mut buffer = image.into_rgba8();
    // gpui draws BGRA.
    for pixel in buffer.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    // gpui draws a decoded image a point a pixel.
    let (width, height) = (buffer.width() as f32, buffer.height() as f32);
    let image = Arc::new(gpui::RenderImage::new([image::Frame::new(buffer)]));
    Some(Picture {
        image,
        width,
        height,
    })
}

/// `bytes` drawn as gpui draws an SVG, sharp on a Retina screen. It keeps its own size
/// within what a diff shows of an image, 800 points wide and 220 tall, but grows to 64
/// points across at least: an icon is the usual SVG, and too small to compare as it is.
fn draw_svg(bytes: &[u8]) -> Option<Picture> {
    let renderer = gpui::SvgRenderer::new(Arc::new(()));
    let svg = renderer.parse_svg(bytes).ok()?;
    // gpui doesn't say how big an SVG is, but drawing it at a sixteenth does: at twice that
    // for the screen, each pixel stands for eight points of it. One too small to draw so
    // counts as eight points.
    let (width, height) = match renderer.render_parsed(&svg, 1. / 16.) {
        Ok(probe) => {
            let size = probe.size(0);
            let points = |pixels: gpui::DevicePixels| i32::from(pixels).max(1) as f32 * 8.;
            (points(size.width), points(size.height))
        }
        Err(_) => (8., 8.),
    };
    let scale = (64. / width.max(height))
        .max(1.)
        .min(800. / width)
        .min(220. / height);
    let image = renderer.render_parsed(&svg, scale).ok()?;
    // Drawn at twice its size in points, for the screen.
    let size = image.size(0);
    Some(Picture {
        width: i32::from(size.width) as f32 / 2.,
        height: i32::from(size.height) as f32 / 2.,
        image,
    })
}

/// SVGs bigger than this show as text, rather than spend the time drawing them.
const SVG_LIMIT: usize = 1_000_000;

/// The most SVGs one diff draws; with more, all show as text.
const SVGS_DRAWN: usize = 24;

fn is_svg(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".svg")
}

/// Both sides of `file` drawn, when it's an SVG that draws.
fn svg_drawing(repo: &gix::Repository, file: &FileChange) -> Option<super::Drawing> {
    if !is_svg(&file.path) {
        return None;
    }
    let (before, after) = both_sides(file, |state, path| {
        let data = read_side(repo, state, path).ok()?;
        (data.len() <= SVG_LIMIT)
            .then(|| decoded(&data, draw_svg))
            .flatten()
    })?;
    Some(super::Drawing { before, after })
}

/// What `side` makes of each of `file`'s sides, the old one at its old path: none before
/// for an added file, none after for a deleted one, and `None` when a side that's there
/// makes nothing.
fn both_sides<T>(
    file: &FileChange,
    side: impl Fn(but_core::ChangeState, &BStr) -> Option<T>,
) -> Option<(Option<T>, Option<T>)> {
    let (status, path) = (&file.change.status, file.change.path.as_bstr());
    let before = match status.previous_state_and_path() {
        Some((state, old_path)) => Some(side(state, old_path.unwrap_or(path))?),
        None => None,
    };
    let after = match status.state() {
        Some(state) => Some(side(state, path)?),
        None => None,
    };
    Some((before, after))
}

/// Images bigger than this show as a binary file, rather than spend the memory.
const IMAGE_LIMIT: usize = 20_000_000;

/// Both sides of an image file, when `file` is one that can be drawn. A side that's there
/// but can't be decoded makes it a binary file, rather than one added or deleted.
fn image_body(repo: &gix::Repository, file: &FileChange) -> Option<DiffBody> {
    use image::ImageFormat as F;
    let extension = file.path.rsplit_once('.')?.1.to_ascii_lowercase();
    let format = match extension.as_str() {
        "png" => F::Png,
        "jpg" | "jpeg" => F::Jpeg,
        "gif" => F::Gif,
        "webp" => F::WebP,
        "bmp" => F::Bmp,
        "tif" | "tiff" => F::Tiff,
        _ => return None,
    };
    let (before, after) = both_sides(file, |state, path| {
        let data = read_side(repo, state, path).ok()?;
        (data.len() <= IMAGE_LIMIT)
            .then(|| decoded(&data, |bytes| raster(format, bytes)))
            .flatten()
    })?;
    Some(DiffBody::Image { before, after })
}

/// The new side of `file` as diff lines, numbered on that side, for showing the unchanged
/// lines around its hunks, with the widest of them in [`columns`]. `None` when it has no new
/// side to show, like a deletion. An uncommitted file is read from the project's linked
/// `worktree` when there is one.
pub fn file_lines(
    path: &Path,
    worktree: Option<&bstr::BStr>,
    file: &FileChange,
) -> Result<Option<(Vec<DiffLine>, u32)>> {
    let Some(state) = file.change.status.state() else {
        return Ok(None);
    };
    let repo = checkout(&open(path)?, worktree)?;
    let bytes = read_side(&repo, state, file.change.path.as_bstr())?;
    if bytes.contains(&0) {
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&bytes);
    let mut highlighter = (text.len() <= WHOLE_HIGHLIGHT_LIMIT)
        .then(|| highlight::Highlighter::for_path(&file.path))
        .flatten();
    let lines: Vec<DiffLine> = text
        .lines()
        .enumerate()
        .map(|(ix, raw)| {
            let text = display_text(raw);
            let spans = highlighter
                .as_mut()
                .map(|h| h.line(&text))
                .unwrap_or_default();
            DiffLine {
                spans,
                ..context_line(ix as u32 + 1, &text)
            }
        })
        .collect();
    let widest = lines.iter().map(|line| columns(&line.text)).max();
    Ok(Some((lines, widest.unwrap_or(0))))
}

/// Files shown whole are highlighted up to this size in bytes.
const WHOLE_HIGHLIGHT_LIMIT: usize = 512 * 1024;

/// The contents of one side of a change at `path`: a side that isn't in the object database
/// is the file in the worktree.
fn read_side(repo: &gix::Repository, state: but_core::ChangeState, path: &BStr) -> Result<Vec<u8>> {
    if state.id.is_null() {
        let workdir = repo.workdir().context("the repository has no worktree")?;
        Ok(std::fs::read(workdir.join(gix::path::from_bstr(path)?))?)
    } else {
        Ok(repo.find_blob(state.id)?.data.clone())
    }
}

/// `f` over `items` on all cores, keeping their order. Each thread gets its own `state`.
fn parallel_map<T: Send, S, R: Send>(
    items: Vec<T>,
    state: impl Fn() -> S + Sync,
    f: impl Fn(&S, usize, T) -> R + Sync,
) -> Vec<R> {
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(items.len());
    let queue = parking_lot::Mutex::new(items.into_iter().enumerate());
    let mut results: Vec<(usize, R)> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let state = state();
                    let mut done = Vec::new();
                    loop {
                        // Taken out first, so the lock is free while `f` runs.
                        let next = queue.lock().next();
                        let Some((index, item)) = next else {
                            break done;
                        };
                        done.push((index, f(&state, index, item)));
                    }
                })
            })
            .collect();
        // A panic in `f` goes on to the caller, whose task makes it an error.
        (workers.into_iter())
            .flat_map(|worker| {
                worker
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    });
    results.sort_unstable_by_key(|(index, _)| *index);
    results.into_iter().map(|(_, result)| result).collect()
}

/// A hash of a file's patch, hunk headers included, so two equal ones have the same lines.
fn fingerprint(file: &FileChange, patch: &Option<but_core::UnifiedPatch>) -> u64 {
    use std::hash::{Hash as _, Hasher as _};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    // Which blobs it's between, so a rebase that keeps the hunks but changes the file around
    // them reads it again; a worktree file has no blob, and goes by its hunks.
    (
        file.base(),
        file.change.status.state().map(|state| state.id),
    )
        .hash(&mut hasher);
    // Bytes that aren't diffed, as an image's, go by when they were written too.
    if !matches!(patch, Some(but_core::UnifiedPatch::Patch { .. })) {
        file.modified.hash(&mut hasher);
    }
    match patch {
        Some(but_core::UnifiedPatch::Patch { hunks, .. }) => {
            hunks.iter().for_each(|hunk| hunk.diff.hash(&mut hasher))
        }
        Some(but_core::UnifiedPatch::TooLarge { size_in_bytes }) => size_in_bytes.hash(&mut hasher),
        Some(but_core::UnifiedPatch::Binary) => "binary".hash(&mut hasher),
        None => "empty".hash(&mut hasher),
    }
    hasher.finish()
}

/// Files bigger than this, both sides together, aren't parsed for scope names: that
/// parses all of both sides, however small the change.
const SCOPE_LIMIT: usize = 1 << 20;

/// Both sides of a text change the patch was computed from, if small enough to parse.
fn scope_sides(filter: &gix::diff::blob::Platform) -> Option<(String, String)> {
    let (old, new) = filter.resources()?;
    let (old, new) = (side_bytes(old.data)?, side_bytes(new.data)?);
    (old.len() + new.len() <= SCOPE_LIMIT).then(|| {
        let text = |bytes| String::from_utf8_lossy(bytes).into_owned();
        (text(old), text(new))
    })
}

/// One side of a change: empty when it doesn't exist, `None` when binary.
fn side_bytes(data: gix::diff::blob::platform::resource::Data<'_>) -> Option<&[u8]> {
    use gix::diff::blob::platform::resource::Data;
    match data {
        Data::Missing => Some(&[]),
        Data::Buffer { buf, .. } => Some(buf),
        Data::Binary { .. } => None,
    }
}

/// The widest line on each side of `hunks`, old then new, in [`columns`].
fn widest(hunks: &[Hunk]) -> [u32; 2] {
    let mut widest = [0, 0];
    let lines =
        (hunks.iter()).flat_map(|hunk| hunk.lead.iter().chain(&hunk.lines).chain(&hunk.tail));
    for line in lines {
        let columns = columns(&line.text);
        if line.kind != LineKind::Added {
            widest[0] = widest[0].max(columns);
        }
        if line.kind != LineKind::Removed {
            widest[1] = widest[1].max(columns);
        }
    }
    widest
}

/// Files with more changed lines than this aren't highlighted, to keep big diffs fast.
const HIGHLIGHT_LIMIT: u32 = 4000;

/// Among the files that show first, those with more changed lines than this are highlighted
/// with the rest, a moment later, so a long one doesn't hold up the diff. TypeScript takes
/// about 80µs a line to highlight.
const FIRST_HIGHLIGHT_LIMIT: u32 = 500;

/// Lines longer than this, as in minified files, are cut short: shaping and highlighting
/// them costs far more than anyone reads.
const LINE_LIMIT: usize = 2000;

/// An unchanged line numbered `new` on the new side only, showing `text` as it is, so pass
/// it through [`display_text`] first.
pub(super) fn context_line(new: u32, text: &str) -> DiffLine {
    DiffLine::new(LineKind::Context, None, Some(new), text)
}

/// A diff line as shown: tabs as spaces, no carriage return, and cut at [`LINE_LIMIT`].
pub(super) fn display_text(raw: &str) -> String {
    cut(raw.trim_end_matches('\r').replace('\t', "    "), 0)
}

/// `text` from byte `from` on, cut at [`LINE_LIMIT`], each cut marked with an ellipsis.
fn cut(text: String, from: usize) -> String {
    let mut text = match text.get(text.floor_char_boundary(from)..) {
        Some(rest) if from > 0 => format!("…{rest}"),
        _ => text,
    };
    if text.len() > LINE_LIMIT {
        text.truncate(text.floor_char_boundary(LINE_LIMIT));
        text.push('…');
    }
    text
}

/// Where to cut each of `lines` from: a removed line and the added one in its place that
/// first differ past [`LINE_LIMIT`], deep in a minified file or a paragraph, from a little
/// before that, so the change shows rather than two sides that look the same.
fn cut_starts(lines: &[(LineKind, String)]) -> Vec<usize> {
    let mut starts = vec![0; lines.len()];
    let mut at = 0;
    while at < lines.len() {
        let kind_at = |ix: usize| lines.get(ix).map(|(kind, _)| *kind);
        let removed = (at..)
            .take_while(|ix| kind_at(*ix) == Some(LineKind::Removed))
            .count();
        let added = (at + removed..)
            .take_while(|ix| kind_at(*ix) == Some(LineKind::Added))
            .count();
        for pair in 0..removed.min(added) {
            let (old, new) = (at + pair, at + removed + pair);
            let (Some((_, a)), Some((_, b))) = (lines.get(old), lines.get(new)) else {
                continue;
            };
            let same = (a.bytes().zip(b.bytes()))
                .take_while(|(x, y)| x == y)
                .count();
            if a.len().max(b.len()) > LINE_LIMIT && same > LINE_LIMIT - LINE_LIMIT / 4 {
                let from = same - LINE_LIMIT / 4;
                for ix in [old, new] {
                    if let Some(start) = starts.get_mut(ix) {
                        *start = from;
                    }
                }
            }
        }
        at += (removed + added).max(1);
    }
    starts
}

/// Highlight `lines`, consecutive lines of a hunk of `path`.
pub(super) fn highlight_lines<'a>(path: &str, lines: impl Iterator<Item = &'a mut DiffLine>) {
    // One per side, so a removed line's state, as a string it opens, doesn't run on into
    // the added lines: unchanged lines go through both.
    let for_path = || highlight::Highlighter::for_path(path);
    let (Some(mut old), Some(mut new)) = (for_path(), for_path()) else {
        return;
    };
    for line in lines {
        line.spans = match line.kind {
            LineKind::Removed => old.line(&line.text),
            LineKind::Added => new.line(&line.text),
            LineKind::Context => {
                old.line(&line.text);
                new.line(&line.text)
            }
        };
    }
}

pub(super) fn parse_hunk(hunk: &but_core::unified_diff::DiffHunk) -> Hunk {
    let mut old = hunk.old_start;
    let mut new = hunk.new_start;
    let mut lines = Vec::new();
    let mut texts = Vec::new();
    let mut header = String::new();
    for raw in hunk.diff.lines() {
        let text = raw.to_str_lossy();
        let (kind, rest) = match raw.first() {
            Some(b'@') => {
                header = text.into_owned();
                continue;
            }
            Some(b'+') => (LineKind::Added, text.get(1..).unwrap_or_default()),
            Some(b'-') => (LineKind::Removed, text.get(1..).unwrap_or_default()),
            Some(b' ') => (LineKind::Context, text.get(1..).unwrap_or_default()),
            // "\ No newline at end of file"
            Some(b'\\') => continue,
            _ => (LineKind::Context, &*text),
        };
        let (old_no, new_no) = match kind {
            LineKind::Context => (Some(old), Some(new)),
            LineKind::Added => (None, Some(new)),
            LineKind::Removed => (Some(old), None),
        };
        old += u32::from(old_no.is_some());
        new += u32::from(new_no.is_some());
        lines.push((old_no, new_no));
        texts.push((kind, rest.trim_end_matches('\r').replace('\t', "    ")));
    }
    let starts = cut_starts(&texts);
    let lines = (lines.into_iter().zip(texts).zip(starts))
        .map(|(((old_no, new_no), (kind, text)), from)| {
            DiffLine::new(kind, old_no, new_no, &cut(text, from))
        })
        .collect();
    Hunk {
        key: HunkKey(hunk.into()),
        header,
        lines,
        scope: Vec::new(),
        lead: Vec::new(),
        tail: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    #[test]
    fn a_change_deep_in_a_long_line_shows_on_both_sides() {
        use crate::backend::LineKind;
        let paragraph = "word ".repeat(1000);
        let lines = [
            (LineKind::Context, "# Title".to_owned()),
            (LineKind::Removed, format!("{paragraph}Tokyo court")),
            (LineKind::Added, format!("{paragraph}Osaka court")),
            (LineKind::Removed, format!("Tokyo {paragraph}")),
            (LineKind::Added, format!("Osaka {paragraph}")),
        ];
        let starts = super::cut_starts(&lines);
        let shown = |ix: usize| super::cut(lines[ix].1.clone(), starts[ix]);
        assert!(
            shown(1).starts_with('…') && shown(1).ends_with("Tokyo court"),
            "cut from before where the two sides differ, past 2000 bytes: {}",
            shown(1)
        );
        assert!(
            shown(2).ends_with("Osaka court"),
            "its pair is cut the same"
        );
        assert!(
            shown(3).starts_with("Tokyo") && shown(3).ends_with('…'),
            "a change the head shows keeps the head"
        );
        let fits = [
            (LineKind::Removed, format!("{}Tokyo", "word ".repeat(340))),
            (LineKind::Added, format!("{}Osaka", "word ".repeat(340))),
        ];
        assert_eq!(
            super::cut_starts(&fits),
            [0, 0],
            "lines that fit whole are shown whole, however late they differ"
        );
    }

    use crate::backend::ops::tests::{git, head, repo_with};

    #[test]
    fn blame_names_the_last_commit_of_each_line_across_a_rename() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\ntwo\nthree\n");
            let path = dir.path();
            let first = head(path);
            git(path, &["mv", "file", "moved"]);
            git(path, &["commit", "-q", "-m", "rename"]);
            std::fs::write(path.join("moved"), "one\nTWO\nthree\n").unwrap();
            git(path, &["commit", "-q", "-am", "edit"]);
            let edit = head(path);

            let blame =
                |lines: &[u32]| super::blame_lines(path, edit, "moved".into(), lines).unwrap();
            assert_eq!(
                blame(&[2]),
                HashMap::from([(edit, "moved".to_owned())]),
                "line 2 counts from 1, the line the edit changed"
            );
            assert_eq!(
                blame(&[1, 3]),
                HashMap::from([(first, "file".to_owned())]),
                "the rename wrote no lines, so the others go back to the commit before it, \
                 where the file had its old name"
            );
        });
    }
}
