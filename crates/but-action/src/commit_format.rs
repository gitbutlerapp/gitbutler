/// Formats a commit message to follow email RFC format with 72-character line wrapping.
///
/// This function:
/// - Preserves the subject line (first line) as-is
/// - Wraps body paragraphs to 72 characters where possible
/// - Preserves blank lines between paragraphs
/// - Preserves list items, numbered ones too, code between fences, trailers and special formatting
/// - Breaks at word boundaries when possible
pub fn format_commit_message(message: &str) -> String {
    let lines: Vec<&str> = message.lines().collect();

    if lines.is_empty() {
        return message.to_string();
    }

    let mut result = Vec::new();

    // First line (subject) is preserved as-is
    result.push(lines[0].to_string());

    let trailers = trailers_start(&lines);
    let mut fenced = false;
    // Process the rest of the message
    let mut i = 1;
    while i < lines.len() {
        let line = lines[i];

        // Code between fences and the trailers stay line by line, as written
        let fence = line.trim_start().starts_with("```");
        let kept = fenced || fence || i >= trailers;
        fenced ^= fence;
        if kept {
            result.push(line.to_string());
            i += 1;
            continue;
        }

        // Preserve blank lines
        if line.trim().is_empty() {
            result.push(String::new());
            i += 1;
            continue;
        }

        // Detect list items or special formatting (lines starting with -, *, #, etc.)
        if is_special_format(line) {
            result.push(line.to_string());
            i += 1;
            continue;
        }

        // Otherwise, collect lines into a paragraph and wrap
        let mut paragraph = line.to_string();
        i += 1;

        // Collect continuation lines (non-blank, non-special lines)
        while i < trailers && !lines[i].trim().is_empty() && !is_special_format(lines[i]) {
            paragraph.push(' ');
            paragraph.push_str(lines[i].trim());
            i += 1;
        }

        // Wrap the paragraph to 72 characters
        result.extend(wrap_paragraph(&paragraph, 72));
    }

    result.join("\n")
}

/// Checks if a line has special formatting that should be preserved
fn is_special_format(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with('-')
        || trimmed.starts_with('*')
        || trimmed.starts_with('#')
        || trimmed.starts_with('>')
        || trimmed.starts_with("```")
        || line.starts_with("    ") // Indented code blocks
        || is_numbered_item(trimmed)
}

/// Whether `line` starts a numbered list item, like `1. ` or `2) `.
fn is_numbered_item(line: &str) -> bool {
    line.split_once(['.', ')']).is_some_and(|(n, rest)| {
        !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) && rest.starts_with(' ')
    })
}

/// Where the trailers start, as `Signed-off-by:` and `Co-authored-by:`: the last paragraph
/// when every line of it is a `Key: value`, as Git reads them; otherwise past the end.
fn trailers_start(lines: &[&str]) -> usize {
    let end = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .map_or(0, |last| last + 1);
    let Some(blank) = lines[..end].iter().rposition(|line| line.trim().is_empty()) else {
        return lines.len();
    };
    let is_trailer = |line: &&str| {
        line.split_once(": ").is_some_and(|(key, _)| {
            !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
    };
    match lines[blank + 1..end].iter().all(is_trailer) {
        true => blank + 1,
        false => lines.len(),
    }
}

/// Wraps a paragraph to the specified width, breaking at word boundaries
fn wrap_paragraph(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current_line = String::new();
    // In characters, as they're read, not bytes
    let mut current_width = 0;

    for word in text.split_whitespace() {
        let word_width = word.chars().count();
        // If adding this word would exceed the width
        if !current_line.is_empty() && current_width + 1 + word_width > width {
            lines.push(current_line);
            current_line = word.to_string();
            current_width = word_width;
        } else {
            if !current_line.is_empty() {
                current_line.push(' ');
                current_width += 1;
            }
            current_line.push_str(word);
            current_width += word_width;
        }
    }

    // Add the last line if there's anything left
    if !current_line.is_empty() {
        lines.push(current_line);
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_simple_message() {
        let input = "Add user authentication\n\nThis commit adds a basic user authentication system to the application. It includes login and logout functionality with JWT tokens.";
        let output = format_commit_message(input);

        // Subject should be preserved
        assert!(output.starts_with("Add user authentication\n\n"));

        // Body lines should not exceed 80 characters
        for line in output.lines().skip(2) {
            if !line.is_empty() {
                assert!(line.len() <= 80, "Line too long: {line}");
            }
        }
    }

    #[test]
    fn test_preserve_list_items() {
        let input = "Fix multiple bugs\n\n- Fix authentication timeout\n- Fix database connection pool\n- Fix UI rendering issue";
        let output = format_commit_message(input);

        assert!(output.contains("- Fix authentication timeout"));
        assert!(output.contains("- Fix database connection pool"));
        assert!(output.contains("- Fix UI rendering issue"));
    }

    #[test]
    fn test_wrap_long_paragraph() {
        let input = "Add feature\n\nThis is a very long paragraph that definitely exceeds 72 characters and should be wrapped into multiple lines when formatted properly according to email RFC standards.";
        let output = format_commit_message(input);

        // Check that body lines don't exceed 72 characters
        for (i, line) in output.lines().enumerate() {
            if i > 1 && !line.is_empty() {
                assert!(
                    line.len() <= 72,
                    "Line {} too long: '{}' ({})",
                    i,
                    line,
                    line.len()
                );
            }
        }
    }

    #[test]
    fn test_preserve_blank_lines() {
        let input = "Add feature\n\nFirst paragraph.\n\nSecond paragraph.";
        let output = format_commit_message(input);

        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines[0], "Add feature");
        assert_eq!(lines[1], "");
        assert!(lines[2].starts_with("First"));
        assert_eq!(lines[3], "");
        assert!(lines[4].starts_with("Second"));
    }

    #[test]
    fn trailers_numbered_lists_and_code_stay_as_written() {
        let long = "word ".repeat(20);
        let input = format!(
            "Subject\n\n{long}\n\n1. first\n2. second\n\n```\nlet a = 1;\nlet b = 2;\n```\n\nSigned-off-by: A <a@x.io>\nCo-authored-by: B <b@x.io>"
        );
        let output = format_commit_message(&input);
        for kept in [
            "\n1. first\n2. second\n",
            "\n```\nlet a = 1;\nlet b = 2;\n```\n",
            "\nSigned-off-by: A <a@x.io>\nCo-authored-by: B <b@x.io>",
        ] {
            assert!(
                output.contains(kept),
                "{kept:?} is kept line by line in {output:?}"
            );
        }
    }

    #[test]
    fn width_counts_characters_not_bytes() {
        let input = format!("Subject\n\n{}", "한국어 ".repeat(20));
        let output = format_commit_message(&input);
        let widest = (output.lines().skip(2))
            .map(|line| line.chars().count())
            .max();
        assert!(
            widest.is_some_and(|w| w > 60),
            "lines of three-byte characters fill the width too: {output:?}"
        );
    }
}
