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
