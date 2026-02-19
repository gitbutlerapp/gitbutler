//! Various functions that involve launching the Git editor (i.e. `GIT_EDITOR`).
use std::ffi::OsStr;

use anyhow::{Result, bail};
use bstr::{BStr, BString, ByteSlice};

/// Launches the user's preferred text editor to edit some `initial_text`,
/// identified by a `filename_safe_intent` to help the user understand what's wanted of them.
/// Note that this string must be valid in filenames.
///
/// Returns the edited text (*without known encoding*), with comment lines (starting with `#`) removed.
pub fn from_editor_no_comments(filename_safe_intent: &str, initial_text: &str) -> Result<BString> {
    let content = from_editor(filename_safe_intent, initial_text, ".txt")?;

    // Strip comment lines (starting with '#')
    let filtered_lines: Vec<&BStr> = content
        .lines_with_terminator()
        .filter(|line| !line.trim_start().starts_with_str("#"))
        .map(|line| line.as_bstr())
        .collect();

    Ok(filtered_lines.into_iter().collect())
}

/// Launches the user's preferred text editor to edit some `initial_text`,
/// identified by a `filename_safe_intent` to help the user understand what's wanted of them.
/// Note that this string must be valid in filenames.
///
/// Returns the edited text (*without known encoding*) verbatim.
pub fn from_editor(filename_safe_intent: &str, initial_text: &str, file_suffix: &str) -> Result<BString> {
    const ALLOWED_SUFFIXES: &[&str] = &[".txt", ".md"]; // feel free to add more allowed suffixes
    if !ALLOWED_SUFFIXES.contains(&file_suffix) {
        bail!(
            "File suffix '{}' is not allowed. Must be one of: {}",
            file_suffix,
            ALLOWED_SUFFIXES.join(", ")
        );
    }

    let editor_cmd = get_editor_command()?;

    // Create a temporary file with the initial text
    let tempfile = tempfile::Builder::new()
        .prefix(&format!("but_{filename_safe_intent}_"))
        .suffix(file_suffix)
        .tempfile()?;
    std::fs::write(&tempfile, initial_text)?;

    // The editor command is allowed to be a shell expression, e.g. "code --wait" is somewhat common.
    // We need to execute within a shell to make sure we don't get "No such file or directory" errors.
    let status = gix::command::prepare(editor_cmd)
        .arg(tempfile.path())
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .with_shell()
        .spawn()?
        .wait()?;

    if !status.success() {
        bail!("Editor exited with non-zero status");
    }
    Ok(std::fs::read(&tempfile)?.into())
}

/// Get the user's preferred editor command.
/// Runs `git var GIT_EDITOR`, which lets git do its resolution of the editor command.
/// This typically uses the git config value for `core.editor`, and env vars like `GIT_EDITOR` or `EDITOR`.
/// We fall back to notepad (Windows) or vi otherwise just in case we don't get something usable from `git var`.
///
/// Note: Because git config parsing is used, the current directory matters for potential local git config overrides.
pub fn get_editor_command() -> Result<String> {
    get_editor_command_impl(std::env::vars_os())
}

/// Internal implementation that can be tested with the controlled environment `env`.
fn get_editor_command_impl<AsOsStr: AsRef<OsStr>>(env: impl IntoIterator<Item = (AsOsStr, AsOsStr)>) -> Result<String> {
    // Run git var with the controlled environment
    let mut cmd = std::process::Command::new(gix::path::env::exe_invocation());
    let res = cmd.args(["var", "GIT_EDITOR"]).env_clear().envs(env).output();
    if res.is_err() {
        // Avoid logging explicit env vars
        cmd.env_clear();
        tracing::warn!(
            ?res,
            ?cmd,
            "Git could not be invoked even though we expect this to work"
        );
    }
    if let Ok(output) = res
        && output.status.success()
    {
        let editor = output.stdout.as_bstr().trim();
        if !editor.is_empty() {
            return Ok(editor.as_bstr().to_string());
        }
    }
    // fallback to platform defaults to have *something*.
    Ok(PLATFORM_EDITOR.into())
}

pub const HTML_COMMENT_START_MARKER: &str = "<!--";
pub const HTML_COMMENT_END_MARKER: &str = "-->";

pub fn strip_html_comments(s: &str) -> String {
    let comment_start_positions = s.match_indices(HTML_COMMENT_START_MARKER).map(|(pos, _)| pos);
    let mut comment_end_positions = s.match_indices(HTML_COMMENT_END_MARKER).map(|(pos, _)| pos);

    let comment_ranges = comment_start_positions.map(|start| {
        comment_end_positions
            .find(|end| end > &start)
            .map(|end| (start, end + HTML_COMMENT_END_MARKER.len()))
    });

    let mut result = String::new();
    let mut last_end = 0;
    for (start, end) in comment_ranges.map_while(|range| range) {
        result.push_str(&s[last_end..start]);
        last_end = end;
    }
    result.push_str(&s[last_end..]);

    result
}

const PLATFORM_EDITOR: &str = if cfg!(windows) { "notepad" } else { "vi" };

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use super::*;

    #[test]
    fn git_editor_takes_precedence() {
        let git_editor_env = Some(("GIT_EDITOR", "from-GIT_EDITOR"));
        let actual = get_editor_command_impl(git_editor_env).unwrap();
        assert_eq!(
            actual, "from-GIT_EDITOR",
            "GIT_EDITOR should take precedence if git is executed correctly"
        );
    }

    #[test]
    fn falls_back_when_nothing_set() {
        // Empty environment, git considers this "dumb terminal" and `git var` will return empty string
        // so our own fallback will be used
        let no_env = None::<(String, String)>;
        let actual = get_editor_command_impl(no_env).unwrap();
        assert!(
            ["notepad", "vim", "vi"].contains(&actual.as_str()),
            "Should fall back to vi/vim/notepad when nothing is set, got: {actual}"
        );
    }

    #[test]
    fn from_editor_bails_on_bad_suffix() {
        // Note: Need to run this test with a timeout, as if we get past the suffix check for
        // whatever reason, everything may just hang waiting for user input in an editor.
        //
        // The controlling terminal tends to go insane when this test fails, but at least it
        // doesn't hang forever :)
        let (tx, rx) = std::sync::mpsc::channel();
        thread::spawn(move || tx.send(from_editor("filename", "", ".notasuffix")));
        let err = rx
            .recv_timeout(Duration::from_secs(1))
            .expect("Test timed out after 1 second")
            .unwrap_err();
        assert!(
            err.to_string().contains("is not allowed"),
            "Expected 'is not allowed' error, got: {err}"
        );
    }

    #[test]
    fn strip_html_comments_removes_all_html_comments() {
        let input = "
This should remain<!-- but not this -->, and so should this.
<!--This
entire
block should go -->
And this should stay here!
";
        let expected_output = "
This should remain, and so should this.

And this should stay here!
";

        let stripped = strip_html_comments(input);

        assert_eq!(stripped, expected_output)
    }

    #[test]
    fn strip_html_comments_does_not_remove_unterminated_html_comment() {
        let input = "
This should remain<!-- and so should this
as there's a start of a comment, but no end!
";

        let stripped = strip_html_comments(input);

        assert_eq!(stripped, input)
    }

    #[test]
    fn strip_html_comments_does_not_remove_comment_termination_without_start() {
        let input = "
This should remain<!-- but this should be stripped -->
but this comment terminator should remain --> as it has no associated start token
";
        let expected_output = "
This should remain
but this comment terminator should remain --> as it has no associated start token
";

        let stripped = strip_html_comments(input);

        assert_eq!(stripped, expected_output)
    }

    #[test]
    fn strip_html_comments_correctly_strips_comments_that_start_within_another_comment() {
        let input = "
This should remain<!-- but this should be stripped
<!-- along with this comment marker, which means nothing as it's in the middle of a comment -->
";
        let expected_output = "
This should remain
";

        let stripped = strip_html_comments(input);

        assert_eq!(stripped, expected_output)
    }
}
