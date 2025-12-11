//! Various functions that involve launching the editor.
use anyhow::Result;
use std::collections::HashMap;

/// Launches the user's preferred text editor to edit some initial text,
/// identified by a unique identifier (to avoid temp file collisions).
/// Returns the edited text, with comment lines (starting with '#') removed.
pub fn from_editor_no_comments(identifier: &str, initial_text: &str) -> Result<String> {
    let content = from_editor(identifier, initial_text)?;

    // Strip comment lines (starting with '#')
    let filtered_lines: Vec<&str> = content
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect();

    Ok(filtered_lines.join("\n").trim().to_string())
}

/// Launches the user's preferred text editor to edit some initial text,
/// identified by a unique identifier (to avoid temp file collisions).
/// Returns the edited text.
pub fn from_editor(identifier: &str, initial_text: &str) -> Result<String> {
    let editor_cmd = get_editor_command()?;

    // Create a temporary file with the initial text
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("{}_{}", identifier, std::process::id()));

    std::fs::write(&temp_file, initial_text)?;

    // The editor command is allowed to be a shell expression, e.g. "code --wait" is somewhat common.
    // We need to execute within a shell to make sure we don't get "No such file or directory" errors.
    let status = gix::command::prepare(editor_cmd)
        .arg(&temp_file)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .with_shell()
        .spawn()?
        .wait()?;

    if !status.success() {
        return Err(anyhow::anyhow!("Editor exited with non-zero status"));
    }

    // Read the edited text back
    let edited_text = std::fs::read_to_string(&temp_file)?;
    std::fs::remove_file(&temp_file).ok(); // Best effort to clean up
    Ok(edited_text)
}

/// Get the user's preferred editor command.
/// Runs `git var GIT_EDITOR`, which lets git do its resolution of the editor command.
/// This typically uses the git config value for `core.editor`, and env vars like `GIT_EDITOR` or `EDITOR`.
/// We fallback to notepad (Windows) or vi otherwise just in case we don't get something usable from `git var`.
///
/// Note: Because git config parsing is used, the current directory matters for potential local git config overrides.
fn get_editor_command() -> Result<String> {
    let env: HashMap<String, String> = std::env::vars().collect();
    get_editor_command_impl(&env)
}

/// Internal implementation that can be tested with controlled environment
fn get_editor_command_impl(env: &HashMap<String, String>) -> Result<String> {
    // Run git var with the controlled environment
    if let Ok(output) = std::process::Command::new(gix::path::env::exe_invocation())
        .args(["var", "GIT_EDITOR"])
        .env_clear()
        .envs(env)
        .output()
        && output.status.success()
    {
        let editor = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !editor.is_empty() {
            return Ok(editor);
        }
    }

    // Simple fallback to platform defaults
    Ok(if cfg!(windows) { "notepad" } else { "vi" }.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUR_PLATFORM_DEFAULT: &str = if cfg!(windows) { "notepad" } else { "vi" };

    #[test]
    fn git_editor_takes_precedence() {
        let env = HashMap::from([("GIT_EDITOR".to_string(), "from-GIT_EDITOR".to_string())]);
        let actual = get_editor_command_impl(&env).unwrap();
        assert_eq!(
            actual, "from-GIT_EDITOR",
            "GIT_EDITOR should take precedence if git is executed correctly"
        );
    }

    #[test]
    fn falls_back_when_nothing_set() {
        // Empty environment, git considers this "dumb terminal" and `git var` will return empty string
        // so our own fallback will be used
        let env = HashMap::new();
        let actual = get_editor_command_impl(&env).unwrap();
        assert_eq!(
            actual, OUR_PLATFORM_DEFAULT,
            "Should fall back to vi/notepad when nothing is set"
        );
    }
}
