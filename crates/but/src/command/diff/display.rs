use but_core::{UnifiedPatch, ui::TreeChange};
use colored::Colorize;
use std::fmt::Display;

use crate::command::legacy::status::{path_with_color_ui, status_letter_ui};

#[derive(Debug)]
pub(crate) struct TreeChangeWithPatch {
    change: TreeChange,
    patch: Option<UnifiedPatch>,
}

impl TreeChangeWithPatch {
    pub fn new(change: TreeChange, patch: Option<UnifiedPatch>) -> Self {
        Self { change, patch }
    }
}

impl Display for TreeChangeWithPatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = status_letter_ui(&self.change.status);
        let path = path_with_color_ui(&self.change.status, self.change.path_bytes.to_string());

        writeln!(f, " {status} {path}")?;

        if let Some(patch) = &self.patch {
            match patch {
                UnifiedPatch::Binary => {
                    writeln!(f, "   {}", "Binary file - no diff available".dimmed())?;
                }
                UnifiedPatch::TooLarge { size_in_bytes } => {
                    writeln!(
                        f,
                        "   {}",
                        format!(
                            "File too large ({} bytes) - no diff available",
                            size_in_bytes
                        )
                        .dimmed()
                    )?;
                }
                UnifiedPatch::Patch {
                    hunks,
                    is_result_of_binary_to_text_conversion,
                    lines_added,
                    lines_removed,
                } => {
                    if *is_result_of_binary_to_text_conversion {
                        writeln!(
                            f,
                            "   {}",
                            "(diff generated from binary-to-text conversion)".yellow()
                        )?;
                    }

                    for hunk in hunks {
                        self.fmt_hunk(f, hunk)?;
                    }

                    writeln!(
                        f,
                        "   {} {}",
                        format!("+{} -{}", lines_added, lines_removed).dimmed(),
                        format!("({} added, {} removed)", lines_added, lines_removed).dimmed()
                    )?;
                }
            }
        }
        Ok(())
    }
}

impl TreeChangeWithPatch {
    fn fmt_hunk(
        &self,
        f: &mut std::fmt::Formatter<'_>,
        hunk: &but_core::unified_diff::DiffHunk,
    ) -> std::fmt::Result {
        use bstr::ByteSlice;

        // Print hunk header
        writeln!(
            f,
            "   {}",
            format!(
                "@@ -{},{} +{},{} @@",
                hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines
            )
            .cyan()
            .bold()
        )?;

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
                    writeln!(f, "   {}", formatted_line)?;
                    new_line += 1;
                }
                '-' => {
                    // Removed line: show old line number, blank new line number
                    let line_nums = format!("{:>width$} {:>width$}", old_line, "", width = width);
                    let formatted_line = format!("{}│-{}", line_nums, content_str).red();
                    writeln!(f, "   {}", formatted_line)?;
                    old_line += 1;
                }
                ' ' => {
                    // Context line: show both line numbers
                    let line_nums =
                        format!("{:>width$} {:>width$}", old_line, new_line, width = width);
                    writeln!(f, "   {}│ {}", line_nums.dimmed(), content_str)?;
                    old_line += 1;
                    new_line += 1;
                }
                _ => unreachable!(),
            }
        }

        Ok(())
    }
}
