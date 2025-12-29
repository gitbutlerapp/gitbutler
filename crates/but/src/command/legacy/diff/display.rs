use but_core::{UnifiedPatch, ui, unified_diff::DiffHunk};
use but_hunk_assignment::HunkAssignment;
use colored::Colorize;

use crate::command::legacy::status::{path_with_color_ui, status_letter_ui};

/// Trait for types that can provide diff display information.
///
/// Implement this trait on your type to get nicely formatted diff output with:
/// - Colored line numbers (old and new line numbers in columns)
/// - Green text for added lines
/// - Red text for removed lines
/// - Dimmed text for context lines
/// - Support for binary files and large files
///
/// # Example
///
/// ```ignore
/// use crate::command::diff::display::DiffDisplay;
///
/// impl DiffDisplay for MyType {
///     fn print_diff(&self) -> String {
///         let mut output = String::new();
///         output.push_str(&format!(" {}\n", self.title));
///         // Add more diff formatting here
///         output
///     }
/// }
///
/// // Usage:
/// let diff_output = my_value.print_diff();
/// write!(out, "{}", diff_output)?;
/// ```
pub(crate) trait DiffDisplay {
    /// Format this diff and return it as a String.
    ///
    /// This method generates a nicely formatted diff with colored output.
    fn print_diff(&self) -> String;
}

#[derive(Debug)]
pub(crate) struct TreeChangeWithPatch {
    change: ui::TreeChange,
    patch: Option<UnifiedPatch>,
}

impl TreeChangeWithPatch {
    pub fn new(change: ui::TreeChange, patch: Option<UnifiedPatch>) -> Self {
        Self { change, patch }
    }
}

impl DiffDisplay for TreeChangeWithPatch {
    fn print_diff(&self) -> String {
        let mut output = String::new();
        let status = status_letter_ui(&self.change.status);
        let path = path_with_color_ui(&self.change.status, self.change.path_bytes.to_string());
        output.push_str(&format!(" {status} {path}\n"));

        if let Some(patch) = &self.patch {
            output.push_str(&format_patch(patch));
        }
        output
    }
}

/// Format a patch (UnifiedPatch) to a String.
///
/// This is a helper function for consistent patch formatting.
fn format_patch(patch: &UnifiedPatch) -> String {
    let mut output = String::new();
    match patch {
        UnifiedPatch::Binary => {
            output.push_str(&format!(
                "   {}\n",
                "Binary file - no diff available".dimmed()
            ));
        }
        UnifiedPatch::TooLarge { size_in_bytes } => {
            output.push_str(&format!(
                "   {}\n",
                format!(
                    "File too large ({} bytes) - no diff available",
                    size_in_bytes
                )
                .dimmed()
            ));
        }
        UnifiedPatch::Patch {
            hunks,
            is_result_of_binary_to_text_conversion,
            lines_added,
            lines_removed,
        } => {
            if *is_result_of_binary_to_text_conversion {
                output.push_str(&format!(
                    "   {}\n",
                    "(diff generated from binary-to-text conversion)".yellow()
                ));
            }

            for hunk in hunks {
                output.push_str(&fmt_hunk(hunk));
            }

            output.push_str(&format!(
                "   {} {}\n",
                format!("+{} -{}", lines_added, lines_removed).dimmed(),
                format!("({} added, {} removed)", lines_added, lines_removed).dimmed()
            ));
        }
    }
    output
}

fn fmt_hunk(hunk: &DiffHunk) -> String {
    use bstr::ByteSlice;

    let mut output = String::new();

    // Print hunk header
    output.push_str(&format!(
        "   {}\n",
        format!(
            "@@ -{},{} +{},{} @@",
            hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines
        )
        .cyan()
        .bold()
    ));

    // Track line numbers for old and new versions
    let mut old_line = hunk.old_start;
    let mut new_line = hunk.new_start;

    // Calculate the width needed for line numbers
    let max_old_line = hunk.old_start + hunk.old_lines;
    let max_new_line = hunk.new_start + hunk.new_lines;
    let width = std::cmp::max(
        max_old_line.to_string().len(),
        max_new_line.to_string().len(),
    );

    for line in hunk.diff.lines() {
        if line.is_empty() {
            continue;
        }

        let (prefix, content) = if let Some(rest) = line.strip_prefix(b"+") {
            ('+', rest)
        } else if let Some(rest) = line.strip_prefix(b"-") {
            ('-', rest)
        } else if let Some(rest) = line.strip_prefix(b" ") {
            (' ', rest)
        } else {
            // Shouldn't happen in well-formed diffs, but handle it
            (' ', line)
        };

        let content_str = content.to_str_lossy();

        match prefix {
            '+' => {
                // Added line: show blank old line number, show new line number
                let line_nums = format!("{:>width$} {:>width$}", "", new_line, width = width);
                let formatted_line = format!("{}│+{}", line_nums, content_str).green();
                output.push_str(&format!("   {}\n", formatted_line));
                new_line += 1;
            }
            '-' => {
                // Removed line: show old line number, blank new line number
                let line_nums = format!("{:>width$} {:>width$}", old_line, "", width = width);
                let formatted_line = format!("{}│-{}", line_nums, content_str).red();
                output.push_str(&format!("   {}\n", formatted_line));
                old_line += 1;
            }
            ' ' => {
                // Context line: show both line numbers
                let line_nums = format!("{:>width$} {:>width$}", old_line, new_line, width = width);
                output.push_str(&format!("   {}│ {}\n", line_nums.dimmed(), content_str));
                old_line += 1;
                new_line += 1;
            }
            _ => unreachable!(),
        }
    }

    output
}

impl DiffDisplay for HunkAssignment {
    fn print_diff(&self) -> String {
        let mut output = String::new();

        // Write the header - show path and optionally the hunk header
        if let Some(header) = &self.hunk_header {
            output.push_str(&format!(
                " {} @@ -{},{} +{},{} @@\n",
                self.path.bright_white(),
                header.old_start,
                header.old_lines,
                header.new_start,
                header.new_lines
            ));
        } else {
            // Binary, too large, or whole file
            output.push_str(&format!(" {}\n", self.path.bright_white()));
        }

        // Check if we have diff data to display
        if let (Some(diff), Some(header)) = (&self.diff, &self.hunk_header) {
            // We have a real hunk to display
            let hunk = DiffHunk {
                old_start: header.old_start,
                old_lines: header.old_lines,
                new_start: header.new_start,
                new_lines: header.new_lines,
                diff: diff.clone(),
            };
            output.push_str(&fmt_hunk(&hunk));

            // Show line count summary
            let added_count = self.line_nums_added.as_ref().map_or(0, |v| v.len());
            let removed_count = self.line_nums_removed.as_ref().map_or(0, |v| v.len());
            output.push_str(&format!(
                "   {} {}\n",
                format!("+{} -{}", added_count, removed_count).dimmed(),
                format!("({} added, {} removed)", added_count, removed_count).dimmed()
            ));
        } else if self.hunk_header.is_none() {
            // Binary, too large, or whole file without detailed diff
            output.push_str(&format!("   {}\n", "(no detailed diff available)".dimmed()));
        }

        output
    }
}
