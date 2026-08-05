//! Tests for the skill file layout, install-path formats, and the frontmatter
//! parsing that decides whether a directory holds a GitButler skill.

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{
        format::{
            SKILL_FILES, SKILL_FORMATS, SkillFile, SkillFormat, SkillFormatAvailability,
            skill_files_in_write_order,
        },
        install::{inject_version, prepare_skill_content, write_skill_files},
        status::{
            extract_installed_version, extract_installed_version_from_content,
            find_format_installations, frontmatter_value, is_complete_skill_installation,
            is_gitbutler_skill, parse_yaml_value,
        },
    };

    #[test]
    fn inject_version_replaces_in_frontmatter() {
        let content = "---\nname: Test\nversion: 0.0.0\n---\n\nContent here with version: 0.0.0";
        let result = inject_version(content, "1.2.3");

        // Should replace the first occurrence in frontmatter
        assert!(result.contains("version: 1.2.3"));
        // The second occurrence should NOT be replaced
        assert!(result.contains("Content here with version: 0.0.0"));
    }

    #[test]
    fn inject_version_handles_windows_line_endings() {
        let content = "---\r\nname: Test\r\nversion: 0.0.0\r\n---\r\n\r\nContent here";
        let result = inject_version(content, "1.2.3");

        assert!(result.contains("version: 1.2.3"));
    }

    #[test]
    fn inject_version_handles_old_mac_line_endings() {
        let content = "---\rname: Test\rversion: 0.0.0\r---\r\rContent here";
        let result = inject_version(content, "1.2.3");

        assert!(result.contains("version: 1.2.3"));
    }

    #[test]
    fn inject_version_fallback_without_frontmatter() {
        let content = "Just some content with version: 0.0.0 in it";
        let result = inject_version(content, "2.0.0");

        assert!(result.contains("version: 2.0.0"));
        assert!(!result.contains("version: 0.0.0"));
    }

    #[test]
    fn inject_version_handles_missing_version_field() {
        let content = "---\nname: Test\n---\n\nContent";
        let result = inject_version(content, "1.0.0");

        // Should not crash, and content should be unchanged
        assert_eq!(content, result);
    }

    #[test]
    fn prepare_skill_content_validates_utf8() {
        // This tests that the function checks UTF-8 validity
        // The actual embedded files should be valid, so this should succeed
        let result = prepare_skill_content("1.0.0");
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }

    #[test]
    fn prepare_skill_content_injects_version() {
        let result = prepare_skill_content("9.9.9").unwrap();
        assert!(result.contains("version: 9.9.9"));
    }

    #[test]
    fn skill_formats_are_valid() {
        // Validate that all SKILL_FORMATS have non-empty fields
        assert!(!SKILL_FORMATS.is_empty(), "Must have at least one format");

        for format in SKILL_FORMATS {
            assert!(!format.name.is_empty(), "Format name cannot be empty");
            assert!(
                !format.description.is_empty(),
                "Format description cannot be empty"
            );
            assert!(
                !format.path_components.is_empty(),
                "Format path components cannot be empty"
            );
            assert!(
                format
                    .path_components
                    .iter()
                    .all(|component| !component.is_empty()),
                "Format path components must not be empty"
            );
            assert!(
                format
                    .path_components
                    .iter()
                    .all(|component| !component.contains('/') && !component.contains('\\')),
                "Format path components must not contain path separators"
            );
        }
    }

    #[test]
    fn skill_format_get_install_path_joins_correctly() {
        let format = SkillFormat {
            name: "Test",
            description: "Test format",
            availability: SkillFormatAvailability::LocalAndGlobal,
            path_components: &[".test", "skills", "foo"],
        };

        let base = PathBuf::from("home").join("user");
        let result = format.get_install_path(&base);

        assert_eq!(result, base.join(".test").join("skills").join("foo"));
    }

    #[test]
    fn skill_files_are_valid() {
        for file in SKILL_FILES {
            assert!(
                !file.path_components.is_empty(),
                "SkillFile path components cannot be empty"
            );
            assert!(
                file.path_components
                    .iter()
                    .all(|component| !component.is_empty()),
                "SkillFile path components must not be empty"
            );
            assert!(
                file.path_components
                    .iter()
                    .all(|component| !component.contains('/') && !component.contains('\\')),
                "SkillFile path components must not contain path separators"
            );
        }
    }

    #[test]
    fn embedded_files_are_not_empty() {
        // This catches build issues where files aren't properly embedded
        for file in SKILL_FILES {
            assert!(
                !file.content.is_empty(),
                "{} should be embedded",
                file.display_path()
            );
        }
    }

    #[test]
    fn embedded_files_are_valid_utf8() {
        // Ensure all embedded files are valid UTF-8
        for file in SKILL_FILES {
            assert!(
                std::str::from_utf8(file.content).is_ok(),
                "{} should be valid UTF-8",
                file.display_path()
            );
        }
    }

    #[test]
    fn skill_file_display_path_is_derived_from_components() {
        assert_eq!(SKILL_FILES[0].display_path(), "SKILL.md");
        assert_eq!(SKILL_FILES[1].display_path(), "references/concepts.md");
    }

    #[test]
    fn is_gitbutler_skill_requires_name_but_in_frontmatter() {
        use std::fs;

        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let skill_path = temp_dir.path().join("SKILL.md");

        // Identity comes from `name: but` in the YAML frontmatter.
        fs::write(
            &skill_path,
            "---\nname: but\nversion: 1.0.0\n---\n# GitButler CLI Skill",
        )
        .unwrap();
        assert!(
            is_gitbutler_skill(&skill_path),
            "frontmatter name: but is the identity marker"
        );

        // Another skill that only mentions GitButler in its header or body must not
        // be misclassified - discovery would otherwise overwrite it in place.
        fs::write(
            &skill_path,
            "---\nname: other-skill\n---\n# GitButler CLI Skill\n\nExample: `name: but`",
        )
        .unwrap();
        assert!(
            !is_gitbutler_skill(&skill_path),
            "a sibling skill's declared name wins over body mentions of GitButler"
        );

        // The header alone, with no frontmatter, is not a reliable marker.
        fs::write(&skill_path, "# GitButler CLI Skill\n\nContent here").unwrap();
        assert!(!is_gitbutler_skill(&skill_path));

        // Prose that merely contains the marker string.
        fs::write(
            &skill_path,
            "I was reading about the GitButler CLI and the name: but that's not right",
        )
        .unwrap();
        assert!(!is_gitbutler_skill(&skill_path));

        // Nonexistent file.
        assert!(!is_gitbutler_skill(&temp_dir.path().join("nonexistent.md")));
    }

    #[test]
    fn extract_installed_version_parses_frontmatter() {
        let version = extract_installed_version_from_content(
            "---\nname: but\nversion: 1.2.3\n---\n# Content",
        );
        assert_eq!(version, Some("1.2.3".to_string()));
    }

    #[test]
    fn extract_installed_version_handles_different_order() {
        // version is not the first field
        let version = extract_installed_version_from_content(
            "---\nname: but\nauthor: Test\nversion: 2.0.0\n---\n# Content",
        );
        assert_eq!(version, Some("2.0.0".to_string()));
    }

    #[test]
    fn extract_installed_version_returns_none_for_missing_version() {
        let version = extract_installed_version_from_content("---\nname: but\n---\n# Content");
        assert_eq!(version, None);
    }

    #[test]
    fn extract_installed_version_returns_none_for_no_frontmatter() {
        let version = extract_installed_version_from_content("# Just a regular markdown file");
        assert_eq!(version, None);
    }

    #[test]
    fn extract_installed_version_returns_none_for_nonexistent_file() {
        let nonexistent = PathBuf::from("/nonexistent/path/SKILL.md");
        let version = extract_installed_version(&nonexistent);
        assert_eq!(version, None);
    }

    #[test]
    fn skill_entrypoint_is_written_last() {
        assert!(
            skill_files_in_write_order()
                .last()
                .is_some_and(SkillFile::is_main_skill_file),
            "a partial bundle must not look installed"
        );
    }

    #[test]
    fn extract_installed_version_trims_whitespace() {
        // Version with extra whitespace
        let version =
            extract_installed_version_from_content("---\nversion:   1.0.0   \n---\n# Content");
        assert_eq!(version, Some("1.0.0".to_string()));
    }

    #[test]
    fn extract_installed_version_handles_empty_version() {
        // Empty version value
        let version = extract_installed_version_from_content("---\nversion:\n---\n# Content");
        assert_eq!(version, Some("".to_string()));
    }

    #[test]
    fn find_all_installations_discovers_skills_in_temp_dir() {
        use std::fs;

        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();

        // Create a Claude Code skill installation
        let claude_skill_dir = temp_dir
            .path()
            .join(".claude")
            .join("skills")
            .join("gitbutler");
        fs::create_dir_all(&claude_skill_dir).unwrap();
        let claude_skill_content = "---\nname: but\nversion: 1.0.0\n---\n# GitButler CLI Skill";
        fs::write(claude_skill_dir.join("SKILL.md"), claude_skill_content).unwrap();

        // Create a Cursor skill installation
        let cursor_skill_dir = temp_dir
            .path()
            .join(".cursor")
            .join("skills")
            .join("gitbutler");
        fs::create_dir_all(&cursor_skill_dir).unwrap();
        let cursor_skill_content = "---\nname: but\nversion: 0.9.0\n---\n# GitButler CLI Skill";
        fs::write(cursor_skill_dir.join("SKILL.md"), cursor_skill_content).unwrap();

        // Create a non-GitButler skill (should be ignored)
        let other_skill_dir = temp_dir
            .path()
            .join(".opencode")
            .join("skills")
            .join("gitbutler");
        fs::create_dir_all(&other_skill_dir).unwrap();
        fs::write(other_skill_dir.join("SKILL.md"), "# Some other skill").unwrap();

        // We can't easily test find_all_installations directly since it uses the user home.
        // But we can test the components it uses

        // Verify is_gitbutler_skill correctly identifies our test files
        assert!(is_gitbutler_skill(&claude_skill_dir.join("SKILL.md")));
        assert!(is_gitbutler_skill(&cursor_skill_dir.join("SKILL.md")));
        assert!(!is_gitbutler_skill(&other_skill_dir.join("SKILL.md")));

        // Verify extract_installed_version parsing works on our test content
        assert_eq!(
            extract_installed_version_from_content(claude_skill_content),
            Some("1.0.0".to_string())
        );
        assert_eq!(
            extract_installed_version_from_content(cursor_skill_content),
            Some("0.9.0".to_string())
        );
    }

    #[test]
    fn find_format_installations_accepts_any_folder_name() {
        use std::fs;

        let temp_dir = tempfile::TempDir::new().unwrap();
        let skills_dir = temp_dir.path().join(".claude").join("skills");

        // A GitButler skill under a custom folder name, plus one under the
        // canonical name - both are real installations.
        let gitbutler_skill = "---\nname: but\nversion: 1.0.0\n---\n# GitButler CLI Skill";
        for folder in ["but", "gitbutler"] {
            fs::create_dir_all(skills_dir.join(folder)).unwrap();
            fs::write(skills_dir.join(folder).join("SKILL.md"), gitbutler_skill).unwrap();
        }
        // Another agent's skill is ignored
        fs::create_dir_all(skills_dir.join("other")).unwrap();
        fs::write(
            skills_dir.join("other").join("SKILL.md"),
            "# Some other skill",
        )
        .unwrap();

        let format = SKILL_FORMATS
            .iter()
            .find(|f| f.name == "Claude Code")
            .unwrap();

        assert_eq!(
            find_format_installations(format, temp_dir.path()),
            vec![skills_dir.join("but"), skills_dir.join("gitbutler")],
            "every GitButler skill is found by SKILL.md contents, not folder name, in sorted order"
        );

        let format_without_dir = SKILL_FORMATS.iter().find(|f| f.name == "Cursor").unwrap();
        assert!(
            find_format_installations(format_without_dir, temp_dir.path()).is_empty(),
            "a missing skills directory yields no installations"
        );
    }

    #[test]
    fn skill_installation_requires_every_embedded_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        write_skill_files(temp_dir.path()).unwrap();
        std::fs::remove_file(temp_dir.path().join("references/concepts.md")).unwrap();

        assert!(!is_complete_skill_installation(temp_dir.path()));
    }

    #[test]
    fn extract_installed_version_stops_at_frontmatter_end() {
        // Version appears both in frontmatter and body - should only get frontmatter version
        let version = extract_installed_version_from_content(
            "---\nversion: 1.0.0\n---\n\nversion: 2.0.0 in the body",
        );
        assert_eq!(version, Some("1.0.0".to_string()));
    }

    #[test]
    fn frontmatter_value_handles_crlf_line_endings() {
        // Windows checkouts use CRLF. `str::lines()` strips the `\r\n` terminator,
        // so the `---` delimiters and keys still match without special handling.
        let content = "---\r\nname: but\r\nversion: 1.2.3\r\n---\r\n# GitButler CLI Skill";
        assert_eq!(frontmatter_value(content, "name:").as_deref(), Some("but"));
        assert_eq!(
            frontmatter_value(content, "version:").as_deref(),
            Some("1.2.3")
        );
    }

    #[test]
    fn parse_yaml_value_handles_plain_values() {
        assert_eq!(parse_yaml_value("1.0.0"), "1.0.0");
        assert_eq!(parse_yaml_value("  1.0.0  "), "1.0.0");
    }

    #[test]
    fn parse_yaml_value_handles_double_quoted_strings() {
        assert_eq!(parse_yaml_value("\"1.0.0\""), "1.0.0");
        assert_eq!(parse_yaml_value("  \"1.0.0\"  "), "1.0.0");
    }

    #[test]
    fn parse_yaml_value_handles_single_quoted_strings() {
        assert_eq!(parse_yaml_value("'1.0.0'"), "1.0.0");
        assert_eq!(parse_yaml_value("  '1.0.0'  "), "1.0.0");
    }

    #[test]
    fn parse_yaml_value_handles_inline_comments() {
        assert_eq!(parse_yaml_value("1.0.0 # this is a comment"), "1.0.0");
        assert_eq!(
            parse_yaml_value("1.0.0  # comment with extra space"),
            "1.0.0"
        );
    }

    #[test]
    fn parse_yaml_value_handles_quoted_with_comment() {
        // Comment after quoted value
        assert_eq!(parse_yaml_value("\"1.0.0\" # comment"), "1.0.0");
    }

    #[test]
    fn extract_installed_version_handles_quoted_version() {
        let version =
            extract_installed_version_from_content("---\nversion: \"1.2.3\"\n---\n# Content");
        assert_eq!(version, Some("1.2.3".to_string()));
    }

    #[test]
    fn extract_installed_version_handles_version_with_comment() {
        let version = extract_installed_version_from_content(
            "---\nversion: 1.2.3 # installed version\n---\n# Content",
        );
        assert_eq!(version, Some("1.2.3".to_string()));
    }
}
