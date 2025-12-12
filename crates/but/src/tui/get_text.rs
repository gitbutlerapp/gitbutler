//! Various functions that involve launching the Git editor (i.e. `GIT_EDITOR`).
use anyhow::Result;
use bstr::ByteSlice;
use std::ffi::OsStr;

/// Launches the user's preferred text editor to edit some `initial_text`,
/// identified by a `filename_safe_intent` to help the user understand what's wanted of them.
/// Note that this string must be valid in filenames.
///
/// Returns the edited text, with comment lines (starting with `#`) removed.
pub fn from_editor_no_comments(filename_safe_intent: &str, initial_text: &str) -> Result<String> {
    let content = from_editor(filename_safe_intent, initial_text)?;

    // Strip comment lines (starting with '#')
    let filtered_lines: Vec<&str> = content
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect();

    Ok(filtered_lines.join("\n").trim().to_string())
}

/// Launches the user's preferred text editor to edit some `initial_text`,
/// identified by a `filename_safe_intent` to help the user understand what's wanted of them.
/// Note that this string must be valid in filenames.
///
/// Returns the edited text verbatim.
pub fn from_editor(identifier: &str, initial_text: &str) -> Result<String> {
    let editor_cmd = get_editor_command()?;

    // Create a temporary file with the initial text
    let tempfile = tempfile::Builder::new()
        .prefix(&format!("but_{identifier}_"))
        .suffix(".txt")
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
        return Err(anyhow::anyhow!("Editor exited with non-zero status"));
    }
    Ok(std::fs::read_to_string(&tempfile)?)
}

/// Get the user's preferred editor command.
/// Runs `git var GIT_EDITOR`, which lets git do its resolution of the editor command.
/// This typically uses the git config value for `core.editor`, and env vars like `GIT_EDITOR` or `EDITOR`.
/// We fall back to notepad (Windows) or vi otherwise just in case we don't get something usable from `git var`.
///
/// Note: Because git config parsing is used, the current directory matters for potential local git config overrides.
fn get_editor_command() -> Result<String> {
    get_editor_command_impl(std::env::vars_os())
}

/// Internal implementation that can be tested with the controlled environment `env`.
fn get_editor_command_impl<AsOsStr: AsRef<OsStr>>(
    env: impl IntoIterator<Item = (AsOsStr, AsOsStr)>,
) -> Result<String> {
    // Run git var with the controlled environment
    let mut cmd = std::process::Command::new(gix::path::env::exe_invocation());
    let res = cmd
        .args(["var", "GIT_EDITOR"])
        .env_clear()
        .envs(env)
        .output();
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

const PLATFORM_EDITOR: &str = if cfg!(windows) { "notepad" } else { "vi" };

#[cfg(test)]
mod tests {
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
        assert_eq!(
            actual, PLATFORM_EDITOR,
            "Should fall back to vi/notepad when nothing is set"
        );
    }
}
