//! Validate a model response against the request, splice the resolved hunks
//! into the merged blobs, and rewrite the conflicted commit into a normal one.

use anyhow::{Context as _, bail};
use bstr::ByteSlice;
use but_core::DryRun;
use but_core::commit::Headers;
use but_core::sync::RepoExclusive;
use but_rebase::commit::DateMode;
use but_rebase::graph_rebase::{Editor, LookupStep as _, Step};

use super::context::{FileConflict, ResolutionRequest, is_marker_shaped, scan_conflict_blocks};
use super::prompt::ResolutionResponse;
use crate::WorkspaceState;

/// A validated, spliced resolution, aligned index-for-index with the
/// request's files.
#[derive(Debug)]
pub(crate) struct ValidatedFile {
    /// The full file content with every conflict block replaced by its resolution.
    pub resolved_text: String,
    /// The per-hunk replacement contents, in file order.
    pub hunks: Vec<String>,
    /// The model's per-file reasoning.
    pub reasoning: String,
}

/// Check `response` against `request` and splice the resolutions into the
/// merged blobs. Any mismatch fails validation so the caller can retry the
/// model once before giving up. Nothing is written to the repository here.
pub(crate) fn validate(
    request: &ResolutionRequest,
    response: &ResolutionResponse,
) -> anyhow::Result<Vec<ValidatedFile>> {
    // Both sides of the path comparison are normalized: models tend to add
    // `./` or backslashes, and request paths are matched under the same rules
    // so a path that normalizes differently can never silently mismatch.
    let mut request_paths = std::collections::BTreeSet::new();
    for file in &request.files {
        if !request_paths.insert(normalize_path(&file.path)) {
            bail!(
                "Two conflicted paths normalize to the same value ({:?}); resolve this commit manually instead",
                normalize_path(&file.path)
            );
        }
    }

    let mut resolutions_by_path = std::collections::BTreeMap::new();
    for resolution in &response.resolutions {
        let path = normalize_path(&resolution.path);
        if resolutions_by_path
            .insert(path.clone(), resolution)
            .is_some()
        {
            bail!("The model returned more than one resolution for \"{path}\"");
        }
    }

    let mut validated = Vec::with_capacity(request.files.len());
    for file in &request.files {
        let resolution = resolutions_by_path
            .remove(&normalize_path(&file.path))
            .with_context(|| {
                format!(
                    "The model returned no resolution for the conflicted file \"{}\"",
                    file.path
                )
            })?;
        if resolution.hunks.len() != file.hunks.len() {
            bail!(
                "The model returned {} resolved hunks for \"{}\" but the file has {} conflicts",
                resolution.hunks.len(),
                file.path,
                file.hunks.len()
            );
        }
        if resolution.reasoning.trim().is_empty() {
            bail!("The model returned no reasoning for \"{}\"", file.path);
        }
        let hunks: Vec<String> = resolution
            .hunks
            .iter()
            .map(|hunk| hunk.resolved_content.clone())
            .collect();
        let resolved_text = splice_resolved_file(file, &hunks)?;
        // The inputs were verified to be free of marker-shaped content when
        // the request was built, so any marker-shaped line in the spliced
        // output can only come from the model.
        if let Some(marker) = resolved_text
            .lines()
            .find(|line| is_marker_shaped(line.strip_suffix('\r').unwrap_or(line)))
        {
            bail!(
                "The model's resolution for \"{}\" still contains a conflict marker ({marker:?})",
                file.path
            );
        }
        validated.push(ValidatedFile {
            resolved_text,
            hunks,
            reasoning: resolution.reasoning.clone(),
        });
    }

    if let Some(path) = resolutions_by_path.into_keys().next() {
        bail!("The model returned a resolution for \"{path}\", which was not requested");
    }

    Ok(validated)
}

/// Replace each conflict block in the merged text with its resolved content.
///
/// Non-conflicted lines are copied byte-for-byte including their own line
/// terminators, so mixed-EOL files stay untouched outside conflict regions.
/// Only the inserted resolution lines use the file's dominant EOL style, and a
/// single trailing newline on a resolution is dropped since the block's own
/// terminator is preserved.
fn splice_resolved_file(file: &FileConflict, resolutions: &[String]) -> anyhow::Result<String> {
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
    if blocks.len() != resolutions.len() {
        bail!(
            "BUG: found {} conflict blocks in \"{}\" while splicing, but validated {} resolutions",
            blocks.len(),
            file.path,
            resolutions.len()
        );
    }

    let mut result = String::with_capacity(file.merged_text.len());
    let mut line = 0;
    for (block, resolution) in blocks.iter().zip(resolutions) {
        for raw in &raw_lines[line..block.start] {
            result.push_str(raw);
        }
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

/// Normalize a path for comparison: trim, backslashes to slashes, strip a
/// leading `./`, collapse duplicate slashes. Applied to both request and
/// response paths.
fn normalize_path(path: &str) -> String {
    let mut normalized = path.trim().replace('\\', "/");
    while normalized.contains("//") {
        normalized = normalized.replace("//", "/");
    }
    normalized
        .strip_prefix("./")
        .map(str::to_owned)
        .unwrap_or(normalized)
}

/// Write the resolved blobs and tree, rewrite the conflicted commit into a
/// normal commit (resolved tree, stripped message, conflict header removed),
/// and rebase its descendants.
///
/// Returns the rewritten commit's final id and the resulting workspace state.
pub(crate) fn apply(
    ctx: &mut but_ctx::Context,
    request: &ResolutionRequest,
    validated: &[ValidatedFile],
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<(gix::ObjectId, WorkspaceState)> {
    let prs_by_head = crate::workspace_state::forge_prs_by_head(ctx)?;
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;

    let mut tree_editor = repo.edit_tree(request.merged_tree_id)?;
    for (file, resolution) in request.files.iter().zip(validated) {
        let blob_id = repo.write_blob(resolution.resolved_text.as_bytes())?;
        tree_editor.upsert(file.rela_path.as_bstr(), file.entry_kind, blob_id)?;
    }
    let resolved_tree_id = tree_editor.write()?.detach();

    let mut editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let (target_selector, mut commit) = editor.find_selectable_commit(request.commit_id)?;
    commit.tree = resolved_tree_id;
    commit.message = but_core::commit::strip_conflict_markers(commit.message.as_ref());
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
    let workspace = WorkspaceState::from_successful_rebase(rebase, &repo, dry_run, &prs_by_head)?;

    Ok((new_commit, workspace))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::context::split_lines;

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
                .map(|_| super::super::context::ConflictHunk {
                    context_before: String::new(),
                    ours: String::new(),
                    base: None,
                    theirs: String::new(),
                    context_after: String::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn splice_replaces_blocks_and_preserves_surroundings() {
        let file = test_file(
            "before\n<<<<<<< gitbutler-resolve-ours\na\n=======\nb\n>>>>>>> gitbutler-resolve-theirs\nbetween\n<<<<<<< gitbutler-resolve-ours\nc\n=======\nd\n>>>>>>> gitbutler-resolve-theirs\nafter\n",
        );
        let result =
            splice_resolved_file(&file, &["merged one".to_owned(), "merged two".to_owned()])
                .unwrap();
        assert_eq!(result, "before\nmerged one\nbetween\nmerged two\nafter\n");
    }

    #[test]
    fn splice_empty_resolution_deletes_the_block() {
        let file = test_file(
            "before\n<<<<<<< gitbutler-resolve-ours\na\n=======\nb\n>>>>>>> gitbutler-resolve-theirs\nafter\n",
        );
        let result = splice_resolved_file(&file, &[String::new()]).unwrap();
        assert_eq!(result, "before\nafter\n");
    }

    #[test]
    fn splice_preserves_crlf() {
        let file = test_file(
            "a\r\n<<<<<<< gitbutler-resolve-ours\r\nx\r\n=======\r\ny\r\n>>>>>>> gitbutler-resolve-theirs\r\nb\r\n",
        );
        let result = splice_resolved_file(&file, &["merged".to_owned()]).unwrap();
        assert_eq!(result, "a\r\nmerged\r\nb\r\n");
    }

    /// Mixed-EOL files must keep every non-conflicted line's own terminator —
    /// only inserted resolution lines use the dominant EOL.
    #[test]
    fn splice_preserves_mixed_eol_outside_conflicts() {
        let file = test_file(
            "line1\nwin\r\nline3\n<<<<<<< gitbutler-resolve-ours\na\n=======\nb\n>>>>>>> gitbutler-resolve-theirs\nline4\n",
        );
        let result = splice_resolved_file(&file, &["merged".to_owned()]).unwrap();
        assert_eq!(
            result, "line1\nwin\r\nline3\nmerged\r\nline4\n",
            "non-conflicted lines keep their own terminators; the inserted line uses the dominant (CRLF) EOL"
        );
    }

    /// A CRLF that only exists inside the replaced conflict block must not
    /// change any line outside it.
    #[test]
    fn splice_ignores_eol_of_deleted_block_content() {
        let file = test_file(
            "line1\n<<<<<<< gitbutler-resolve-ours\na\r\n=======\nb\n>>>>>>> gitbutler-resolve-theirs\nline2\n",
        );
        let result = splice_resolved_file(&file, &[String::new()]).unwrap();
        assert_eq!(result, "line1\nline2\n");
    }

    #[test]
    fn splice_strips_a_single_trailing_newline_from_resolutions() {
        let file = test_file(
            "before\n<<<<<<< gitbutler-resolve-ours\na\n=======\nb\n>>>>>>> gitbutler-resolve-theirs\nafter\n",
        );
        let result = splice_resolved_file(&file, &["merged\n".to_owned()]).unwrap();
        assert_eq!(result, "before\nmerged\nafter\n");
        // An intentional blank line (two newlines) keeps one.
        let result = splice_resolved_file(&file, &["merged\n\n".to_owned()]).unwrap();
        assert_eq!(result, "before\nmerged\n\nafter\n");
    }

    #[test]
    fn splice_without_trailing_newline_at_eof() {
        let file = test_file(
            "before\n<<<<<<< gitbutler-resolve-ours\na\n=======\nb\n>>>>>>> gitbutler-resolve-theirs",
        );
        let result = splice_resolved_file(&file, &["merged".to_owned()]).unwrap();
        assert_eq!(
            result, "before\nmerged",
            "an unterminated closing marker keeps the file unterminated"
        );
    }

    #[test]
    fn path_normalization() {
        assert_eq!(normalize_path("./src//main.rs "), "src/main.rs");
        assert_eq!(normalize_path("src\\main.rs"), "src/main.rs");
        assert_eq!(normalize_path("plain.txt"), "plain.txt");
    }
}
