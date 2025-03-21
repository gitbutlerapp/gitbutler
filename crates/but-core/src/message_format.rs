//! Message formatting utilities for GitButler commit messages.
//!
//! This module handles the formatting and parsing of commit messages,
//! allowing for a standardized format in the Git repository while providing
//! UI-friendly versions for editing.

/// Wrap a line of text at 72 characters, preserving list items and quotes.
/// - everything is indented by <indent> spaces
/// - leading is prepended to everything other than the first line
fn wrap_line(line: &str, leading: Option<String>, indent: Option<usize>) -> String {
    let leading_spaces = line.len() - line.trim_start().len();
    let words: Vec<&str> = line.split_whitespace().collect();
    let mut lines = 0;

    let mut result = String::new();
    let mut current_line = String::new();

    if leading_spaces > 0 {
        for _ in 0..leading_spaces {
            result.push(' ');
        }
    }

    let mut current_indent = String::new();
    if let Some(indent) = indent {
        for _ in 0..indent {
            current_indent.push(' ');
        }
    }

    for (j, word) in words.iter().enumerate() {
        if current_line.is_empty() {
            current_line = word.to_string();
        } else if current_line.len() + word.len() + 1 > 72 {
            // Line would be too long, start a new line
            if lines > 0 {
                if let Some(ref leading_str) = leading {
                    result.push_str(leading_str);
                }
            }
            result.push_str(&current_line);
            result.push('\n');
            result.push_str(&current_indent);
            lines += 1;
            current_line = word.to_string();
        } else {
            // Add word to current line
            current_line.push(' ');
            current_line.push_str(word);
        }

        // If this is the last word and we're not at the end of the input
        if j == words.len() - 1 {
            if lines > 0 {
                if let Some(ref leading_str) = leading {
                    result.push_str(leading_str);
                }
            }
            result.push_str(current_line.trim_end());
            current_line.clear();
        }
    }

    result
}

// turn a multi-line quote into a single line
// join all lines, remove leading > chars
fn quote_unwrap(paragraph: &str) -> String {
    let mut result = String::new();

    let lines: Vec<&str> = paragraph.split('\n').collect();
    let mut j = 0;

    while j < lines.len() {
        let mut line = lines[j];

        // preserve indentation of first line
        if j == 0 {
            let leading_spaces = line.len() - line.trim_start().len();
            for _ in 0..leading_spaces {
                result.push(' ');
            }
        }

        line = line.trim();

        if line.starts_with(">") && j > 0 {
            result.push_str(line.trim_start_matches(">").trim());
        } else {
            result.push_str(line);
        }
        result.push(' ');
        j += 1;
    }
    result.trim_end().to_string()
}

fn bullet_unwrap(paragraph: &str) -> String {
    let possible_bullets = ["*", "-", "+"];

    let mut result = String::new();
    let lines: Vec<&str> = paragraph.split('\n').collect();
    let mut j = 0;
    while j < lines.len() {
        let line = lines[j];

        // if it starts with any of the possible bullets, start a new line
        if possible_bullets
            .iter()
            .any(|bullet| line.trim().starts_with(bullet))
        {
            if j > 0 {
                result = result.trim_end().to_string();
                result.push('\n');
            }
            result.push_str(line);
        } else {
            // it's a continuation of the last bullet
            result.push_str(line.trim());
        }

        result.push(' ');

        j += 1;
    }
    result.trim_end().to_string()
}

fn simple_unwrap(paragraph: &str) -> String {
    let mut result = String::new();
    let lines: Vec<&str> = paragraph.split('\n').collect();
    let trailer_regex = regex::Regex::new(r"^[!-9;-~]+:\s*.*$").unwrap();

    // Process each line in the paragraph
    let mut j = 0;
    while j < lines.len() {
        let line = lines[j];

        // if it's a trailer (RFC 822 grammar), add it to the result
        if trailer_regex.is_match(line.trim()) {
            result.push_str(line);
            result.push('\n');
        } else {
            result.push_str(line);
            result.push(' ');
        }
        j += 1;
    }
    result.trim_end().to_string()
}

/// Format a user-provided message for storage in a commit.
pub fn format_for_commit(message: String) -> String {
    // Split the message into paragraphs
    let paragraphs: Vec<&str> = message.split("\n\n").collect();

    if paragraphs.is_empty() {
        return String::new();
    }

    // Keep the first line as is, this is the subject line
    let mut result = paragraphs[0].to_string();
    result.push('\n');
    result.push('\n');

    let mut code_block = false;

    // Format the rest of the message with hard wrapping text paragraphs at 72 chars
    if paragraphs.len() > 1 {
        // Process remaining paragraphs
        let mut i = 1;

        while i < paragraphs.len() {
            let paragraph = paragraphs[i];

            let lines: Vec<&str> = paragraph.split('\n').collect();

            // Process each line in the paragraph
            let mut x = 0;
            while x < lines.len() {
                let line = lines[x];

                if line.starts_with("```") {
                    code_block = !code_block;
                    result.push_str(line);
                } else if code_block || line.len() <= 72 {
                    result.push_str(line);
                } else {
                    // is this a list item or quote?
                    let is_list_item = line.trim().starts_with("* ");
                    let is_quote = line.trim().starts_with("> ");
                    if is_list_item || is_quote {
                        let leading_spaces = line.len() - line.trim_start().len();
                        if is_list_item {
                            result.push_str(&wrap_line(line, None, Some(leading_spaces + 2)));
                        } else {
                            result.push_str(&wrap_line(
                                line,
                                Some("> ".to_string()),
                                Some(leading_spaces),
                            ));
                        }
                    } else {
                        result.push_str(&wrap_line(line, None, None));
                    }
                }
                x += 1; // next line in paragraph
                if x < lines.len() {
                    result.push('\n');
                }
            }
            i += 1; // next paragraph
            result.push('\n');
            result.push('\n');
        }
    }

    result.trim_end().to_string()
}

/// Parse a commit message back into its user-editable form.
pub fn parse_for_ui(formatted_message: &str) -> String {
    // Split the message into paragraphs
    let paragraphs: Vec<&str> = formatted_message.split("\n\n").collect();

    if paragraphs.is_empty() {
        return String::new();
    }

    let mut code_block = false;

    // Keep the first line as is, this is the subject line
    let mut result = paragraphs[0].to_string();
    result.push('\n');
    result.push('\n');

    if paragraphs.len() > 1 {
        // Process remaining paragraphs
        let mut i = 1;

        while i < paragraphs.len() {
            let paragraph = paragraphs[i];

            // is this a list item or quote?
            let is_list_item = paragraph.trim().starts_with("* ");
            let is_quote = paragraph.trim().starts_with(">");
            let starts_code_block = paragraph.trim().starts_with("```");
            let ends_code_block = paragraph.trim().ends_with("```");

            if starts_code_block {
                code_block = !code_block;
            }

            if code_block {
                result.push_str(paragraph);
            } else if is_list_item {
                result.push_str(&bullet_unwrap(paragraph));
            } else if is_quote {
                result.push_str(&quote_unwrap(paragraph));
            } else {
                result.push_str(&simple_unwrap(paragraph));
            }

            if ends_code_block {
                code_block = !code_block;
            }

            i += 1; // next paragraph
            result.push('\n');
            result.push('\n');
        }
    }

    result.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_message_roundtrip() {
        let original = "Add new feature".to_string();
        let formatted = format_for_commit(original.clone());
        let parsed = parse_for_ui(&formatted);
        assert_eq!(parsed, original);
    }

    #[test]
    fn test_simple_roundtrip() {
        let original = r#"Add new feature

This is a new feature that we are adding to the project.

It is a great feature that will help us to do great things.

We are very excited to add it to the project."#
            .to_string();
        let formatted = format_for_commit(original.clone());
        let parsed = parse_for_ui(&formatted);
        assert_eq!(parsed, original);
    }

    #[test]
    fn test_parse_wrapped_paragraphs_roundtrip() {
        let formatted = r#"Short subject line

This is a very long line that exceeds the 72 character limit and should
be wrapped at word boundaries."#;
        let parsed = parse_for_ui(formatted);
        let expected= "Short subject line\n\nThis is a very long line that exceeds the 72 character limit and should be wrapped at word boundaries.".to_string();
        assert_eq!(parsed, expected);
        let formatted_again = format_for_commit(parsed);
        assert_eq!(formatted_again, formatted);
    }

    #[test]
    fn test_format_with_short_line_paragraphs_and_trailers() {
        let message = r#"Short subject line

this is a series
of rather
short lines
where just one of them is longer and should be wrapped at word boundaries because it is a super long sentence that is longer than 72 characters.

The rest of the lines are short and should not be wrapped.

Signed-off-by: John Doe <john.doe@example.com>
Co-authored-by: Jane Doe <jane.doe@example.com>
Reviewed-by: Alice Smith <alice.smith@example.com>"#.to_string();
        let formatted = format_for_commit(message.clone());
        let expected = r#"Short subject line

this is a series
of rather
short lines
where just one of them is longer and should be wrapped at word
boundaries because it is a super long sentence that is longer than 72
characters.

The rest of the lines are short and should not be wrapped.

Signed-off-by: John Doe <john.doe@example.com>
Co-authored-by: Jane Doe <jane.doe@example.com>
Reviewed-by: Alice Smith <alice.smith@example.com>"#
            .to_string();
        assert_eq!(formatted, expected);

        // if it's incorrectly formatted, it will fix it coming back
        let parsed = parse_for_ui(&formatted);
        let ui_expected = r#"Short subject line

this is a series of rather short lines where just one of them is longer and should be wrapped at word boundaries because it is a super long sentence that is longer than 72 characters.

The rest of the lines are short and should not be wrapped.

Signed-off-by: John Doe <john.doe@example.com>
Co-authored-by: Jane Doe <jane.doe@example.com>
Reviewed-by: Alice Smith <alice.smith@example.com>"#
            .to_string();
        assert_eq!(parsed, ui_expected);
    }

    #[test]
    fn test_format_with_quotes_and_bullets() {
        let message = r#"Short subject line

Starting paragraph that is pretty long and should be wrapped at word boundaries because it is a super long sentence that is longer than 72 characters.

* here are some bullets
  * this is a sub-bullet that is longer and should be wrapped at word boundaries because it is a super long sentence that is longer than 72 characters.
* they are awesome
* one of them is very long and should be wrapped at word boundaries because it is a super long sentence
* one more

The next section is a quote

> This is a quote that is longer and should be wrapped at word boundaries because it is a super long sentence that is longer than 72 characters.

  > This is a quote that is longer and should be wrapped at word boundaries because it is a super long sentence that is longer than 72 characters.

https://u.gitbutler.com/websites/2f6dbf62-091f-4e57-bc47-7b1c6611a98b?url=%2Fgitbutlers-new-patch-based-code-review

The rest of the lines are short and should not be wrapped."#.to_string();
        let formatted = format_for_commit(message.clone());
        let expected = r#"Short subject line

Starting paragraph that is pretty long and should be wrapped at word
boundaries because it is a super long sentence that is longer than 72
characters.

* here are some bullets
  * this is a sub-bullet that is longer and should be wrapped at word
    boundaries because it is a super long sentence that is longer than 72
    characters.
* they are awesome
* one of them is very long and should be wrapped at word boundaries
  because it is a super long sentence
* one more

The next section is a quote

> This is a quote that is longer and should be wrapped at word
> boundaries because it is a super long sentence that is longer than 72
> characters.

  > This is a quote that is longer and should be wrapped at word
  > boundaries because it is a super long sentence that is longer than 72
  > characters.

https://u.gitbutler.com/websites/2f6dbf62-091f-4e57-bc47-7b1c6611a98b?url=%2Fgitbutlers-new-patch-based-code-review

The rest of the lines are short and should not be wrapped."#;

        assert_eq!(formatted, expected);

        let parsed_expected = r#"Short subject line

Starting paragraph that is pretty long and should be wrapped at word boundaries because it is a super long sentence that is longer than 72 characters.

* here are some bullets
  * this is a sub-bullet that is longer and should be wrapped at word boundaries because it is a super long sentence that is longer than 72 characters.
* they are awesome
* one of them is very long and should be wrapped at word boundaries because it is a super long sentence
* one more

The next section is a quote

> This is a quote that is longer and should be wrapped at word boundaries because it is a super long sentence that is longer than 72 characters.

  > This is a quote that is longer and should be wrapped at word boundaries because it is a super long sentence that is longer than 72 characters.

https://u.gitbutler.com/websites/2f6dbf62-091f-4e57-bc47-7b1c6611a98b?url=%2Fgitbutlers-new-patch-based-code-review

The rest of the lines are short and should not be wrapped."#.to_string();

        let parsed = parse_for_ui(&formatted);
        assert_eq!(parsed, parsed_expected);
    }

    #[test]
    fn test_format_with_code_blocks() {
        let message = r#"Short subject line

Starting paragraph that is pretty long and should be wrapped at word boundaries because it is a super long sentence that is longer than 72 characters.

```
  simple example

   can have newlines in them
```

Also a more complex example with a type

```ruby
def code
  def indeted_code
    # other random lines that have lines that can be pretty long and should NOT be wrapped at 72 but kept as they are
    random()
  end
end
```

The rest of the lines are short and should not be wrapped."#.to_string();
        let formatted = format_for_commit(message.clone());
        let expected = r#"Short subject line

Starting paragraph that is pretty long and should be wrapped at word
boundaries because it is a super long sentence that is longer than 72
characters.

```
  simple example

   can have newlines in them
```

Also a more complex example with a type

```ruby
def code
  def indeted_code
    # other random lines that have lines that can be pretty long and should NOT be wrapped at 72 but kept as they are
    random()
  end
end
```

The rest of the lines are short and should not be wrapped."#;

        assert_eq!(formatted, expected);

        let parsed = parse_for_ui(&formatted);
        assert_eq!(parsed, message);
    }
}
