//! Build the conflict-resolution request for a conflicted commit, entirely in memory.
//!
//! The conflicted commit stores its merge inputs as trees (`.conflict-base-0`,
//! `.conflict-side-0/1`), so re-merging them reproduces the conflicts without a
//! checkout: the merged blobs carry standard conflict markers which are parsed
//! into per-hunk ours/base/theirs sections here.

use anyhow::{Context as _, bail};
use bstr::{BString, ByteSlice};
use but_core::RepositoryExt as _;
use but_error::Code;

/// The maximum size of a conflicted file to send to the model.
const MAX_FILE_SIZE: usize = 1024 * 1024;
/// How many lines of surrounding context to include per conflict hunk.
const CONTEXT_LINES: usize = 3;

/// Sentinel side labels passed to the merge so that machine-generated marker
/// lines are exactly known strings. The scanner matches marker lines exactly,
/// so file content that merely looks like a conflict marker (patches, test
/// fixtures, docs) can never open or close a block.
const OURS_MARKER: &str = "<<<<<<< gitbutler-resolve-ours";
const BASE_MARKER: &str = "||||||| gitbutler-resolve-base";
const THEIRS_MARKER: &str = ">>>>>>> gitbutler-resolve-theirs";
const SEPARATOR: &str = "=======";

fn merge_labels() -> gix::merge::blob::builtin_driver::text::Labels<'static> {
    gix::merge::blob::builtin_driver::text::Labels {
        ancestor: Some("gitbutler-resolve-base".into()),
        current: Some("gitbutler-resolve-ours".into()),
        other: Some("gitbutler-resolve-theirs".into()),
    }
}

/// One conflicted region of a file, with the content of each side and a few
/// lines of surrounding context.
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ConflictHunk {
    /// Survives the rewrites resolving causes, unlike `hunks` positions: a
    /// hash of the three sides — which narrowing keeps, unlike contexts —
    /// plus an occurrence counter. Treat an id that stops matching as dropped.
    pub id: String,
    /// The 1-based line where the conflicted region starts, counted in the
    /// intended result — the merge with every conflict taking the commit's
    /// side, the new side of [`ConflictedFile::change`] — so anchors land in
    /// the diff the caller renders.
    ///
    /// [`ConflictedFile::change`]: super::ConflictedFile::change
    pub line: u32,
    /// Unconflicted lines directly before the conflict, clamped to the previous conflict.
    pub context_before: String,
    /// The content of the *ours* side, i.e. the new base the commit is rebased onto.
    pub ours: String,
    /// The content of the common ancestor, if the merge produced diff3-style markers.
    pub base: Option<String>,
    /// The content of the *theirs* side, i.e. the conflicted commit's own version.
    pub theirs: String,
    /// Unconflicted lines directly after the conflict, clamped to the next conflict.
    pub context_after: String,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ConflictHunk);

/// A conflicted file together with its merged marker text and extracted hunks.
#[derive(Debug, Clone)]
pub struct FileConflict {
    /// The repo-relative path, lossily decoded for display and prompting.
    pub path: String,
    /// The exact repo-relative path as stored in the tree.
    pub rela_path: BString,
    /// The kind of the merged tree entry, preserved when writing the resolved blob.
    pub entry_kind: gix::objs::tree::EntryKind,
    /// The full content of the merged blob, still containing conflict markers.
    pub merged_text: String,
    /// The conflicts found in `merged_text`, in file order.
    pub hunks: Vec<ConflictHunk>,
}

/// Everything needed to prompt for and apply a resolution of one conflicted commit.
#[derive(Debug, Clone)]
pub struct ResolutionRequest {
    /// The conflicted commit to resolve.
    pub commit_id: gix::ObjectId,
    /// The commit's message with the conflict markers stripped.
    pub commit_message: String,
    /// The title of the commit's parent, i.e. the new base it was rebased onto.
    pub parent_message: Option<String>,
    /// The commit's stored merge-base tree.
    pub base_tree_id: gix::ObjectId,
    /// The commit's stored *ours* tree, i.e. the new base it was rebased onto.
    pub ours_tree_id: gix::ObjectId,
    /// The commit's stored *theirs* tree, i.e. its own version.
    pub theirs_tree_id: gix::ObjectId,
    /// All conflicted files that decompose into hunks, sorted by path.
    pub files: Vec<FileConflict>,
    /// Conflicted files that have no hunk representation, sorted by path.
    /// They cannot be addressed through this API at all, so a commit is only
    /// fully resolved once this is empty.
    pub manual: Vec<ManualConflict>,
}

/// A conflicted file with no hunk representation — a side deletion or rename, a
/// non-blob entry, a binary, or one too large to splice — which therefore needs
/// manual resolution in edit mode.
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ManualConflict {
    /// The repo-relative path of the file.
    pub path: String,
    /// Why it cannot be resolved automatically, for display to the user.
    pub reason: String,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ManualConflict);

/// Re-merge the conflict trees of `commit_id` and extract all conflict hunks.
///
/// Conflicts with no marker block to splice a resolution into — side deletions,
/// non-blob entries, binary or oversized files — are reported in `manual`
/// rather than failing the request, so the rest of the commit stays workable.
pub fn build_request(
    repo: &gix::Repository,
    commit_id: gix::ObjectId,
) -> anyhow::Result<ResolutionRequest> {
    use gix::prelude::ObjectIdExt as _;

    let commit = but_core::Commit::from_id(commit_id.attach(repo))?;
    let Some((base, ours, theirs)) = commit.conflicted_tree_ids()? else {
        bail!(
            anyhow::anyhow!(Code::Validation)
                .context(format!("Commit {commit_id} is not conflicted"))
        );
    };

    let commit_message = but_core::commit::strip_conflict_markers(commit.message.as_ref())
        .to_str_lossy()
        .into_owned();
    let parent_message = commit
        .parents
        .first()
        .and_then(|parent_id| but_core::Commit::from_id(parent_id.attach(repo)).ok())
        .map(|parent| commit_title(&parent));

    let (base, ours, theirs) = (base.detach(), ours.detach(), theirs.detach());
    let repo = repo.clone().for_tree_diffing()?;
    // Merge without favoring a side to reproduce the actual conflicts, and
    // force diff3-style markers with the sentinel labels so every hunk carries
    // the common ancestor and marker lines are exactly known strings.
    let mut options: gix::merge::plumbing::tree::Options = repo.tree_merge_options()?.into();
    options.blob_merge.text.conflict = gix::merge::blob::builtin_driver::text::Conflict::Keep {
        style: gix::merge::blob::builtin_driver::text::ConflictStyle::Diff3,
        marker_size: 7.try_into().expect("non-zero constant"),
    };
    let mut outcome = repo.merge_trees(base, ours, theirs, merge_labels(), options.into())?;
    let merged_tree_id = outcome.tree.write()?.detach();

    let mut index = repo.index_from_tree(&merged_tree_id)?;
    if !outcome.index_changed_after_applying_conflicts(
        &mut index,
        gix::merge::tree::TreatAsUnresolved::git(),
        gix::merge::tree::apply_index_entries::RemovalMode::Mark,
    ) {
        bail!(
            "Re-merging the conflicting trees of commit {commit_id} yielded no conflicts to resolve"
        );
    }

    let mut sides_by_path: std::collections::BTreeMap<BString, ConflictSides> = Default::default();
    for entry in index.entries() {
        use gix::index::entry::Stage;
        let sides = sides_by_path
            .entry(entry.path(&index).to_owned())
            .or_default();
        match entry.stage() {
            Stage::Unconflicted => continue,
            Stage::Base => sides.base = true,
            Stage::Ours => sides.ours = true,
            Stage::Theirs => sides.theirs = true,
        }
    }
    sides_by_path.retain(|_, sides| sides.base || sides.ours || sides.theirs);

    let merged_tree = repo.find_tree(merged_tree_id)?;
    let mut files = Vec::with_capacity(sides_by_path.len());
    let mut manual = Vec::new();
    // A file with no hunk representation is reported rather than failing the
    // whole commit: the conflicts that *can* be addressed stay addressable, and
    // the commit simply remains conflicted until this one is resolved in edit
    // mode. Failing outright would hide every other conflict behind one binary.
    macro_rules! needs_manual_resolution {
        ($path:expr, $reason:expr) => {{
            manual.push(ManualConflict {
                path: $path,
                reason: $reason,
            });
            continue;
        }};
    }
    for (rela_path, sides) in sides_by_path {
        let path = rela_path.to_str_lossy().into_owned();
        if !(sides.ours && sides.theirs) {
            needs_manual_resolution!(path, "The conflict involves a deletion or rename.".into());
        }
        let entry = merged_tree
            .lookup_entry(rela_path.split(|b| *b == b'/'))?
            .with_context(|| {
                format!("Conflicted path \"{path}\" is missing from the merged tree")
            })?;
        let entry_kind = entry.mode().kind();
        if !matches!(
            entry_kind,
            gix::objs::tree::EntryKind::Blob | gix::objs::tree::EntryKind::BlobExecutable
        ) {
            needs_manual_resolution!(path, "The conflict is not a regular file.".into());
        }
        let blob = entry.object()?.into_blob();
        if blob.data.len() > MAX_FILE_SIZE {
            needs_manual_resolution!(path, "The file exceeds the 1MB size limit.".into());
        }
        let Ok(merged_text) = std::str::from_utf8(&blob.data) else {
            needs_manual_resolution!(path, "The file is binary or not valid UTF-8.".into());
        };
        let merged_text = merged_text.to_owned();
        let lines = split_lines(&merged_text);
        let blocks = scan_conflict_blocks(&lines);
        // Content that merely looks like a conflict marker cannot open or
        // close a block (markers are matched exactly and positionally), but it
        // would confuse the section decomposition or any later marker-based
        // reader of the resolved file, so hand such files to manual resolution.
        if let Some(line) = find_ambiguous_marker_line(&lines, &blocks) {
            needs_manual_resolution!(
                path,
                format!("The file contains content ambiguous with conflict markers ({line:?}).")
            );
        }
        if blocks.is_empty() {
            needs_manual_resolution!(path, "No conflict markers were found in the file.".into());
        }
        let hunks = extract_hunks(&lines, &blocks);
        files.push(FileConflict {
            path,
            rela_path,
            entry_kind,
            merged_text,
            hunks,
        });
    }

    if files.is_empty() && manual.is_empty() {
        bail!("Commit {commit_id} has no conflicted files to resolve");
    }

    Ok(ResolutionRequest {
        commit_id,
        commit_message,
        parent_message,
        base_tree_id: base,
        ours_tree_id: ours,
        theirs_tree_id: theirs,
        files,
        manual,
    })
}

#[derive(Debug, Default)]
struct ConflictSides {
    base: bool,
    ours: bool,
    theirs: bool,
}

fn commit_title(commit: &but_core::Commit<'_>) -> String {
    gix::objs::commit::MessageRef::from_bytes(
        but_core::commit::strip_conflict_markers(commit.message.as_ref()).as_ref(),
    )
    .title
    .trim_ascii_end()
    .to_str_lossy()
    .chars()
    .take(80)
    .collect()
}

/// A well-formed conflict block found in marker text, as line ranges.
#[derive(Debug, Clone)]
pub(crate) struct ConflictBlock {
    /// Line index of the `<<<<<<<` marker.
    pub start: usize,
    /// Line index of the `>>>>>>>` marker.
    pub end: usize,
    /// Content lines of the *ours* section.
    pub ours: std::ops::Range<usize>,
    /// Content lines of the *base* section, present with diff3-style markers.
    pub base: Option<std::ops::Range<usize>>,
    /// Content lines of the *theirs* section.
    pub theirs: std::ops::Range<usize>,
}

/// Whether `line` is shaped like a `<<<<<<<` / `|||||||` / `>>>>>>>` conflict
/// marker of any origin: seven (or more, for nested-merge sizes) marker
/// characters followed by nothing or a space.
///
/// A lone `=======` is deliberately not treated as marker-shaped — it cannot
/// open or close a block by itself and legitimately occurs as e.g. an RST
/// heading underline.
pub(crate) fn is_marker_shaped(line: &str) -> bool {
    let bytes = line.as_bytes();
    let Some(&first) = bytes.first() else {
        return false;
    };
    if !matches!(first, b'<' | b'|' | b'>') {
        return false;
    }
    let run = bytes.iter().take_while(|byte| **byte == first).count();
    run >= 7 && (bytes.len() == run || bytes[run] == b' ')
}

/// The first line of file content that could be mistaken for a conflict
/// marker: a marker-shaped line (including a sentinel-marker lookalike)
/// anywhere but at a block's own marker positions, or a lone `=======` inside
/// a block, where the scanner would bind it as the section separator.
///
/// A lone `=======` outside all blocks is fine — it can only be content.
fn find_ambiguous_marker_line<'a>(lines: &[&'a str], blocks: &[ConflictBlock]) -> Option<&'a str> {
    let marker_positions: std::collections::HashSet<usize> = blocks
        .iter()
        .flat_map(|block| {
            [
                Some(block.start),
                Some(block.end),
                Some(block.theirs.start - 1),
            ]
            .into_iter()
            .chain([block.base.as_ref().map(|base| base.start - 1)])
            .flatten()
        })
        .collect();

    lines.iter().enumerate().find_map(|(index, line)| {
        if marker_positions.contains(&index) {
            return None;
        }
        let inside_a_block = || {
            blocks
                .iter()
                .any(|block| index > block.start && index < block.end)
        };
        (is_marker_shaped(line) || (*line == SEPARATOR && inside_a_block())).then_some(*line)
    })
}

/// Split marker text into lines, treating `\r\n` and `\n` alike.
pub(crate) fn split_lines(text: &str) -> Vec<&str> {
    text.split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect()
}

/// Find all well-formed conflict blocks in `lines`.
///
/// A block requires an ours marker, a separator, and a closing theirs marker;
/// anything else (stray or malformed markers) is treated as plain content.
/// This scanner is shared by hunk extraction and resolution splicing so both
/// always agree on the number and position of conflicts.
pub(crate) fn scan_conflict_blocks(lines: &[&str]) -> Vec<ConflictBlock> {
    let mut blocks = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i] != OURS_MARKER {
            i += 1;
            continue;
        }
        let mut base_start = None;
        let mut separator = None;
        let mut close = None;
        for (j, line) in lines.iter().enumerate().skip(i + 1) {
            if separator.is_none() && base_start.is_none() && *line == BASE_MARKER {
                base_start = Some(j);
            } else if separator.is_none() && *line == SEPARATOR {
                separator = Some(j);
            } else if separator.is_some() && *line == THEIRS_MARKER {
                close = Some(j);
                break;
            }
        }
        let (Some(separator), Some(close)) = (separator, close) else {
            i += 1;
            continue;
        };
        blocks.push(ConflictBlock {
            start: i,
            end: close,
            ours: (i + 1)..base_start.unwrap_or(separator),
            base: base_start.map(|base_start| (base_start + 1)..separator),
            theirs: (separator + 1)..close,
        });
        i = close + 1;
    }
    blocks
}

fn extract_hunks(lines: &[&str], blocks: &[ConflictBlock]) -> Vec<ConflictHunk> {
    let join = |range: std::ops::Range<usize>| lines[range].join("\n");

    // Anchors count lines in the intended result — the merged text with each
    // block replaced by its theirs lines. A block's line there is its
    // merged-text line minus everything earlier blocks contribute beyond
    // their theirs lines.
    let mut removed_before = 0;
    let mut occurrences: std::collections::HashMap<u64, usize> = std::collections::HashMap::new();
    let mut hunks = Vec::with_capacity(blocks.len());
    for (index, block) in blocks.iter().enumerate() {
        let previous_end = if index == 0 {
            0
        } else {
            blocks[index - 1].end + 1
        };
        let next_start = blocks
            .get(index + 1)
            .map(|next| next.start)
            .unwrap_or(lines.len());
        let context_before_start = block.start.saturating_sub(CONTEXT_LINES).max(previous_end);
        let context_after_end = (block.end + 1 + CONTEXT_LINES).min(next_start);

        let ours = join(block.ours.clone());
        let base = block.base.clone().map(join);
        let theirs = join(block.theirs.clone());
        let sides_hash = {
            use std::hash::{Hash as _, Hasher as _};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            ours.hash(&mut hasher);
            base.hash(&mut hasher);
            theirs.hash(&mut hasher);
            hasher.finish()
        };
        let occurrence = occurrences.entry(sides_hash).or_insert(0);
        let id = format!("{sides_hash:016x}-{occurrence}");
        *occurrence += 1;

        hunks.push(ConflictHunk {
            id,
            line: (block.start - removed_before + 1) as u32,
            context_before: join(context_before_start..block.start),
            ours,
            base,
            theirs,
            context_after: join((block.end + 1)..context_after_end),
        });
        removed_before += (block.end - block.start + 1) - block.theirs.len();
    }
    hunks
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO_WAY: &str = "line 1\nline 2\n<<<<<<< gitbutler-resolve-ours\nour change\n=======\ntheir change\n>>>>>>> gitbutler-resolve-theirs\nline 3\n";
    const DIFF3: &str = "start\n<<<<<<< gitbutler-resolve-ours\nours\n||||||| gitbutler-resolve-base\noriginal\n=======\ntheirs\n>>>>>>> gitbutler-resolve-theirs\nend\n";

    fn extract_hunks(text: &str) -> Vec<ConflictHunk> {
        let lines = split_lines(text);
        let blocks = scan_conflict_blocks(&lines);
        super::extract_hunks(&lines, &blocks)
    }

    fn ambiguous_line(text: &str) -> Option<String> {
        let lines = split_lines(text);
        let blocks = scan_conflict_blocks(&lines);
        find_ambiguous_marker_line(&lines, &blocks).map(str::to_owned)
    }

    #[test]
    fn two_way_markers_are_extracted() {
        let hunks = extract_hunks(TWO_WAY);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].ours, "our change");
        assert_eq!(hunks[0].theirs, "their change");
        assert_eq!(hunks[0].base, None);
        assert_eq!(hunks[0].context_before, "line 1\nline 2");
        assert_eq!(hunks[0].context_after, "line 3\n");
    }

    #[test]
    fn diff3_markers_include_base() {
        let hunks = extract_hunks(DIFF3);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].ours, "ours");
        assert_eq!(hunks[0].base.as_deref(), Some("original"));
        assert_eq!(hunks[0].theirs, "theirs");
    }

    #[test]
    fn adjacent_hunks_clamp_context() {
        let text = "a\n<<<<<<< gitbutler-resolve-ours\n1\n=======\n2\n>>>>>>> gitbutler-resolve-theirs\nb\n<<<<<<< gitbutler-resolve-ours\n3\n=======\n4\n>>>>>>> gitbutler-resolve-theirs\nc\n";
        let hunks = extract_hunks(text);
        assert_eq!(hunks.len(), 2);
        assert_eq!(hunks[0].context_after, "b");
        assert_eq!(hunks[1].context_before, "b");
    }

    /// The line anchors count lines in the intended result, i.e. the merged
    /// text with each block replaced by its theirs lines.
    #[test]
    fn hunk_lines_anchor_into_the_intended_result() {
        // Intended result: start, theirs, end — the conflict starts at line 2.
        assert_eq!(extract_hunks(DIFF3)[0].line, 2);
        // Intended result: a, 2, b, 4, c — conflicts at lines 2 and 4.
        let text = "a\n<<<<<<< gitbutler-resolve-ours\n1\n=======\n2\n>>>>>>> gitbutler-resolve-theirs\nb\n<<<<<<< gitbutler-resolve-ours\n3\n=======\n4\n>>>>>>> gitbutler-resolve-theirs\nc\n";
        let hunks = extract_hunks(text);
        assert_eq!(hunks[0].line, 2);
        assert_eq!(hunks[1].line, 4);
    }

    /// The sides can differ in length, which is the only case where anchoring
    /// to the wrong one shows: in ours coordinates the second conflict would
    /// be line 5.
    #[test]
    fn anchors_follow_theirs_when_the_sides_differ_in_length() {
        // Intended result: a, t1, b, t2, c.
        let text = "a\n<<<<<<< gitbutler-resolve-ours\no1\no2\n=======\nt1\n>>>>>>> gitbutler-resolve-theirs\nb\n<<<<<<< gitbutler-resolve-ours\no3\n=======\nt2\n>>>>>>> gitbutler-resolve-theirs\nc\n";
        let hunks = extract_hunks(text);
        assert_eq!(hunks[0].line, 2);
        assert_eq!(hunks[1].line, 4);
    }

    #[test]
    fn malformed_markers_are_ignored() {
        let text = "a\n<<<<<<< gitbutler-resolve-ours\nno separator or close\n";
        assert!(extract_hunks(text).is_empty());
        let text = "=======\n>>>>>>> gitbutler-resolve-theirs\n";
        assert!(extract_hunks(text).is_empty());
    }

    #[test]
    fn crlf_lines_are_handled() {
        let text = "a\r\n<<<<<<< gitbutler-resolve-ours\r\nx\r\n=======\r\ny\r\n>>>>>>> gitbutler-resolve-theirs\r\nb\r\n";
        let hunks = extract_hunks(text);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].ours, "x");
        assert_eq!(hunks[0].theirs, "y");
    }

    #[test]
    fn only_exact_machine_markers_open_blocks() {
        for lookalike in ["<<<<<<<", "<<<<<<< ours", "<<<<<<< HEAD"] {
            let text = format!("{lookalike}\nx\n=======\ny\n>>>>>>> gitbutler-resolve-theirs\n");
            assert!(
                extract_hunks(&text).is_empty(),
                "{lookalike:?} must not open a block"
            );
        }
    }

    /// A marker-shaped line inside a side's content must not close the block
    /// early — only the exact machine-generated closing marker ends it.
    #[test]
    fn content_marker_lines_do_not_close_blocks_early() {
        let text = "before\n<<<<<<< gitbutler-resolve-ours\nour code\n=======\ntheir code part 1\n>>>>>>> fixture-line\ntheir code part 2\n>>>>>>> gitbutler-resolve-theirs\nafter\n";
        let hunks = extract_hunks(text);
        assert_eq!(hunks.len(), 1);
        assert_eq!(
            hunks[0].theirs, "their code part 1\n>>>>>>> fixture-line\ntheir code part 2",
            "the fixture line must remain part of the theirs content"
        );
    }

    #[test]
    fn marker_shaped_detection() {
        assert!(is_marker_shaped("<<<<<<< HEAD"));
        assert!(is_marker_shaped("<<<<<<<"));
        assert!(is_marker_shaped(">>>>>>>>>>> nested-size-marker"));
        assert!(is_marker_shaped("||||||| base"));
        assert!(
            !is_marker_shaped("======="),
            "lone separators are legal content"
        );
        assert!(!is_marker_shaped("<<<<<<no"));
        assert!(!is_marker_shaped("plain line"));
    }

    #[test]
    fn marker_like_content_is_flagged_as_ambiguous() {
        // Marker-shaped content anywhere, including sentinel lookalikes and
        // exact sentinels outside their block positions.
        assert_eq!(
            ambiguous_line(&format!("a\n<<<<<<< HEAD\nb\n{TWO_WAY}")).as_deref(),
            Some("<<<<<<< HEAD")
        );
        assert_eq!(
            ambiguous_line(&format!("a\n{THEIRS_MARKER}\nb\n{TWO_WAY}")).as_deref(),
            Some(THEIRS_MARKER)
        );
        // A lone separator inside a block gets mis-bound as the section
        // separator, which displaces the real markers to non-marker positions
        // and must be flagged; outside all blocks it is harmless content.
        let separator_in_ours = "<<<<<<< gitbutler-resolve-ours\nTitle\n=======\nours body\n||||||| gitbutler-resolve-base\nbase\n=======\ntheirs\n>>>>>>> gitbutler-resolve-theirs\n";
        assert!(ambiguous_line(separator_in_ours).is_some());
        let separator_in_theirs = "<<<<<<< gitbutler-resolve-ours\nours\n=======\nTitle\n=======\ntheirs\n>>>>>>> gitbutler-resolve-theirs\n";
        assert_eq!(
            ambiguous_line(separator_in_theirs).as_deref(),
            Some("=======")
        );
        assert_eq!(
            ambiguous_line(&format!("Heading\n=======\n\n{DIFF3}")),
            None
        );
        // The machine markers themselves are not ambiguous.
        assert_eq!(ambiguous_line(TWO_WAY), None);
        assert_eq!(ambiguous_line(DIFF3), None);
    }
}
