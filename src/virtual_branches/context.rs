use crate::git::diff;

pub fn hunk_with_context(
    hunk_diff: &str,
    hunk_old_start_line: usize,
    hunk_new_start_line: usize,
    is_binary: bool,
    context_lines: usize,
    file_lines_before: &[&str],
    change_type: diff::ChangeType,
) -> diff::GitHunk {
    let diff_lines = hunk_diff
        .lines()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>();
    if diff_lines.is_empty() {
        #[allow(clippy::cast_possible_truncation)]
        return diff::GitHunk {
            diff: hunk_diff.to_owned(),
            old_start: hunk_old_start_line as u32,
            old_lines: 0,
            new_start: hunk_new_start_line as u32,
            new_lines: 0,
            binary: is_binary,
            change_type,
        };
    }

    let new_file = hunk_old_start_line == 0;
    let deleted_file = hunk_new_start_line == 0;

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
    let before_context_ending_index = if removed_count == 0 {
        // Compensate for when the removed_count is 0
        hunk_old_start_line
    } else {
        hunk_old_start_line.saturating_sub(1)
    };
    let before_context_starting_index = before_context_ending_index.saturating_sub(context_lines);

    for index in before_context_starting_index..before_context_ending_index {
        if let Some(l) = file_lines_before.get(index) {
            let mut s = (*l).to_string();
            s.insert(0, ' ');
            context_before.push(s);
        }
    }

    // Get context lines after the diff
    let mut context_after = Vec::new();
    let after_context_starting_index = before_context_ending_index + removed_count;
    let after_context_ending_index = after_context_starting_index + context_lines;

    for index in after_context_starting_index..after_context_ending_index {
        if let Some(l) = file_lines_before.get(index) {
            let mut s = (*l).to_string();
            s.insert(0, ' ');
            context_after.push(s);
        }
    }

    let start_line_before = if new_file {
        // If we've created a new file, start_line_before should be 0
        0
    } else {
        before_context_starting_index + 1
    };

    let start_line_after = if deleted_file {
        // If we've deleted a new file, start_line_after should be 0
        0
    } else if added_count == 0 {
        // Compensate for when the added_count is 0
        hunk_new_start_line.saturating_sub(context_before.len()) + 1
    } else {
        hunk_new_start_line.saturating_sub(context_before.len())
    };

    let line_count_before = removed_count + context_before.len() + context_after.len();
    let line_count_after = added_count + context_before.len() + context_after.len();
    let header = format!(
        "@@ -{},{} +{},{} @@",
        start_line_before, line_count_before, start_line_after, line_count_after
    );

    let body = &diff_lines[1..];
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
    let hunk = diff::GitHunk {
        diff,
        old_start: start_line_before as u32,
        old_lines: line_count_before as u32,
        new_start: start_line_after as u32,
        new_lines: line_count_after as u32,
        binary: is_binary,
        change_type,
    };

    hunk
}
