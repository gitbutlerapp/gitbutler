use anyhow::Result;

/// Launches the user's preferred text editor to edit some initial text,
/// identified by a unique identifier (to avoid temp file collisions).
/// Returns the edited text, with comment lines (starting with '#') removed.
pub fn get_text_from_editor_no_comments(identifier: &str, initial_text: &str) -> Result<String> {
    let content = get_text_from_editor(identifier, initial_text)?;

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
pub fn get_text_from_editor(identifier: &str, initial_text: &str) -> Result<String> {
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

fn get_editor_command() -> Result<String> {
    // Try $EDITOR first
    if let Ok(editor) = std::env::var("EDITOR") {
        return Ok(editor);
    }

    // Try git config core.editor
    if let Ok(output) = std::process::Command::new("git")
        .args(["config", "--get", "core.editor"])
        .output()
        && output.status.success()
    {
        let editor = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !editor.is_empty() {
            return Ok(editor);
        }
    }

    // Fallback to platform defaults
    #[cfg(windows)]
    return Ok("notepad".to_string());

    #[cfg(not(windows))]
    return Ok("vi".to_string());
}
