//! Writing the bundled skill files into an install directory.

use anyhow::{Context as _, Result};

use crate::format::{
    CONCEPTS_MD, EXAMPLES_MD, REFERENCE_MD, SKILL_FILES, SKILL_MD, skill_files_in_write_order,
};

/// Replace version in SKILL.md content
pub fn inject_version(content: &str, version: &str) -> String {
    // Handle different line endings (Unix \n, Windows \r\n, or old Mac \r)
    let frontmatter_end = content
        .find("---\n\n")
        .or_else(|| content.find("---\r\n\r\n"))
        .or_else(|| content.find("---\r\r"));

    if let Some(end_pos) = frontmatter_end {
        let frontmatter = &content[..end_pos];
        let rest = &content[end_pos..];
        let updated_frontmatter =
            frontmatter.replace("version: 0.0.0", &format!("version: {version}"));
        format!("{updated_frontmatter}{rest}")
    } else {
        // Fallback if frontmatter format is unexpected
        content.replace("version: 0.0.0", &format!("version: {version}"))
    }
}

/// Prepare SKILL.md content with version injection and validate all files
pub fn prepare_skill_content(version: &str) -> Result<String> {
    // Validate all embedded files are valid UTF-8
    let skill_content = std::str::from_utf8(SKILL_MD).context("SKILL.md is not valid UTF-8")?;
    std::str::from_utf8(CONCEPTS_MD).context("concepts.md is not valid UTF-8")?;
    std::str::from_utf8(EXAMPLES_MD).context("examples.md is not valid UTF-8")?;
    std::str::from_utf8(REFERENCE_MD).context("reference.md is not valid UTF-8")?;

    // Inject version into SKILL.md
    Ok(inject_version(skill_content, version))
}

/// Write the bundled skill files into `install_path`, creating the directory
/// structure as needed and injecting the CLI version into SKILL.md. Returns the
/// version that was written.
pub fn write_skill_files(install_path: &std::path::Path) -> Result<&'static str> {
    if SKILL_FILES.iter().any(|f| f.content.is_empty()) {
        anyhow::bail!(
            "Skill files were not properly embedded at build time. Please report this as a bug."
        );
    }

    // Prepare all content before writing (validate UTF-8 and inject version)
    let version = crate::cli_version();
    let skill_md_content = prepare_skill_content(version)?;

    let references_dir = install_path.join("references");
    std::fs::create_dir_all(&references_dir).with_context(|| {
        format!(
            "Failed to create skill directory at {}. Check that you have write permissions for this location.",
            install_path.display()
        )
    })?;

    for file in skill_files_in_write_order() {
        let file_path = file.get_install_path(install_path);
        let content = if file.is_main_skill_file() {
            // Use the version-injected content for SKILL.md
            skill_md_content.as_bytes()
        } else {
            file.content
        };
        write_skill_file(&file_path, content, file.display_name)?;
    }
    Ok(version)
}

/// Write a skill file with proper error context
fn write_skill_file(path: &std::path::Path, content: &[u8], name: &str) -> Result<()> {
    std::fs::write(path, content).with_context(|| {
        format!(
            "Failed to write {} to {}. Check write permissions.",
            name,
            path.parent()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| path.display().to_string())
        )
    })
}

/// What [`remove_skill_files`] managed to clean up.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "outcome")]
pub enum RemovalOutcome {
    /// The skill files and their directory are gone.
    Removed,
    /// The skill files are gone, but the directory still holds files we did
    /// not put there, so it was left in place.
    PartiallyRemoved {
        /// Names of the entries left behind, for the UI to report.
        remaining: Vec<String>,
    },
}

/// Remove an installed GitButler skill from `install_path`.
///
/// Refuses anything whose `SKILL.md` does not identify it as ours, using the
/// same check discovery uses — so uninstall and discovery can never disagree
/// about what belongs to GitButler.
///
/// Only the files this crate writes are deleted, and the directory itself is
/// removed non-recursively. If the user kept notes alongside the skill, or a
/// newer version wrote a file this one does not know about, the directory
/// survives and is reported via [`RemovalOutcome::PartiallyRemoved`]. That is
/// the difference between uninstalling a skill and deleting a user's folder.
///
/// The parent `skills/` directory belongs to the agent, not to us, and is
/// never touched.
pub fn remove_skill_files(install_path: &std::path::Path) -> Result<RemovalOutcome> {
    if !crate::status::is_gitbutler_skill(&install_path.join("SKILL.md")) {
        anyhow::bail!(
            "Refusing to remove {}: it does not contain a GitButler skill.",
            install_path.display()
        );
    }

    for file in SKILL_FILES {
        let path = file.get_install_path(install_path);
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err).with_context(|| format!("Failed to remove {}", path.display()));
            }
        }
    }

    // `references/` only ever holds our files, so an empty one goes too. A
    // non-empty one means the user put something there.
    let references = install_path.join("references");
    if references.is_dir() && dir_entry_names(&references)?.is_empty() {
        std::fs::remove_dir(&references)
            .with_context(|| format!("Failed to remove {}", references.display()))?;
    }

    let remaining = dir_entry_names(install_path)?;
    if !remaining.is_empty() {
        return Ok(RemovalOutcome::PartiallyRemoved { remaining });
    }

    std::fs::remove_dir(install_path)
        .with_context(|| format!("Failed to remove {}", install_path.display()))?;
    Ok(RemovalOutcome::Removed)
}

/// The names of everything directly inside `dir`, sorted for stable reporting.
fn dir_entry_names(dir: &std::path::Path) -> Result<Vec<String>> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .with_context(|| format!("Failed to read {}", dir.display()))?
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn install(dir: &std::path::Path) -> std::path::PathBuf {
        let path = dir.join(".claude").join("skills").join("gitbutler");
        write_skill_files(&path).unwrap();
        path
    }

    #[test]
    fn removes_an_installed_skill_and_its_directory() {
        let temp = tempfile::tempdir().unwrap();
        let path = install(temp.path());

        assert_eq!(remove_skill_files(&path).unwrap(), RemovalOutcome::Removed);
        assert!(!path.exists(), "the skill directory is gone");
        assert!(
            path.parent().unwrap().is_dir(),
            "the agent's own skills directory is left alone"
        );
    }

    /// Discovery accepts any folder name and identifies a skill by its
    /// frontmatter, so removal must refuse anything that is not ours — even
    /// when it sits exactly where a GitButler skill would.
    #[test]
    fn refuses_a_directory_that_is_not_a_gitbutler_skill() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("some-other-skill");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("SKILL.md"), "---\nname: something-else\n---\n").unwrap();

        let err = remove_skill_files(&path).unwrap_err();
        assert!(
            err.to_string()
                .contains("does not contain a GitButler skill"),
            "explains the refusal, got: {err}"
        );
        assert!(path.join("SKILL.md").is_file(), "nothing was deleted");
    }

    #[test]
    fn refuses_a_directory_with_no_skill_at_all() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("empty");
        std::fs::create_dir_all(&path).unwrap();

        assert!(remove_skill_files(&path).is_err());
        assert!(path.is_dir(), "the directory survives");
    }

    /// Anything the user added alongside the skill must survive, and the UI
    /// needs to be told so it can say the folder is still there.
    #[test]
    fn keeps_and_reports_files_we_did_not_write() {
        let temp = tempfile::tempdir().unwrap();
        let path = install(temp.path());
        std::fs::write(path.join("my-notes.md"), "personal").unwrap();

        assert_eq!(
            remove_skill_files(&path).unwrap(),
            RemovalOutcome::PartiallyRemoved {
                remaining: vec!["my-notes.md".to_string()],
            }
        );
        assert!(path.is_dir(), "the directory survives");
        assert!(path.join("my-notes.md").is_file(), "the note survives");
        assert!(!path.join("SKILL.md").exists(), "our files are gone");
    }

    #[test]
    fn keeps_a_references_directory_holding_foreign_files() {
        let temp = tempfile::tempdir().unwrap();
        let path = install(temp.path());
        std::fs::write(path.join("references").join("mine.md"), "personal").unwrap();

        assert_eq!(
            remove_skill_files(&path).unwrap(),
            RemovalOutcome::PartiallyRemoved {
                remaining: vec!["references".to_string()],
            }
        );
        assert!(path.join("references").join("mine.md").is_file());
        assert!(!path.join("references").join("concepts.md").exists());
    }
}
