//! Reading and writing GitButler's managed block inside agent instruction
//! files such as `AGENTS.md` and `CLAUDE.md`.
//!
//! These files are usually hand-edited and git-tracked, so every operation here
//! is deliberately conservative: markers are only recognised on their own line
//! and outside fenced code blocks, and a malformed marker pair is an error
//! rather than a guess.

use std::path::Path;

use anyhow::{Context as _, Result};

/// Opening delimiter of the block GitButler owns inside an instruction file.
pub const MANAGED_BLOCK_START: &str = "<!-- gitbutler-agent-setup:start -->";
/// Closing delimiter of the block GitButler owns inside an instruction file.
pub const MANAGED_BLOCK_END: &str = "<!-- gitbutler-agent-setup:end -->";

pub fn upsert_managed_block_file(path: &Path, block: &str) -> Result<()> {
    let original = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err).with_context(|| format!("Failed to read {}", path.display())),
    };
    let updated = upsert_managed_block(&original, block)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    std::fs::write(path, updated).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

/// Find the byte offset of the next occurrence of `needle` in `haystack` at or
/// after `from` that sits on its own line (i.e. at the start of the file or
/// right after a newline, and immediately followed by a newline or EOF).
///
/// This keeps a marker quoted inside prose or inline code (where surrounding
/// text or backticks keep it off its own line), or shown as an example inside a
/// fenced code block, from being mistaken for a real block delimiter — which
/// would otherwise splice away the surrounding text.
pub fn find_line_anchored(haystack: &str, needle: &str, from: usize) -> Option<usize> {
    let bytes = haystack.as_bytes();
    let mut search = from;
    while let Some(rel) = haystack[search..].find(needle) {
        let idx = search + rel;
        let at_line_start = idx == 0 || bytes[idx - 1] == b'\n';
        let after = idx + needle.len();
        let at_line_end = after == haystack.len() || matches!(bytes[after], b'\n' | b'\r');
        if at_line_start && at_line_end && !inside_fenced_block(haystack, idx) {
            return Some(idx);
        }
        search = idx + needle.len();
    }
    None
}

/// Whether the line starting at byte `idx` falls inside a fenced code block
/// (```` ``` ```` or `~~~`). An odd number of fence delimiters before it means a
/// fence is open, so a managed-block marker shown there as a documented example
/// is left alone rather than treated as a real delimiter.
fn inside_fenced_block(haystack: &str, idx: usize) -> bool {
    let mut open = false;
    for line in haystack[..idx].lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            open = !open;
        }
    }
    open
}

/// Byte spans of each managed block, from the first byte of the start marker
/// to just past the last byte of the end marker (both markers lie inside the
/// end-exclusive `Range`). Pairs every line-anchored start with the first
/// line-anchored end after it. Errors when a start marker has no matching end,
/// so callers refuse to touch a malformed block.
pub fn managed_block_spans(existing: &str) -> Result<Vec<std::ops::Range<usize>>> {
    let mut spans = Vec::new();
    let mut pos = 0;
    while let Some(start) = find_line_anchored(existing, MANAGED_BLOCK_START, pos) {
        let Some(end) = find_line_anchored(
            existing,
            MANAGED_BLOCK_END,
            start + MANAGED_BLOCK_START.len(),
        ) else {
            anyhow::bail!(
                "Found a GitButler managed block start marker without a matching end marker. Refusing to edit a partial managed block."
            );
        };
        let span_end = end + MANAGED_BLOCK_END.len();
        spans.push(start..span_end);
        pos = span_end;
    }
    Ok(spans)
}

/// Rewrite `block` to use CRLF line endings when `existing` already does, so a
/// replaced or appended block does not introduce mixed line endings.
fn match_line_endings(existing: &str, block: &str) -> String {
    if existing.contains("\r\n") {
        block.replace("\r\n", "\n").replace('\n', "\r\n")
    } else {
        block.to_string()
    }
}

pub fn upsert_managed_block(existing: &str, block: &str) -> Result<String> {
    let start = find_line_anchored(existing, MANAGED_BLOCK_START, 0);
    let end = find_line_anchored(existing, MANAGED_BLOCK_END, 0);

    match (start, end) {
        // No managed block yet: append a fresh one.
        (None, None) => return Ok(append_managed_block(existing, block)),
        (Some(_), None) => anyhow::bail!(
            "Found only the GitButler managed block start marker. Refusing to edit a partial managed block."
        ),
        (None, Some(_)) => anyhow::bail!(
            "Found only the GitButler managed block end marker. Refusing to edit a partial managed block."
        ),
        (Some(start), Some(end)) if end < start => anyhow::bail!(
            "Found GitButler managed block markers in the wrong order (end before start). Refusing to edit a malformed managed block."
        ),
        // Well-formed: a start marker with an end after it. Fall through to replace.
        (Some(_), Some(_)) => {}
    }

    // Replace the first block with the fresh one and drop any extra blocks
    // (e.g. left over from an earlier buggy run), so the file converges to
    // exactly one block.
    let mut spans = Vec::new();
    for span in managed_block_spans(existing)? {
        let mut block_end = span.end;
        if existing[block_end..].starts_with("\r\n") {
            block_end += 2;
        } else if existing[block_end..].starts_with('\n') {
            block_end += 1;
        }
        spans.push((span.start, block_end));
    }

    let block = match_line_endings(existing, block);
    let mut updated = String::with_capacity(existing.len() + block.len());
    let mut copied = 0;
    for (index, (start, block_end)) in spans.iter().enumerate() {
        updated.push_str(&existing[copied..*start]);
        if index == 0 {
            updated.push_str(&block);
        }
        copied = *block_end;
    }
    updated.push_str(&existing[copied..]);
    Ok(updated)
}

fn append_managed_block(existing: &str, block: &str) -> String {
    if existing.is_empty() {
        return block.to_string();
    }

    let block = match_line_endings(existing, block);
    let crlf = existing.contains("\r\n");
    let mut updated = String::with_capacity(existing.len() + block.len() + 2);
    updated.push_str(existing);
    if existing.ends_with("\r\n\r\n") || existing.ends_with("\n\n") {
        // Already separated by a blank line.
    } else if crlf {
        updated.push_str(if existing.ends_with("\r\n") {
            "\r\n"
        } else {
            "\r\n\r\n"
        });
    } else if existing.ends_with('\n') {
        updated.push('\n');
    } else {
        updated.push_str("\n\n");
    }
    updated.push_str(&block);
    updated
}

/// The contents of the managed block in `existing`, markers included, or
/// `None` when there is no block.
///
/// Errors on a malformed marker pair for the same reason
/// [`upsert_managed_block`] does: a partial block is not ours to interpret.
pub fn read_managed_block(existing: &str) -> Result<Option<String>> {
    Ok(managed_block_spans(existing)?
        .first()
        .map(|span| existing[span.clone()].to_string()))
}

/// Like [`read_managed_block`], but reads `path` first. A missing file has no
/// block.
pub fn read_managed_block_file(path: &Path) -> Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(content) => read_managed_block(&content),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("Failed to read {}", path.display())),
    }
}

/// Splice every managed block out of `existing`, returning `None` when there
/// was none to remove.
///
/// Inherits [`managed_block_spans`]'s protections: markers quoted in prose or
/// shown inside a fenced code block are left alone, and a partial or reversed
/// marker pair errors rather than guessing at a span to delete.
pub fn remove_managed_block(existing: &str) -> Result<Option<String>> {
    let spans = managed_block_spans(existing)?;
    if spans.is_empty() {
        return Ok(None);
    }

    let mut updated = String::with_capacity(existing.len());
    let mut copied = 0;
    for span in spans {
        updated.push_str(&existing[copied..span.start]);
        copied = span.end;
        // Take the block's own line terminator with it, so removing a block
        // does not leave a widening gap behind each time.
        if existing[copied..].starts_with("\r\n") {
            copied += 2;
        } else if existing[copied..].starts_with('\n') {
            copied += 1;
        }
        // `append_managed_block` separates the block from preceding text with
        // a blank line. Take that back too, but only when leaving it would
        // strand a trailing or doubled blank line — otherwise a block that
        // merely follows a blank line would lose it.
        let rest_starts_new_line =
            existing[copied..].is_empty() || existing[copied..].starts_with('\n');
        if rest_starts_new_line {
            if updated.ends_with("\r\n\r\n") {
                updated.truncate(updated.len() - 2);
            } else if updated.ends_with("\n\n") {
                updated.truncate(updated.len() - 1);
            }
        }
    }
    updated.push_str(&existing[copied..]);
    Ok(Some(updated))
}

/// Remove the managed block from `path`, leaving the rest of the file intact.
///
/// Never deletes the file, even when nothing but the block was in it: these
/// are usually git-tracked files the user owns, and removing one is a
/// surprising, visible side effect of uninstalling a skill.
///
/// Returns whether anything changed.
pub fn remove_managed_block_file(path: &Path) -> Result<bool> {
    let original = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err).with_context(|| format!("Failed to read {}", path.display())),
    };
    let Some(updated) = remove_managed_block(&original)? else {
        return Ok(false);
    };
    std::fs::write(path, updated).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn managed(body: &str) -> String {
        format!("{MANAGED_BLOCK_START}\n{body}\n{MANAGED_BLOCK_END}\n")
    }

    #[test]
    fn removes_the_block_and_nothing_else() {
        let existing = format!("# Rules\n\n{}\nAfter.\n", managed("- policy"));
        let updated = remove_managed_block(&existing).unwrap().unwrap();
        assert_eq!(updated, "# Rules\n\nAfter.\n");
    }

    #[test]
    fn reports_no_change_when_there_is_no_block() {
        assert_eq!(remove_managed_block("# Just my notes\n").unwrap(), None);
    }

    /// The same refusal `upsert_managed_block` makes: half a marker pair is
    /// not a span we can safely delete.
    #[test]
    fn refuses_a_partial_marker_pair() {
        let existing = format!("# Rules\n\n{MANAGED_BLOCK_START}\n- policy\n");
        assert!(remove_managed_block(&existing).is_err());
    }

    /// A marker shown as documentation inside a fenced block is not a real
    /// delimiter, so nothing should be spliced out around it.
    #[test]
    fn ignores_markers_inside_a_fenced_code_block() {
        let existing =
            format!("# Docs\n\n```\n{MANAGED_BLOCK_START}\n{MANAGED_BLOCK_END}\n```\n\nEnd.\n");
        assert_eq!(remove_managed_block(&existing).unwrap(), None);
    }

    #[test]
    fn preserves_crlf_content_around_the_block() {
        let existing = format!(
            "# Rules\r\n\r\n{MANAGED_BLOCK_START}\r\n- policy\r\n{MANAGED_BLOCK_END}\r\nAfter.\r\n"
        );
        let updated = remove_managed_block(&existing).unwrap().unwrap();
        assert_eq!(updated, "# Rules\r\n\r\nAfter.\r\n");
    }

    /// Uninstalling a skill must not delete a file the user owns and git
    /// tracks, even if the block was all it contained.
    #[test]
    fn leaves_an_emptied_file_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("AGENTS.md");
        std::fs::write(&path, managed("- policy")).unwrap();

        assert!(remove_managed_block_file(&path).unwrap());
        assert!(path.is_file(), "the file still exists");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
    }

    #[test]
    fn round_trips_with_upsert() {
        let original = "# Rules\n\nMy own notes.\n";
        let block = managed("- policy");
        let written = upsert_managed_block(original, &block).unwrap();
        assert_eq!(remove_managed_block(&written).unwrap().unwrap(), original);
    }
}
