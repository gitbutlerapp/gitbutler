//! Turn per-hunk resolutions into a rewritten commit.
//!
//! Resolutions are applied by *narrowing the conflict*: each resolved hunk's
//! content is written into all three side blobs (ours/base/theirs), while
//! unresolved hunks keep each side's original lines. Re-merging the synthesized
//! trees then conflicts exactly at the unresolved hunks — so a partial
//! resolution yields a conflicted commit with fewer conflicts, and a full
//! resolution falls out of the very same path as a clean merge.

use std::collections::BTreeMap;

use anyhow::{Context as _, bail};
use bstr::ByteSlice;
use but_core::DryRun;
use but_core::commit::Headers;
use but_core::commit::tree_expression::TreeExpression;
use but_core::sync::RepoExclusive;
use but_rebase::commit::DateMode;
use but_rebase::graph_rebase::{Editor, LookupStep as _, Step};

use super::context::{FileConflict, ResolutionRequest, is_marker_shaped, scan_conflict_blocks};
use super::{HunkResolution, RemainingConflicts, ResolutionSpec};
use crate::WorkspaceState;

/// What applying resolutions produced.
pub(crate) struct AppliedResolution {
    /// The rewritten commit's final id after the rebase.
    pub new_commit: gix::ObjectId,
    /// The conflicts that remain per file; empty when fully resolved.
    pub remaining: Vec<RemainingConflicts>,
    /// Workspace state after the apply.
    pub workspace: WorkspaceState,
}

/// A validated resolution for one hunk. Side picks stay symbolic so the
/// synthesis can copy the picked side's raw bytes instead of a re-rendered
/// string.
#[derive(Debug, Clone)]
pub(crate) enum HunkPick {
    Ours,
    Theirs,
    Content(String),
}

/// Resolved hunks per file, aligned index-for-index with the request's files.
/// Keys are 0-based hunk indices; an empty map leaves the file's conflicts
/// untouched.
pub(crate) type PicksPerFile = Vec<BTreeMap<usize, HunkPick>>;

/// Validate caller-provided resolution specs against the request and translate
/// them into per-file hunk contents. Nothing is written to the repository here.
pub(crate) fn validate_specs(
    request: &ResolutionRequest,
    specs: &[ResolutionSpec],
) -> anyhow::Result<PicksPerFile> {
    let files_by_path = index_files_by_path(request)?;
    let mut picks: PicksPerFile = vec![BTreeMap::new(); request.files.len()];

    for spec in specs {
        let &file_index = files_by_path
            .get(&normalize_path(&spec.path))
            .with_context(|| {
                format!("\"{}\" is not a conflicted file of this commit", spec.path)
            })?;
        let file = &request.files[file_index];
        if spec.hunk == 0 || spec.hunk > file.hunks.len() {
            bail!(
                "\"{}\" has conflicts 1..{}, but conflict {} was addressed",
                file.path,
                file.hunks.len(),
                spec.hunk
            );
        }
        let hunk_index = spec.hunk - 1;
        let pick = match &spec.resolution {
            HunkResolution::Ours => HunkPick::Ours,
            HunkResolution::Theirs => HunkPick::Theirs,
            HunkResolution::Content(content) => {
                ensure_no_markers(content, &file.path)?;
                HunkPick::Content(content.clone())
            }
        };
        if picks[file_index].insert(hunk_index, pick).is_some() {
            bail!(
                "Conflict {} of \"{}\" was addressed more than once",
                spec.hunk,
                file.path
            );
        }
    }

    Ok(picks)
}

/// Map normalized request paths to file indices, rejecting collisions.
pub(crate) fn index_files_by_path(
    request: &ResolutionRequest,
) -> anyhow::Result<BTreeMap<String, usize>> {
    let mut files_by_path = BTreeMap::new();
    for (index, file) in request.files.iter().enumerate() {
        if files_by_path
            .insert(normalize_path(&file.path), index)
            .is_some()
        {
            bail!(
                "Two conflicted paths normalize to the same value ({:?}); resolve this commit manually instead",
                normalize_path(&file.path)
            );
        }
    }
    Ok(files_by_path)
}

/// Reject content that contains a conflict-marker-shaped line.
pub(crate) fn ensure_no_markers(content: &str, path: &str) -> anyhow::Result<()> {
    if let Some(marker) = content
        .lines()
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .find(|line| is_marker_shaped(line))
    {
        bail!("The resolution for \"{path}\" contains a conflict marker ({marker:?})");
    }
    Ok(())
}

/// Normalize a path for comparison: trim, backslashes to slashes, strip a
/// leading `./`, collapse duplicate slashes. Applied to both request and
/// caller-provided paths.
pub(crate) fn normalize_path(path: &str) -> String {
    let mut normalized = path.trim().replace('\\', "/");
    while normalized.contains("//") {
        normalized = normalized.replace("//", "/");
    }
    normalized
        .strip_prefix("./")
        .map(str::to_owned)
        .unwrap_or(normalized)
}

#[derive(Debug, Clone, Copy)]
enum SideKind {
    Ours,
    Base,
    Theirs,
}

/// Render one side's narrowed version of a conflicted file: resolved blocks
/// become the picked content, unresolved blocks keep this side's own lines.
///
/// Non-conflicted lines, unresolved side lines, and side picks are copied
/// byte-for-byte including their own terminators, so mixed-EOL files stay
/// untouched outside resolved regions. Inserted custom-content lines use the
/// file's dominant EOL, and a single trailing newline on such content is
/// dropped since the block's own terminator is preserved.
fn synthesize_side(
    file: &FileConflict,
    picks: &BTreeMap<usize, HunkPick>,
    side: SideKind,
) -> anyhow::Result<String> {
    let eol = if file.merged_text.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    // Raw lines keep their terminators; the stripped view drives the scanner.
    let raw_lines: Vec<&str> = file.merged_text.split_inclusive('\n').collect();
    let stripped_lines: Vec<&str> = raw_lines
        .iter()
        .map(|line| strip_terminator(line))
        .collect();
    let blocks = scan_conflict_blocks(&stripped_lines);
    if blocks.len() != file.hunks.len() {
        bail!(
            "BUG: found {} conflict blocks in \"{}\" while applying, but the request has {} hunks",
            blocks.len(),
            file.path,
            file.hunks.len()
        );
    }

    let mut result = String::with_capacity(file.merged_text.len());
    let mut line = 0;
    for (index, block) in blocks.iter().enumerate() {
        for raw in &raw_lines[line..block.start] {
            result.push_str(raw);
        }
        if let Some(pick) = picks.get(&index) {
            let resolution = match pick {
                // The picked side's raw bytes, terminators included.
                HunkPick::Ours => {
                    for raw in &raw_lines[block.ours.clone()] {
                        result.push_str(raw);
                    }
                    line = block.end + 1;
                    continue;
                }
                HunkPick::Theirs => {
                    for raw in &raw_lines[block.theirs.clone()] {
                        result.push_str(raw);
                    }
                    line = block.end + 1;
                    continue;
                }
                HunkPick::Content(content) => content,
            };
            let resolution = resolution
                .strip_suffix('\n')
                .map(|r| r.strip_suffix('\r').unwrap_or(r))
                .unwrap_or(resolution);
            if !resolution.is_empty() {
                let block_is_terminated = raw_lines[block.end].ends_with('\n');
                for (index, resolved_line) in resolution
                    .split('\n')
                    .map(|l| l.strip_suffix('\r').unwrap_or(l))
                    .enumerate()
                {
                    if index > 0 {
                        result.push_str(eol);
                    }
                    result.push_str(resolved_line);
                }
                if block_is_terminated {
                    result.push_str(eol);
                }
            }
        } else {
            let side_range = match side {
                SideKind::Ours => block.ours.clone(),
                SideKind::Theirs => block.theirs.clone(),
                SideKind::Base => block.base.clone().with_context(|| {
                    format!(
                        "A conflict in \"{}\" carries no common-ancestor section and cannot be narrowed. Resolve this commit manually instead.",
                        file.path
                    )
                })?,
            };
            for raw in &raw_lines[side_range] {
                result.push_str(raw);
            }
        }
        line = block.end + 1;
    }
    for raw in &raw_lines[line..] {
        result.push_str(raw);
    }
    Ok(result)
}

fn strip_terminator(line: &str) -> &str {
    let line = line.strip_suffix('\n').unwrap_or(line);
    line.strip_suffix('\r').unwrap_or(line)
}

/// Apply `picks` to the conflicted commit: synthesize narrowed side trees,
/// re-merge them, and rewrite the commit — into a normal commit when nothing
/// is left unresolved, or into a conflicted commit with the remaining
/// conflicts otherwise. Descendants are rebased either way.
///
/// Returns the rewritten commit's final id, the conflicts that remain per
/// file, and the resulting workspace state.
pub(crate) fn apply(
    ctx: &mut but_ctx::Context,
    request: &ResolutionRequest,
    picks_per_file: &PicksPerFile,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<AppliedResolution> {
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;

    // Narrow the sides: every resolved hunk's content goes into all three
    // trees, so the re-merge below agrees there and conflicts only at what
    // remains unresolved.
    let mut base_edit = repo.edit_tree(request.base_tree_id)?;
    let mut ours_edit = repo.edit_tree(request.ours_tree_id)?;
    let mut theirs_edit = repo.edit_tree(request.theirs_tree_id)?;
    for (file, picks) in request.files.iter().zip(picks_per_file) {
        if picks.is_empty() {
            continue;
        }
        for (tree, side) in [
            (&mut base_edit, SideKind::Base),
            (&mut ours_edit, SideKind::Ours),
            (&mut theirs_edit, SideKind::Theirs),
        ] {
            let content = synthesize_side(file, picks, side)?;
            let blob_id = repo.write_blob(content.as_bytes())?;
            tree.upsert(file.rela_path.as_bstr(), file.entry_kind, blob_id)?;
        }
    }
    let base_tree_id = base_edit.write()?.detach();
    let ours_tree_id = ours_edit.write()?.detach();
    let theirs_tree_id = theirs_edit.write()?.detach();

    // Auto-resolve like the rebase engine does when it writes conflicted
    // commits: favor *ours* so the merged tree is clean content, never marker
    // text, and detect the forcefully-resolved conflicts as unresolved.
    use but_core::RepositoryExt as _;
    let mut outcome = repo.merge_trees(
        base_tree_id,
        ours_tree_id,
        theirs_tree_id,
        repo.default_merge_labels(),
        repo.merge_options_force_ours()?,
    )?;
    let merged_tree_id = outcome.tree.write()?.detach();
    let treat_as_unresolved = gix::merge::tree::TreatAsUnresolved::forced_resolution();
    let unresolved = outcome.has_unresolved_conflicts(treat_as_unresolved);

    let remaining: Vec<RemainingConflicts> = request
        .files
        .iter()
        .zip(picks_per_file)
        .filter_map(|(file, picks)| {
            let remaining = file.hunks.len() - picks.len();
            (remaining > 0).then(|| RemainingConflicts {
                path: file.path.clone(),
                hunks: remaining,
            })
        })
        .collect();
    if unresolved && remaining.is_empty() {
        bail!(
            "BUG: all conflicts of commit {} were resolved, yet re-merging the narrowed trees still conflicts",
            request.commit_id
        );
    }

    let mut editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let (target_selector, mut commit) = editor.find_selectable_commit(request.commit_id)?;
    if unresolved {
        // Still conflicted: same commit shape the rebase engine writes, with
        // the narrowed trees. Adding message markers is idempotent, and covers
        // commits that were only marked by the legacy header — the header is
        // cleared below, so the message must carry the state.
        commit.message = but_core::commit::add_conflict_markers(commit.message.as_ref());
        let conflict_entries = but_core::commit::conflict_entries_from_merge_outcome(
            &repo,
            merged_tree_id,
            &outcome,
            treat_as_unresolved,
        )?;
        let tree_expression = TreeExpression {
            base_tree_ids: vec![base_tree_id],
            side_tree_ids: [ours_tree_id, theirs_tree_id].into_iter().collect(),
        };
        commit.tree = but_core::commit::write_conflicted_tree(
            &repo,
            merged_tree_id,
            &tree_expression,
            &conflict_entries,
        )?;
    } else {
        commit.tree = merged_tree_id;
        commit.message = but_core::commit::strip_conflict_markers(commit.message.as_ref());
    }
    if let Some(headers) = Headers::try_from_commit(&commit) {
        Headers {
            conflicted: None,
            ..headers
        }
        .set_in_commit(&mut commit);
    }
    let new_id = editor.new_commit(commit, DateMode::CommitterUpdateAuthorKeep)?;
    editor.replace(target_selector, Step::new_pick(new_id))?;

    let rebase = editor.rebase()?;
    let new_commit = rebase.lookup_pick(target_selector)?;
    let workspace = WorkspaceState::from_successful_rebase(rebase, &repo, dry_run)?;

    Ok(AppliedResolution {
        new_commit,
        remaining,
        workspace,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::context::split_lines;

    const OURS: &str = "<<<<<<< gitbutler-resolve-ours";
    const BASE: &str = "||||||| gitbutler-resolve-base";
    const THEIRS: &str = ">>>>>>> gitbutler-resolve-theirs";

    fn test_file(merged_text: &str) -> FileConflict {
        let lines = split_lines(merged_text);
        let blocks = scan_conflict_blocks(&lines);
        FileConflict {
            path: "file.txt".into(),
            rela_path: "file.txt".into(),
            entry_kind: gix::objs::tree::EntryKind::Blob,
            merged_text: merged_text.to_owned(),
            hunks: blocks
                .iter()
                .map(|block| super::super::context::ConflictHunk {
                    line: 0,
                    context_before: String::new(),
                    ours: lines[block.ours.clone()].join("\n"),
                    base: block.base.clone().map(|range| lines[range].join("\n")),
                    theirs: lines[block.theirs.clone()].join("\n"),
                    context_after: String::new(),
                })
                .collect(),
        }
    }

    fn picks(entries: &[(usize, &str)]) -> BTreeMap<usize, HunkPick> {
        entries
            .iter()
            .map(|(index, content)| (*index, HunkPick::Content(content.to_string())))
            .collect()
    }

    fn two_hunks() -> FileConflict {
        test_file(&format!(
            "start\n{OURS}\nours one\n{BASE}\nbase one\n=======\ntheirs one\n{THEIRS}\nmiddle\n{OURS}\nours two\n{BASE}\nbase two\n=======\ntheirs two\n{THEIRS}\nend\n"
        ))
    }

    #[test]
    fn full_resolution_makes_all_sides_agree() {
        let file = two_hunks();
        let all = picks(&[(0, "merged one"), (1, "merged two")]);
        let expected = "start\nmerged one\nmiddle\nmerged two\nend\n";
        for side in [SideKind::Ours, SideKind::Base, SideKind::Theirs] {
            assert_eq!(synthesize_side(&file, &all, side).unwrap(), expected);
        }
    }

    #[test]
    fn partial_resolution_keeps_each_sides_lines_for_unresolved_blocks() {
        let file = two_hunks();
        let first_only = picks(&[(0, "merged one")]);
        assert_eq!(
            synthesize_side(&file, &first_only, SideKind::Ours).unwrap(),
            "start\nmerged one\nmiddle\nours two\nend\n"
        );
        assert_eq!(
            synthesize_side(&file, &first_only, SideKind::Base).unwrap(),
            "start\nmerged one\nmiddle\nbase two\nend\n"
        );
        assert_eq!(
            synthesize_side(&file, &first_only, SideKind::Theirs).unwrap(),
            "start\nmerged one\nmiddle\ntheirs two\nend\n"
        );
    }

    #[test]
    fn empty_resolution_deletes_the_block() {
        let file = test_file(&format!(
            "before\n{OURS}\na\n{BASE}\nb\n=======\nc\n{THEIRS}\nafter\n"
        ));
        let result = synthesize_side(&file, &picks(&[(0, "")]), SideKind::Theirs).unwrap();
        assert_eq!(result, "before\nafter\n");
    }

    /// Mixed-EOL files must keep every untouched line's own terminator —
    /// only inserted resolution lines use the dominant EOL.
    #[test]
    fn synthesis_preserves_mixed_eol_outside_resolved_blocks() {
        let file = test_file(&format!(
            "line1\nwin\r\nline3\n{OURS}\na\n{BASE}\nb\n=======\nc\n{THEIRS}\nline4\n"
        ));
        let result = synthesize_side(&file, &picks(&[(0, "merged")]), SideKind::Ours).unwrap();
        assert_eq!(
            result, "line1\nwin\r\nline3\nmerged\r\nline4\n",
            "untouched lines keep their own terminators; the inserted line uses the dominant (CRLF) EOL"
        );
    }

    #[test]
    fn synthesis_strips_a_single_trailing_newline_from_resolutions() {
        let file = test_file(&format!(
            "before\n{OURS}\na\n{BASE}\nb\n=======\nc\n{THEIRS}\nafter\n"
        ));
        let result = synthesize_side(&file, &picks(&[(0, "merged\n")]), SideKind::Ours).unwrap();
        assert_eq!(result, "before\nmerged\nafter\n");
        let result = synthesize_side(&file, &picks(&[(0, "merged\n\n")]), SideKind::Ours).unwrap();
        assert_eq!(result, "before\nmerged\n\nafter\n");
    }

    #[test]
    fn synthesis_without_trailing_newline_at_eof() {
        let file = test_file(&format!(
            "before\n{OURS}\na\n{BASE}\nb\n=======\nc\n{THEIRS}"
        ));
        let result = synthesize_side(&file, &picks(&[(0, "merged")]), SideKind::Ours).unwrap();
        assert_eq!(result, "before\nmerged");
    }

    fn test_request(files: Vec<FileConflict>) -> ResolutionRequest {
        ResolutionRequest {
            commit_id: gix::ObjectId::null(gix::hash::Kind::Sha1),
            commit_message: String::new(),
            parent_message: None,
            base_tree_id: gix::ObjectId::null(gix::hash::Kind::Sha1),
            ours_tree_id: gix::ObjectId::null(gix::hash::Kind::Sha1),
            theirs_tree_id: gix::ObjectId::null(gix::hash::Kind::Sha1),
            files,
        }
    }

    fn spec(path: &str, hunk: usize, resolution: HunkResolution) -> ResolutionSpec {
        ResolutionSpec {
            path: path.into(),
            hunk,
            resolution,
        }
    }

    #[test]
    fn specs_translate_side_picks_and_content() {
        let request = test_request(vec![two_hunks()]);
        let picks = validate_specs(
            &request,
            &[
                spec("file.txt", 1, HunkResolution::Ours),
                spec("./file.txt", 2, HunkResolution::Content("mixed".into())),
            ],
        )
        .unwrap();
        assert!(matches!(picks[0][&0], HunkPick::Ours));
        assert!(matches!(&picks[0][&1], HunkPick::Content(content) if content == "mixed"));
    }

    /// A side pick must reproduce the picked side byte-for-byte — including a
    /// trailing blank line and the side's own line terminators.
    #[test]
    fn side_picks_preserve_the_sides_raw_bytes() {
        let file = test_file(&format!(
            "start\n{OURS}\nfoo\n\n{BASE}\nbase\n=======\ntheirs one\r\ntheirs two\n{THEIRS}\nend\n"
        ));
        let ours: BTreeMap<usize, HunkPick> = [(0, HunkPick::Ours)].into_iter().collect();
        for side in [SideKind::Ours, SideKind::Base, SideKind::Theirs] {
            assert_eq!(
                synthesize_side(&file, &ours, side).unwrap(),
                "start\nfoo\n\nend\n",
                "the ours side's trailing blank line must survive"
            );
        }
        let theirs: BTreeMap<usize, HunkPick> = [(0, HunkPick::Theirs)].into_iter().collect();
        assert_eq!(
            synthesize_side(&file, &theirs, SideKind::Ours).unwrap(),
            "start\ntheirs one\r\ntheirs two\nend\n",
            "the theirs side's own terminators must survive"
        );
    }

    #[test]
    fn specs_are_validated() {
        let request = test_request(vec![two_hunks()]);
        let err = |specs: &[ResolutionSpec]| validate_specs(&request, specs).unwrap_err();

        assert!(
            err(&[spec("other.txt", 1, HunkResolution::Ours)])
                .to_string()
                .contains("not a conflicted file")
        );
        assert!(
            err(&[spec("file.txt", 3, HunkResolution::Ours)])
                .to_string()
                .contains("has conflicts 1..2")
        );
        assert!(
            err(&[spec("file.txt", 0, HunkResolution::Ours)])
                .to_string()
                .contains("has conflicts 1..2")
        );
        assert!(
            err(&[
                spec("file.txt", 1, HunkResolution::Ours),
                spec("file.txt", 1, HunkResolution::Theirs),
            ])
            .to_string()
            .contains("more than once")
        );
        assert!(
            err(&[spec(
                "file.txt",
                1,
                HunkResolution::Content("<<<<<<< HEAD\nx".into()),
            )])
            .to_string()
            .contains("conflict marker")
        );
    }

    #[test]
    fn path_normalization() {
        assert_eq!(normalize_path("./src//main.rs "), "src/main.rs");
        assert_eq!(normalize_path("src\\main.rs"), "src/main.rs");
        assert_eq!(normalize_path("plain.txt"), "plain.txt");
    }
}
