//! Various functions that involve launching the Git editor (i.e. `GIT_EDITOR`).
//!
//! When no external editor is configured, falls back to the built-in TUI editor.
use std::{ffi::OsStr, io::Write as _};

use anyhow::{Context as _, Result, bail};
use bstr::{BStr, BString, ByteSlice};

const REST_TEXT_MARKER: &str = "# --- ignore-rest ---";

/// Launches the user's preferred text editor to edit some `initial_text`,
/// identified by a `filename_safe_intent` to help the user understand what's wanted of them.
/// Note that this string must be valid in filenames.
///
/// Returns the edited text (*without known encoding*), with comment lines (starting with `#`) removed.
pub fn from_editor_no_comments(filename_safe_intent: &str, initial_text: &str) -> Result<BString> {
    let content = from_editor(filename_safe_intent, initial_text, None, ".txt")?;
    let filtered_lines = filter_content_from_editor(content.as_bstr());
    Ok(filtered_lines.into_iter().collect())
}

/// Like `from_editor_no_comments` but uses ".patch" file extension that enables syntax
/// highlighting in some editors.
///
/// If `diff_text` is `Some`, appends it after a `REST_TEXT_MARKER` separator line in the editor.
/// The marker and everything below it is automatically stripped from the returned text.
///
/// Returns the edited text (*without known encoding*), with comment lines (starting with `#`) removed.
pub fn from_editor_no_comments_as_patch(
    filename_safe_intent: &str,
    initial_text: &str,
    diff_text: Option<&str>,
) -> Result<BString> {
    let content = from_editor(filename_safe_intent, initial_text, diff_text, ".patch")?;
    let filtered_lines = filter_content_from_editor(content.as_bstr());
    Ok(filtered_lines.into_iter().collect())
}

/// Strip comment lines (starting with '#') and everything below `REST_TEXT_MARKER`.
fn filter_content_from_editor(content: &BStr) -> Vec<&BStr> {
    content
        .lines_with_terminator()
        .take_while(|line| !line.trim_start().starts_with_str(REST_TEXT_MARKER))
        .filter(|line| !line.trim_start().starts_with_str("#"))
        .map(|line| line.as_bstr())
        .collect()
}

/// Launches the user's preferred text editor to edit some `initial_text`,
/// identified by a `filename_safe_intent` to help the user understand what's wanted of them.
/// Note that this string must be valid in filenames.
///
/// If the user has an external editor configured (via `GIT_EDITOR`, `core.editor`, or `EDITOR`),
/// that editor is used. Otherwise, the built-in TUI editor is launched.
///
/// Returns the edited text (*without known encoding*) verbatim.
pub fn from_editor(
    filename_safe_intent: &str,
    initial_text: &str,
    rest_text: Option<&str>,
    file_suffix: &str,
) -> Result<BString> {
    const ALLOWED_SUFFIXES: &[&str] = &[".txt", ".md", ".patch"]; // feel free to add more allowed suffixes
    if !ALLOWED_SUFFIXES.contains(&file_suffix) {
        bail!(
            "File suffix '{}' is not allowed. Must be one of: {}",
            file_suffix,
            ALLOWED_SUFFIXES.join(", ")
        );
    }

    match get_editor_command() {
        Some(editor_cmd) => from_external_editor(
            &editor_cmd,
            filename_safe_intent,
            initial_text,
            rest_text,
            file_suffix,
        ),
        None => from_builtin_editor(filename_safe_intent, initial_text, rest_text),
    }
}

/// Launch an external editor (vim, code, etc.) to edit text via a temporary file.
fn from_external_editor(
    editor_cmd: &str,
    filename_safe_intent: &str,
    initial_text: &str,
    rest_text: Option<&str>,
    file_suffix: &str,
) -> Result<BString> {
    // Create a temporary file with the initial text
    let mut tempfile = tempfile::Builder::new()
        .prefix(&format!("but_{filename_safe_intent}_"))
        .suffix(file_suffix)
        .tempfile()?;

    write!(&mut tempfile, "{initial_text}")?;

    if let Some(rest_text) = rest_text {
        if !initial_text.ends_with('\n') {
            writeln!(&mut tempfile)?;
        }
        writeln!(&mut tempfile, "{REST_TEXT_MARKER}")?;
        writeln!(&mut tempfile, "{rest_text}")?;
    }

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

    Ok(std::fs::read(&tempfile)
        .context("failed to read contents of commit message file")?
        .into())
}

/// Launch the built-in TUI editor.
fn from_builtin_editor(
    filename_safe_intent: &str,
    initial_text: &str,
    rest_text: Option<&str>,
) -> Result<BString> {
    // Determine editor mode based on the intent
    let mode = if filename_safe_intent.contains("commit") {
        super::editor::EditorMode::CommitMessage
    } else if filename_safe_intent.contains("branch") {
        super::editor::EditorMode::BranchName
    } else {
        super::editor::EditorMode::PullRequest
    };

    let editor_output = if let Some(rest_text) = rest_text {
        let mut initial_text = initial_text.to_owned();
        if !initial_text.ends_with('\n') {
            initial_text.push('\n');
        }
        initial_text.push_str(REST_TEXT_MARKER);
        initial_text.push('\n');
        initial_text.push_str(rest_text);
        super::editor::run_builtin_editor(filename_safe_intent, &initial_text, mode)?
    } else {
        super::editor::run_builtin_editor(filename_safe_intent, initial_text, mode)?
    };
    match editor_output {
        Some(content) => Ok(content.into()),
        None => bail!("Editor cancelled"),
    }
}

/// Get the user's preferred editor command, if one is configured.
///
/// Runs `git var GIT_EDITOR`, which lets git do its resolution of the editor command.
/// This typically uses the git config value for `core.editor`, and env vars like `GIT_EDITOR` or `EDITOR`.
///
/// Returns `None` when no editor is configured, signalling that the built-in editor should be used.
///
/// Note: Because git config parsing is used, the current directory matters for potential local git config overrides.
pub fn get_editor_command() -> Option<String> {
    get_editor_command_impl(std::env::vars_os())
}

/// Internal implementation that can be tested with the controlled environment `env`.
///
/// Checks the standard Git editor sources in precedence order:
/// 1. `GIT_EDITOR` env var
/// 2. `core.editor` git config
/// 3. `VISUAL` env var
/// 4. `EDITOR` env var
///
/// Unlike `git var GIT_EDITOR`, this does NOT fall back to `vi` when nothing
/// is configured — it returns `None` so the caller can use the built-in editor.
fn get_editor_command_impl<AsOsStr: AsRef<OsStr>>(
    env: impl IntoIterator<Item = (AsOsStr, AsOsStr)>,
) -> Option<String> {
    let env: Vec<(String, String)> = env
        .into_iter()
        .filter_map(|(k, v)| {
            Some((
                k.as_ref().to_str()?.to_owned(),
                v.as_ref().to_str()?.to_owned(),
            ))
        })
        .collect();

    let lookup_env = |name: &str| -> Option<String> {
        env.iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .filter(|v| !v.is_empty())
    };

    // 1. GIT_EDITOR environment variable
    if let Some(editor) = lookup_env("GIT_EDITOR") {
        return Some(editor);
    }

    // 2. core.editor from git config
    {
        let mut cmd = std::process::Command::new(gix::path::env::exe_invocation());
        let res = cmd
            .args(["config", "core.editor"])
            .env_clear()
            .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .output();
        if let Ok(output) = res
            && output.status.success()
        {
            let editor = output.stdout.as_bstr().trim().as_bstr().to_string();
            if !editor.is_empty() {
                return Some(editor);
            }
        }
    }

    // 3. VISUAL environment variable
    if let Some(editor) = lookup_env("VISUAL") {
        return Some(editor);
    }

    // 4. EDITOR environment variable
    if let Some(editor) = lookup_env("EDITOR") {
        return Some(editor);
    }

    // No configured editor — the caller should use the built-in editor.
    None
}

pub const HTML_COMMENT_START_MARKER: &str = "<!--";
pub const HTML_COMMENT_END_MARKER: &str = "-->";

pub fn strip_html_comments(s: &str) -> String {
    let comment_start_positions = s
        .match_indices(HTML_COMMENT_START_MARKER)
        .map(|(pos, _)| pos);
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

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use super::*;

    #[test]
    fn git_editor_takes_precedence() {
        let env = vec![
            ("GIT_EDITOR", "from-GIT_EDITOR"),
            ("VISUAL", "from-VISUAL"),
            ("EDITOR", "from-EDITOR"),
        ];
        let actual = get_editor_command_impl(env);
        assert_eq!(actual.as_deref(), Some("from-GIT_EDITOR"));
    }

    #[test]
    fn visual_used_when_no_git_editor() {
        let env = vec![("VISUAL", "from-VISUAL"), ("EDITOR", "from-EDITOR")];
        let actual = get_editor_command_impl(env);
        assert_eq!(actual.as_deref(), Some("from-VISUAL"));
    }

    #[test]
    fn editor_used_as_last_env_fallback() {
        let env = vec![("EDITOR", "from-EDITOR")];
        let actual = get_editor_command_impl(env);
        assert_eq!(actual.as_deref(), Some("from-EDITOR"));
    }

    #[test]
    fn falls_back_to_builtin_when_nothing_set() {
        let no_env = None::<(String, String)>;
        let actual = get_editor_command_impl(no_env);
        assert!(
            actual.is_none(),
            "Should return None when no editor is configured, got: {actual:?}"
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
        thread::spawn(move || tx.send(from_editor("filename", "", None, ".notasuffix")));
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

    #[test]
    fn test_filter_content_from_editor() {
        let raw_content = BString::from(format!(
            r#"commit message

here is a longer description about the commit

1. It does the thing
2. It does the other thing

# this line will be ignored
# as will this
{REST_TEXT_MARKER}
all
this
will
be
ignored"#
        ));
        let filtered_content = filter_content_from_editor(raw_content.as_bstr());

        assert_eq!(
            filtered_content,
            Vec::from([
                "commit message\n",
                "\n",
                "here is a longer description about the commit\n",
                "\n",
                "1. It does the thing\n",
                "2. It does the other thing\n",
                "\n",
            ])
        );
    }
}
