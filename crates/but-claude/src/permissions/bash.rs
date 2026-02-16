/// Splits a bash command string into individual commands, respecting quotes, escape sequences,
/// and heredocs. Splits on: &&, ||, ;, |, &, and newlines (when not inside quotes or heredocs)
///
/// # Examples
/// ```
/// # use but_claude::permissions::bash::split_bash_commands;
/// let result = split_bash_commands(r#"echo "hello world" && ls -la"#);
/// assert_eq!(result, vec!["echo \"hello world\"", "ls -la"]);
///
/// let result = split_bash_commands("git add . && git commit -m 'test commit' && git push");
/// assert_eq!(result, vec!["git add .", "git commit -m 'test commit'", "git push"]);
/// ```
pub(super) fn split_bash_commands(command: &str) -> Vec<String> {
    #[derive(Debug, Clone, PartialEq)]
    enum State {
        Normal,
        InSingleQuote,
        InDoubleQuote,
        Escaped,
        /// Heredoc state tracking delimiter and current line for efficient matching.
        /// strip_tabs is true for <<- heredocs where leading tabs on the closing line are ignored.
        InHeredoc {
            delimiter: String,
            strip_tabs: bool,
            current_line: String,
        },
    }

    /// Checks if a line matches the heredoc delimiter, accounting for <<- tab stripping.
    fn line_matches_delimiter(line: &str, delimiter: &str, strip_tabs: bool) -> bool {
        let line_to_check = if strip_tabs {
            line.trim_start_matches('\t')
        } else {
            line
        };
        line_to_check == delimiter
    }

    /// Parses a heredoc delimiter after `<<` or `<<-`, handling optional quotes.
    /// Returns the unquoted delimiter string, or None if empty.
    fn parse_heredoc_delimiter(
        chars: &mut std::iter::Peekable<std::str::Chars>,
        current_command: &mut String,
    ) -> Option<String> {
        // Skip whitespace before delimiter
        while matches!(chars.peek(), Some(' ' | '\t')) {
            current_command.push(chars.next().unwrap());
        }

        let mut delimiter = String::new();
        let mut in_quote = false;
        let mut quote_char = ' ';

        while let Some(&c) = chars.peek() {
            if !in_quote && matches!(c, '\'' | '"') {
                in_quote = true;
                quote_char = c;
                current_command.push(chars.next().unwrap());
            } else if in_quote && c == quote_char {
                in_quote = false;
                current_command.push(chars.next().unwrap());
            } else if !in_quote && matches!(c, ' ' | '\t' | '\n' | ';' | '&' | '|') {
                break;
            } else {
                delimiter.push(c);
                current_command.push(chars.next().unwrap());
            }
        }

        (!delimiter.is_empty()).then_some(delimiter)
    }

    let mut commands = Vec::new();
    let mut current_command = String::new();
    let mut state = State::Normal;
    let mut chars = command.chars().peekable();

    while let Some(ch) = chars.next() {
        match &mut state {
            State::Escaped => {
                current_command.push(ch);
                state = State::Normal;
            }
            State::InSingleQuote => {
                current_command.push(ch);
                if ch == '\'' {
                    state = State::Normal;
                }
            }
            State::InDoubleQuote => {
                current_command.push(ch);
                if ch == '\\' && matches!(chars.peek(), Some('"' | '\\' | '$' | '`' | '\n')) {
                    current_command.push(chars.next().unwrap());
                } else if ch == '"' {
                    state = State::Normal;
                }
            }
            State::InHeredoc {
                delimiter,
                strip_tabs,
                current_line,
            } => {
                current_command.push(ch);

                if ch == '\n' {
                    if line_matches_delimiter(current_line, delimiter, *strip_tabs) {
                        state = State::Normal;
                    } else {
                        current_line.clear();
                    }
                } else {
                    current_line.push(ch);

                    // Check for heredoc terminated by end-of-input or operator (no trailing newline)
                    if line_matches_delimiter(current_line, delimiter, *strip_tabs) {
                        // Skip optional whitespace, then check for operator or end
                        while matches!(chars.peek(), Some(' ' | '\t')) {
                            current_command.push(chars.next().unwrap());
                        }
                        if matches!(chars.peek(), None | Some('&' | '|' | ';')) {
                            state = State::Normal;
                        }
                    }
                }
            }
            State::Normal => {
                match ch {
                    '\'' => {
                        current_command.push(ch);
                        state = State::InSingleQuote;
                    }
                    '"' => {
                        current_command.push(ch);
                        state = State::InDoubleQuote;
                    }
                    '\\' => {
                        current_command.push(ch);
                        state = State::Escaped;
                    }
                    '<' => {
                        current_command.push(ch);
                        if chars.peek() == Some(&'<') {
                            current_command.push(chars.next().unwrap());
                            let strip_tabs = if chars.peek() == Some(&'-') {
                                current_command.push(chars.next().unwrap());
                                true
                            } else {
                                false
                            };
                            if let Some(delimiter) = parse_heredoc_delimiter(&mut chars, &mut current_command) {
                                state = State::InHeredoc {
                                    delimiter,
                                    strip_tabs,
                                    current_line: String::new(),
                                };
                            }
                        }
                    }
                    '>' => {
                        current_command.push(ch);
                        // Handle `2>&1` style syntax which should be considered
                        // part of the one command.
                        if chars.peek() == Some(&'&') {
                            current_command.push(chars.next().unwrap());
                        }
                    }
                    // Check for two-character operators
                    '&' => {
                        if chars.peek() == Some(&'&') {
                            chars.next(); // consume second &
                            let trimmed = current_command.trim().to_string();
                            if !trimmed.is_empty() {
                                commands.push(trimmed);
                            }
                            current_command.clear();
                        } else if chars.peek() == Some(&'>') {
                            // Handle &> style pipe redirection syntax
                            current_command.push(ch);
                            current_command.push(chars.next().unwrap());
                        } else {
                            // Single & is also a separator (background process)
                            let trimmed = current_command.trim().to_string();
                            if !trimmed.is_empty() {
                                commands.push(trimmed);
                            }
                            current_command.clear();
                        }
                    }
                    '|' => {
                        if chars.peek() == Some(&'|') {
                            chars.next(); // consume second |
                            let trimmed = current_command.trim().to_string();
                            if !trimmed.is_empty() {
                                commands.push(trimmed);
                            }
                            current_command.clear();
                        } else {
                            // Single | is a pipe, which we also split on
                            let trimmed = current_command.trim().to_string();
                            if !trimmed.is_empty() {
                                commands.push(trimmed);
                            }
                            current_command.clear();
                        }
                    }
                    ';' | '\n' => {
                        let trimmed = current_command.trim().to_string();
                        if !trimmed.is_empty() {
                            commands.push(trimmed);
                        }
                        current_command.clear();
                    }
                    _ => {
                        current_command.push(ch);
                    }
                }
            }
        }
    }

    // Don't forget the last command
    let trimmed = current_command.trim().to_string();
    if !trimmed.is_empty() {
        commands.push(trimmed);
    }

    commands
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_command() {
        let result = split_bash_commands("ls -la");
        assert_eq!(result, vec!["ls -la"]);
    }

    #[test]
    fn double_ampersand_separator() {
        let result = split_bash_commands("git add . && git commit -m 'message'");
        assert_eq!(result, vec!["git add .", "git commit -m 'message'"]);
    }

    #[test]
    fn double_pipe_separator() {
        let result = split_bash_commands("command1 || command2");
        assert_eq!(result, vec!["command1", "command2"]);
    }

    #[test]
    fn semicolon_separator() {
        let result = split_bash_commands("echo hello; echo world");
        assert_eq!(result, vec!["echo hello", "echo world"]);
    }

    #[test]
    fn single_pipe_separator() {
        let result = split_bash_commands("cat file.txt | grep pattern");
        assert_eq!(result, vec!["cat file.txt", "grep pattern"]);
    }

    #[test]
    fn single_ampersand_separator() {
        let result = split_bash_commands("long_command &");
        assert_eq!(result, vec!["long_command"]);
    }

    #[test]
    fn newline_separator() {
        let result = split_bash_commands("command1\ncommand2\ncommand3");
        assert_eq!(result, vec!["command1", "command2", "command3"]);
    }

    #[test]
    fn double_quotes_with_separator() {
        let result = split_bash_commands(r#"echo "hello && world" && echo "done""#);
        assert_eq!(result, vec![r#"echo "hello && world""#, r#"echo "done""#]);
    }

    #[test]
    fn single_quotes_with_separator() {
        let result = split_bash_commands("echo 'foo || bar' || echo 'baz'");
        assert_eq!(result, vec!["echo 'foo || bar'", "echo 'baz'"]);
    }

    #[test]
    fn escaped_quote_in_double_quotes() {
        let result = split_bash_commands(r#"echo "say \"hello\"" && ls"#);
        assert_eq!(result, vec![r#"echo "say \"hello\"""#, "ls"]);
    }

    #[test]
    fn backslash_escape_outside_quotes() {
        let result = split_bash_commands(r"echo hello\ world && echo done");
        assert_eq!(result, vec![r"echo hello\ world", "echo done"]);
    }

    #[test]
    fn complex_command_chain() {
        let result = split_bash_commands(r#"cd /tmp && touch "test file.txt" && echo "created" || echo "failed""#);
        assert_eq!(
            result,
            vec![
                "cd /tmp",
                r#"touch "test file.txt""#,
                r#"echo "created""#,
                r#"echo "failed""#
            ]
        );
    }

    #[test]
    fn mixed_quotes() {
        let result = split_bash_commands(r#"echo "double" && echo 'single' && echo plain"#);
        assert_eq!(result, vec![r#"echo "double""#, "echo 'single'", "echo plain"]);
    }

    #[test]
    fn empty_commands_filtered() {
        let result = split_bash_commands("  &&  command1  &&  && command2  ");
        assert_eq!(result, vec!["command1", "command2"]);
    }

    #[test]
    fn whitespace_trimmed() {
        let result = split_bash_commands("  command1  &&   command2  ");
        assert_eq!(result, vec!["command1", "command2"]);
    }

    #[test]
    fn pipe_in_quotes_not_split() {
        let result = split_bash_commands(r#"echo "pipe | here" && echo done"#);
        assert_eq!(result, vec![r#"echo "pipe | here""#, "echo done"]);
    }

    #[test]
    fn semicolon_in_quotes_not_split() {
        let result = split_bash_commands(r#"echo "semi;colon" && echo done"#);
        assert_eq!(result, vec![r#"echo "semi;colon""#, "echo done"]);
    }

    #[test]
    fn ampersand_in_single_quotes_not_split() {
        let result = split_bash_commands("echo 'foo & bar' && echo baz");
        assert_eq!(result, vec!["echo 'foo & bar'", "echo baz"]);
    }

    #[test]
    fn nested_quotes_different_types() {
        let result = split_bash_commands(r#"echo "it's working" && echo 'say "hi"'"#);
        assert_eq!(result, vec![r#"echo "it's working""#, r#"echo 'say "hi"'"#]);
    }

    #[test]
    fn real_world_git_workflow() {
        let result = split_bash_commands(
            r#"git add . && git commit -m "$(cat <<'EOF'
fix: update permissions system

Added quote-aware splitting
EOF
)" && git push"#,
        );
        assert_eq!(
            result,
            vec![
                "git add .",
                r#"git commit -m "$(cat <<'EOF'
fix: update permissions system

Added quote-aware splitting
EOF
)""#,
                "git push"
            ]
        );
    }

    #[test]
    fn command_with_backticks() {
        let result = split_bash_commands("echo `date` && echo done");
        // Note: backticks are not currently treated as quotes in our simple parser
        // This test documents current behavior
        assert_eq!(result, vec!["echo `date`", "echo done"]);
    }

    #[test]
    fn escaped_newline_in_double_quotes() {
        let result = split_bash_commands("echo \"line1\\nline2\" && echo done");
        assert_eq!(result, vec!["echo \"line1\\nline2\"", "echo done"]);
    }

    #[test]
    fn bash_multi_line_string() {
        let result = split_bash_commands(
            r#"cat << AStartEndIndicator
hello world
what is going on?
something odd
AStartEndIndicator"#,
        );

        assert_eq!(
            result,
            vec![
                r#"cat << AStartEndIndicator
hello world
what is going on?
something odd
AStartEndIndicator"#,
            ]
        );
    }

    #[test]
    fn heredoc_strip_tabs() {
        // <<- allows the closing delimiter to be indented with tabs
        let result = split_bash_commands("cat <<-EOF\n\thello world\n\tEOF");

        assert_eq!(result, vec!["cat <<-EOF\n\thello world\n\tEOF"]);
    }

    #[test]
    fn heredoc_strip_tabs_followed_by_command() {
        // <<- heredoc followed by another command
        let result = split_bash_commands("cat <<-EOF\n\tindented content\n\tEOF && echo done");

        assert_eq!(result, vec!["cat <<-EOF\n\tindented content\n\tEOF", "echo done"]);
    }

    #[test]
    fn heredoc_simple_eof() {
        // Basic unquoted heredoc with EOF delimiter
        let result = split_bash_commands("cat <<EOF\nhello\nEOF");
        assert_eq!(result, vec!["cat <<EOF\nhello\nEOF"]);
    }

    #[test]
    fn heredoc_followed_by_ampersand() {
        // Heredoc followed by && operator
        let result = split_bash_commands("cat <<EOF\ndata\nEOF && echo done");
        assert_eq!(result, vec!["cat <<EOF\ndata\nEOF", "echo done"]);
    }

    #[test]
    fn heredoc_followed_by_pipe() {
        // Heredoc followed by || operator
        let result = split_bash_commands("cat <<EOF\ndata\nEOF || echo failed");
        assert_eq!(result, vec!["cat <<EOF\ndata\nEOF", "echo failed"]);
    }

    #[test]
    fn heredoc_followed_by_semicolon() {
        // Heredoc followed by semicolon
        let result = split_bash_commands("cat <<EOF\ndata\nEOF; echo next");
        assert_eq!(result, vec!["cat <<EOF\ndata\nEOF", "echo next"]);
    }

    #[test]
    fn heredoc_delimiter_not_at_line_start() {
        // EOF appearing mid-line should not end the heredoc
        let result = split_bash_commands("cat <<EOF\nthis has EOF in the middle\nEOF");
        assert_eq!(result, vec!["cat <<EOF\nthis has EOF in the middle\nEOF"]);
    }

    #[test]
    fn heredoc_quoted_delimiter() {
        // Quoted delimiter (disables variable expansion in bash, but delimiter is still unquoted when matching)
        let result = split_bash_commands("cat <<'END'\n$VAR stays literal\nEND");
        assert_eq!(result, vec!["cat <<'END'\n$VAR stays literal\nEND"]);
    }

    #[test]
    fn heredoc_double_quoted_delimiter() {
        // Double-quoted delimiter
        let result = split_bash_commands("cat <<\"END\"\ncontent\nEND");
        assert_eq!(result, vec!["cat <<\"END\"\ncontent\nEND"]);
    }

    #[test]
    fn heredoc_strip_tabs_content_preserved() {
        // <<- only strips tabs from the closing delimiter line, content tabs are preserved
        let result = split_bash_commands("cat <<-EOF\n\tline1\n\t\tline2\n\tEOF");
        assert_eq!(result, vec!["cat <<-EOF\n\tline1\n\t\tline2\n\tEOF"]);
    }

    #[test]
    fn heredoc_strip_tabs_non_tab_indent_not_matched() {
        // <<- only strips tabs, not spaces - delimiter with space indent won't match
        let result = split_bash_commands("cat <<-EOF\ncontent\n  EOF\nEOF");
        // The "  EOF" (space-indented) doesn't match, but "EOF" does
        assert_eq!(result, vec!["cat <<-EOF\ncontent\n  EOF\nEOF"]);
    }

    #[test]
    fn heredoc_empty_content() {
        // Heredoc with no content between start and delimiter
        let result = split_bash_commands("cat <<EOF\nEOF");
        assert_eq!(result, vec!["cat <<EOF\nEOF"]);
    }

    #[test]
    fn heredoc_at_end_of_input() {
        // Heredoc that ends exactly at end of input (no trailing newline)
        let result = split_bash_commands("cat <<EOF\ncontent\nEOF");
        assert_eq!(result, vec!["cat <<EOF\ncontent\nEOF"]);
    }

    #[test]
    fn multiple_heredocs() {
        // Multiple heredocs in sequence
        let result = split_bash_commands("cat <<A\nfirst\nA && cat <<B\nsecond\nB");
        assert_eq!(result, vec!["cat <<A\nfirst\nA", "cat <<B\nsecond\nB"]);
    }

    #[test]
    fn stderr_redirection() {
        let result = split_bash_commands("pnpm check 2>&1");
        assert_eq!(result, vec!["pnpm check 2>&1"]);
        let result = split_bash_commands("pnpm check >&");
        assert_eq!(result, vec!["pnpm check >&"]);
    }

    #[test]
    fn stderr_redirection2() {
        let result = split_bash_commands("pnpm check 2&>1");
        assert_eq!(result, vec!["pnpm check 2&>1"]);
        let result = split_bash_commands("pnpm check &>");
        assert_eq!(result, vec!["pnpm check &>"]);
    }
}
