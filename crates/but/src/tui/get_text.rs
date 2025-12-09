//! Various functions that involve launching the editor.
use anyhow::Result;

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

    // Launch the editor
    let status = std::process::Command::new(editor_cmd)
        .arg(&temp_file)
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("Editor exited with non-zero status"));
    }

    // Read the edited text back
    let edited_text = std::fs::read_to_string(&temp_file)?;
    std::fs::remove_file(&temp_file).ok(); // Best effort to clean up
    Ok(edited_text)
}

/// Checks if the terminal is dumb (TERM environment variable is "dumb")
fn is_terminal_dumb() -> bool {
    std::env::var("TERM")
        .map(|term| term == "dumb")
        .unwrap_or(false)
}

/// Implement get_editor_command to match Git's C implementation of `git_editor`.
///
/// - Check GIT_EDITOR environment variable first
/// - Check git config `core.editor` (editor_program)
/// - Check VISUAL if terminal is not dumb
/// - Check EDITOR
/// - Return error if terminal is dumb and no editor found
/// - Fall back to platform defaults (vi on Unix, notepad on Windows)
/// - Add comprehensive unit tests for all scenarios
fn get_editor_command() -> Result<String> {
    get_editor_command_impl(&|key| std::env::var(key), is_terminal_dumb())
}

/// Internal get_editor_command_implementation that can be tested without modifying environment
fn get_editor_command_impl<F>(env_var: &F, terminal_is_dumb: bool) -> Result<String>
where
    F: Fn(&str) -> Result<String, std::env::VarError>,
{
    // Try $GIT_EDITOR first
    if let Ok(editor) = env_var("GIT_EDITOR")
        && !editor.is_empty()
    {
        return Ok(editor);
    }

    // Try git config `core.editor` (editor_program)
    if let Ok(output) = std::process::Command::new(gix::path::env::exe_invocation())
        .args(["config", "--get", "core.editor"])
        .output()
        && output.status.success()
    {
        let editor = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !editor.is_empty() {
            return Ok(editor);
        }
    }

    // Try $VISUAL if terminal is not dumb
    if !terminal_is_dumb
        && let Ok(editor) = env_var("VISUAL")
        && !editor.is_empty()
    {
        return Ok(editor);
    }

    if let Ok(editor) = env_var("EDITOR")
        && !editor.is_empty()
    {
        return Ok(editor);
    }

    // If terminal is dumb and no editor was found, return an error
    if terminal_is_dumb {
        return Err(anyhow::anyhow!(
            "Terminal is dumb, but no editor specified in GIT_EDITOR, core.editor, or EDITOR"
        ));
    }

    // Fallback to platform defaults (DEFAULT_EDITOR)
    let default_editor = if cfg!(windows) { "notepad" } else { "vi" };
    Ok(default_editor.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // Helper to create a mock environment function from a hashmap
    fn mock_env(vars: HashMap<&str, &str>) -> impl Fn(&str) -> Result<String, std::env::VarError> {
        move |key: &str| {
            vars.get(key)
                .map(|v| v.to_string())
                .ok_or(std::env::VarError::NotPresent)
        }
    }

    #[test]
    fn git_editor_takes_precedence() {
        let env = mock_env(HashMap::from([
            ("GIT_EDITOR", "git-editor"),
            ("VISUAL", "visual-editor"),
            ("EDITOR", "editor"),
        ]));
        let actual = get_editor_command_impl(&env, false).unwrap();
        assert_eq!(actual, "git-editor");
    }

    #[test]
    fn visual_when_terminal_not_dumb() {
        let env = mock_env(visual_and_editor());
        let actual = get_editor_command_impl(&env, false).unwrap();
        assert_eq!(actual, "visual-editor");
    }

    #[test]
    fn skips_visual_when_terminal_dumb() {
        let env = mock_env(visual_and_editor());
        let actual = get_editor_command_impl(&env, true).unwrap();
        assert_eq!(actual, "editor", "Should skip VISUAL and use EDITOR");
    }

    #[test]
    fn uses_editor() {
        let env = mock_env(HashMap::from([("EDITOR", "editor")]));
        let actual = get_editor_command_impl(&env, false).unwrap();
        assert_eq!(actual, "editor");
    }

    #[test]
    fn fails_when_terminal_dumb_and_no_editor() {
        let env = mock_env(HashMap::new());
        let actual = get_editor_command_impl(&env, true);
        assert!(actual.is_err());
        assert!(actual.unwrap_err().to_string().contains("Terminal is dumb"));
    }

    #[test]
    fn ignores_empty_git_editor() {
        let env = mock_env(HashMap::from([
            ("GIT_EDITOR", ""),
            ("VISUAL", "visual-editor"),
            ("EDITOR", "editor"),
        ]));
        let actual = get_editor_command_impl(&env, false).unwrap();
        assert_eq!(
            actual, "visual-editor",
            "Empty GIT_EDITOR should be ignored, fall through to VISUAL"
        );
    }

    #[test]
    fn ignores_empty_visual() {
        let env = mock_env(HashMap::from([("VISUAL", ""), ("EDITOR", "editor")]));
        let actual = get_editor_command_impl(&env, false).unwrap();
        assert_eq!(
            actual, "editor",
            "Empty VISUAL should be ignored, fall through to EDITOR"
        );
    }

    #[test]
    fn ignores_empty_editor() {
        let env = mock_env(HashMap::from([("EDITOR", "")]));
        let actual = get_editor_command_impl(&env, false).unwrap();
        assert_eq!(
            actual, PLATFORM_DEFAULT,
            "Empty EDITOR should be ignored, fall back to default"
        );
    }

    #[test]
    fn falls_back_to_default_when_no_vars_set() {
        let env = mock_env(HashMap::new());
        let actual = get_editor_command_impl(&env, false).unwrap();
        assert_eq!(actual, PLATFORM_DEFAULT);
    }

    const PLATFORM_DEFAULT: &str = if cfg!(windows) { "notepad" } else { "vi" };

    pub fn visual_and_editor() -> HashMap<&'static str, &'static str> {
        HashMap::from([("VISUAL", "visual-editor"), ("EDITOR", "editor")])
    }
}
