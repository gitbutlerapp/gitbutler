use std::path::Path;

use anyhow::{Context as _, Result};

use super::{MANAGED_BLOCK_END, MANAGED_BLOCK_START};

pub(super) fn upsert_managed_block_file(path: &Path, block: &str) -> Result<()> {
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
pub(super) fn find_line_anchored(haystack: &str, needle: &str, from: usize) -> Option<usize> {
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

/// Rewrite `block` to use CRLF line endings when `existing` already does, so a
/// replaced or appended block does not introduce mixed line endings.
fn match_line_endings(existing: &str, block: &str) -> String {
    if existing.contains("\r\n") {
        block.replace("\r\n", "\n").replace('\n', "\r\n")
    } else {
        block.to_string()
    }
}

pub(super) fn upsert_managed_block(existing: &str, block: &str) -> Result<String> {
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

    // Pair each start marker with the first end marker after it. Replace the
    // first block with the fresh one and drop any extra blocks (e.g. left over
    // from an earlier buggy run), so the file converges to exactly one block.
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
        let mut block_end = end + MANAGED_BLOCK_END.len();
        if existing[block_end..].starts_with("\r\n") {
            block_end += 2;
        } else if existing[block_end..].starts_with('\n') {
            block_end += 1;
        }
        spans.push((start, block_end));
        pos = block_end;
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
