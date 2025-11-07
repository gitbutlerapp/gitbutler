/// Splits a bash command string into individual commands, respecting quotes and escape sequences.
/// Splits on: &&, ||, ;, |, &, and newlines (when not inside quotes)
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
    #[derive(Debug, Clone, Copy, PartialEq)]
    enum State {
        Normal,
        InSingleQuote,
        InDoubleQuote,
        Escaped,
    }

    let mut commands = Vec::new();
    let mut current_command = String::new();
    let mut state = State::Normal;
    let mut chars = command.chars().peekable();

    while let Some(ch) = chars.next() {
        match state {
            State::Escaped => {
                // In escaped state, add the character literally and return to previous state
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
                if ch == '\\' {
                    // In double quotes, backslash escapes certain characters
                    if let Some(&next_ch) = chars.peek()
                        && matches!(next_ch, '"' | '\\' | '$' | '`' | '\n')
                    {
                        current_command.push(chars.next().unwrap());
                    }
                } else if ch == '"' {
                    state = State::Normal;
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
                    // Check for two-character operators
                    '&' => {
                        if chars.peek() == Some(&'&') {
                            chars.next(); // consume second &
                            let trimmed = current_command.trim().to_string();
                            if !trimmed.is_empty() {
                                commands.push(trimmed);
                            }
                            current_command.clear();
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
        let result = split_bash_commands(
            r#"cd /tmp && touch "test file.txt" && echo "created" || echo "failed""#,
        );
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
        assert_eq!(
            result,
            vec![r#"echo "double""#, "echo 'single'", "echo plain"]
        );
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
}
