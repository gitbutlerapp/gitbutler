use crate::git::diff;
use anyhow::{Context, Result};

pub fn hunk_with_context(
    hunk_diff: &str,
    hunk_start_line: usize,
    is_binary: bool,
    context_lines: usize,
    file_lines_before: &[&str],
) -> Result<diff::Hunk> {
    let diff_lines = hunk_diff
        .lines()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>();

    let removed_count = diff_lines
        .iter()
        .filter(|line| line.starts_with('-'))
        .count();
    let added_count = diff_lines
        .iter()
        .filter(|line| line.starts_with('+'))
        .count();

    // Get context lines before the diff
    let mut context_before = Vec::new();
    for i in 1..=context_lines {
        if hunk_start_line > i {
            let idx = hunk_start_line - i - 1;
            let mut s = file_lines_before[idx].to_string();
            s.insert(0, ' ');
            context_before.push(s);
        }
    }
    context_before.reverse();

    // Get context lines after the diff
    let mut context_after = Vec::new();
    let end = context_lines - 1;
    for i in 0..=end {
        let idx = hunk_start_line + removed_count + i - 1;
        if idx <= file_lines_before.len() {
            let mut s = file_lines_before[idx].to_string();
            s.insert(0, ' ');
            context_after.push(s);
        }
    }

    let header = &diff_lines[0];
    let body = &diff_lines[1..];

    // Update unidiff header values
    let header = header
        .split(|c| c == ' ' || c == '@')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    let start_line_before = header[0].split(',').collect::<Vec<_>>()[0]
        .parse::<isize>()
        .context("failed to parse unidiff header value for start line before")?
        .unsigned_abs()
        - context_before.len();
    let line_count_before = removed_count + context_before.len() + context_after.len();
    let start_line_after = header[1].split(',').collect::<Vec<_>>()[0]
        .parse::<isize>()
        .context("failed to parse unidiff header value for start line after")?
        .unsigned_abs()
        - context_before.len();
    let line_count_after = added_count + context_before.len() + context_after.len();
    let header = format!(
        "@@ -{},{} +{},{} @@",
        start_line_before, line_count_before, start_line_after, line_count_after
    );

    // Update unidiff body with context lines
    let mut b = Vec::new();
    b.extend(context_before.clone());
    b.extend_from_slice(body);
    b.extend(context_after.clone());
    let body = b;

    // Construct a new diff with updated header and body
    let mut diff_lines = Vec::new();
    diff_lines.push(header);
    diff_lines.extend(body);
    let mut diff = diff_lines.join("\n");
    // Add trailing newline
    diff.push('\n');

    #[allow(clippy::cast_possible_truncation)]
    let hunk = diff::Hunk {
        diff,
        old_start: start_line_before as u32,
        old_lines: line_count_before as u32,
        new_start: start_line_after as u32,
        new_lines: line_count_after as u32,
        binary: is_binary,
    };
    Ok(hunk)
}
