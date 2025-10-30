use std::fmt::Display;

use anyhow::Result;

#[derive(Clone, Debug, PartialEq)]
pub struct PushOutput {
    pub success: bool,
    pub warnings: Vec<String>,
    pub changes: Vec<ChangeInfo>,
    pub processing_info: Option<ProcessingInfo>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChangeInfo {
    pub url: String,
    pub commit_title: String,
    pub is_new: bool,
    pub is_wip: bool,
    pub is_private: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProcessingInfo {
    pub refs_count: u32,
    pub updated_count: Option<u32>,
    pub new_count: Option<u32>,
}

impl Display for PushOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PushOutput {{ success: {}, warnings: {}, changes: {} }}",
            self.success,
            self.warnings.len(),
            self.changes.len()
        )
    }
}

pub fn push_output(output: &str) -> Result<PushOutput> {
    let mut success = false;
    let mut warnings = Vec::new();
    let mut changes = Vec::new();
    let mut processing_info = None;
    let mut current_section = ChangeSection::None;

    for line in output.lines() {
        let line = line.trim_start_matches("remote: ");

        // Check for SUCCESS
        if line.trim() == "SUCCESS" {
            success = true;
        }

        // Check for warnings
        if line.contains("warning:") {
            warnings.push(line.to_string());
        }

        // Check for section headers
        if line.trim() == "New Changes:" {
            current_section = ChangeSection::New;
        } else if line.trim() == "Updated Changes:" {
            current_section = ChangeSection::Updated;
        }

        // Check for change URL (http links) and parse change info
        if line.trim_start().starts_with("http")
            && let Some(mut change_info) = parse_change_info(line.trim())
        {
            // Override is_new based on current section if we have section info
            match current_section {
                ChangeSection::New => change_info.is_new = true,
                ChangeSection::Updated => change_info.is_new = false,
                ChangeSection::None => {
                    // Keep the existing logic for backward compatibility
                    // (relies on [NEW] tag detection)
                }
            }
            changes.push(change_info);
        }

        // Parse processing info
        if line.contains("Processing changes:")
            && line.contains("refs:")
            && let Some(info) = parse_processing_info(line)
        {
            processing_info = Some(info);
        }
    }

    Ok(PushOutput {
        success,
        warnings,
        changes,
        processing_info,
    })
}

#[derive(Clone, Debug, PartialEq)]
enum ChangeSection {
    None,
    New,
    Updated,
}

fn parse_change_info(line: &str) -> Option<ChangeInfo> {
    // Parse lines like:
    // "http://15a45d4cba1a/c/gerrit-test/+/42 aaaaaaa [NEW]"
    // "http://15a45d4cba1a/c/gerrit-test/+/47 hello [WIP] [NEW]"
    // "https://codereview.qt-project.org/c/qt/qtbase/+/687666 Test manual push [PRIVATE] [NEW]"
    // or "http://15a45d4cba1a/c/gerrit-test/+/41 sup5"

    let parts: Vec<&str> = line.splitn(2, ' ').collect();
    if parts.len() < 2 {
        // Just URL with no title
        return Some(ChangeInfo {
            url: line.to_string(),
            commit_title: String::new(),
            is_new: false,
            is_wip: false,
            is_private: false,
        });
    }

    let url = parts[0].to_string();
    let rest = parts[1];

    // Check for [WIP], [NEW], and [PRIVATE] tags
    let is_wip = rest.contains("[WIP]");
    let is_new = rest.contains("[NEW]");
    let is_private = rest.contains("[PRIVATE]");

    // Remove tags to get clean commit title
    let mut commit_title = rest.to_string();
    if is_wip {
        commit_title = commit_title.replace(" [WIP]", "");
    }
    if is_new {
        commit_title = commit_title.replace(" [NEW]", "");
    }
    if is_private {
        commit_title = commit_title.replace(" [PRIVATE]", "");
    }
    let commit_title = commit_title.trim().to_string();

    Some(ChangeInfo {
        url,
        commit_title,
        is_new,
        is_wip,
        is_private,
    })
}

fn parse_processing_info(line: &str) -> Option<ProcessingInfo> {
    // Parse "Processing changes: refs: 1, updated: 1, done" or
    // "Processing changes: refs: 1, new: 3, done"
    let refs_count = extract_number_after(line, "refs:")?;
    let updated_count = extract_number_after(line, "updated:");
    let new_count = extract_number_after(line, "new:");

    Some(ProcessingInfo {
        refs_count,
        updated_count,
        new_count,
    })
}

fn extract_number_after(text: &str, pattern: &str) -> Option<u32> {
    let start_pos = text.find(pattern)?;
    let start = start_pos.checked_add(pattern.len())?;

    // Safe bounds check before slicing
    if start >= text.len() {
        return None;
    }

    let rest = &text[start..];
    let number_str: String = rest
        .chars()
        .skip_while(|c| c.is_whitespace())
        .take_while(|c| c.is_ascii_digit())
        .collect();

    // Return None for empty strings to avoid parse errors
    if number_str.is_empty() {
        return None;
    }

    number_str.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gerrit_push_output_single_change() {
        let output = r#"remote:
remote: Processing changes: refs: 1, updated: 1
remote: Processing changes: refs: 1, updated: 1
remote: Processing changes: refs: 1, updated: 1
remote: Processing changes: refs: 1, updated: 1, done
remote: warning: b643793: no files changed, message updated
remote:
remote: SUCCESS
remote:
remote:   http://15a45d4cba1a/c/gerrit-test/+/41 sup5
remote:"#;

        let result = push_output(output).unwrap();

        assert!(result.success);
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("no files changed, message updated"));
        assert_eq!(result.changes.len(), 1);

        let change = &result.changes[0];
        assert_eq!(change.url, "http://15a45d4cba1a/c/gerrit-test/+/41");
        assert_eq!(change.commit_title, "sup5");
        assert!(!change.is_new);

        assert!(result.processing_info.is_some());
        let processing = result.processing_info.unwrap();
        assert_eq!(processing.refs_count, 1);
        assert_eq!(processing.updated_count, Some(1));
        assert_eq!(processing.new_count, None);
    }

    #[test]
    fn test_parse_multiple_new_commits() {
        let output = r#"remote: 
remote: Processing changes: refs: 1, new: 3        
remote: Processing changes: refs: 1, new: 3        
remote: Processing changes: refs: 1, new: 3, done            
remote: 
remote: SUCCESS        
remote: 
remote:   http://15a45d4cba1a/c/gerrit-test/+/42 aaaaaaa [NEW]        
remote:   http://15a45d4cba1a/c/gerrit-test/+/43 bbbbbb [NEW]        
remote:   http://15a45d4cba1a/c/gerrit-test/+/44 cccccc [NEW]        
remote:"#;

        let result = push_output(output).unwrap();

        assert!(result.success);
        assert_eq!(result.warnings.len(), 0);
        assert_eq!(result.changes.len(), 3);

        // Check first change
        let change1 = &result.changes[0];
        assert_eq!(change1.url, "http://15a45d4cba1a/c/gerrit-test/+/42");
        assert_eq!(change1.commit_title, "aaaaaaa");
        assert!(change1.is_new);

        // Check second change
        let change2 = &result.changes[1];
        assert_eq!(change2.url, "http://15a45d4cba1a/c/gerrit-test/+/43");
        assert_eq!(change2.commit_title, "bbbbbb");
        assert!(change2.is_new);

        // Check third change
        let change3 = &result.changes[2];
        assert_eq!(change3.url, "http://15a45d4cba1a/c/gerrit-test/+/44");
        assert_eq!(change3.commit_title, "cccccc");
        assert!(change3.is_new);

        assert!(result.processing_info.is_some());
        let processing = result.processing_info.unwrap();
        assert_eq!(processing.refs_count, 1);
        assert_eq!(processing.updated_count, None);
        assert_eq!(processing.new_count, Some(3));
    }

    #[test]
    fn test_parse_failed_push() {
        let output = r#"remote:
remote: ERROR: some error occurred
remote:"#;

        let result = push_output(output).unwrap();

        assert!(!result.success);
        assert_eq!(result.warnings.len(), 0);
        assert_eq!(result.changes.len(), 0);
        assert_eq!(result.processing_info, None);
    }

    #[test]
    fn test_extract_number_after() {
        assert_eq!(
            extract_number_after("refs: 5, updated: 3", "refs:"),
            Some(5)
        );
        assert_eq!(
            extract_number_after("refs: 5, updated: 3", "updated:"),
            Some(3)
        );
        assert_eq!(extract_number_after("no number here", "refs:"), None);
    }

    #[test]
    fn test_edge_cases_no_panic() {
        // Test empty string
        let result = push_output("");
        assert!(result.is_ok());

        // Test malformed input that could cause panics
        let result = push_output("refs:");
        assert!(result.is_ok());

        // Test with just the pattern at the end
        let result = push_output("remote: refs:");
        assert!(result.is_ok());

        // Test with unicode characters
        let result = push_output("remote: 🦀 Processing changes: refs: 1, updated: 1");
        assert!(result.is_ok());

        // Test very long string
        let long_output = "remote: ".repeat(10000) + "SUCCESS";
        let result = push_output(&long_output);
        assert!(result.is_ok());
        assert!(result.unwrap().success);
    }

    #[test]
    fn test_extract_number_edge_cases() {
        // Pattern at end of string
        assert_eq!(extract_number_after("refs:", "refs:"), None);

        // Pattern with no number following
        assert_eq!(extract_number_after("refs: abc", "refs:"), None);

        // Pattern with only whitespace following
        assert_eq!(extract_number_after("refs:   ", "refs:"), None);

        // Pattern not found
        assert_eq!(extract_number_after("no refs here", "refs:"), None);

        // Empty text
        assert_eq!(extract_number_after("", "refs:"), None);

        // Number overflow - should return None gracefully
        let big_number = format!("refs: {}", u64::MAX);
        assert_eq!(extract_number_after(&big_number, "refs:"), None);
    }

    #[test]
    fn test_parse_change_info() {
        // URL with commit title (not new)
        let result = parse_change_info("http://example.com/change/123 my commit message");
        let expected = ChangeInfo {
            url: "http://example.com/change/123".to_string(),
            commit_title: "my commit message".to_string(),
            is_new: false,
            is_wip: false,
            is_private: false,
        };
        assert_eq!(result, Some(expected));

        // URL with NEW tag
        let result = parse_change_info("http://gerrit.local/c/project/+/41 fix bug [NEW]");
        let expected = ChangeInfo {
            url: "http://gerrit.local/c/project/+/41".to_string(),
            commit_title: "fix bug".to_string(),
            is_new: true,
            is_wip: false,
            is_private: false,
        };
        assert_eq!(result, Some(expected));

        // URL without title
        let result = parse_change_info("http://example.com/change/123");
        let expected = ChangeInfo {
            url: "http://example.com/change/123".to_string(),
            commit_title: String::new(),
            is_new: false,
            is_wip: false,
            is_private: false,
        };
        assert_eq!(result, Some(expected));

        // URL with multi-word title and NEW tag
        let result =
            parse_change_info("http://gerrit.local/c/project/+/41 fix: update dependencies [NEW]");
        let expected = ChangeInfo {
            url: "http://gerrit.local/c/project/+/41".to_string(),
            commit_title: "fix: update dependencies".to_string(),
            is_new: true,
            is_wip: false,
            is_private: false,
        };
        assert_eq!(result, Some(expected));
    }

    #[test]
    fn test_parse_wip_changes() {
        // Test WIP tag only
        let result = parse_change_info("http://gerrit.local/c/project/+/47 hello [WIP]");
        let expected = ChangeInfo {
            url: "http://gerrit.local/c/project/+/47".to_string(),
            commit_title: "hello".to_string(),
            is_new: false,
            is_wip: true,
            is_private: false,
        };
        assert_eq!(result, Some(expected));

        // Test both WIP and NEW tags
        let result = parse_change_info("http://15a45d4cba1a/c/gerrit-test/+/47 hello [WIP] [NEW]");
        let expected = ChangeInfo {
            url: "http://15a45d4cba1a/c/gerrit-test/+/47".to_string(),
            commit_title: "hello".to_string(),
            is_new: true,
            is_wip: true,
            is_private: false,
        };
        assert_eq!(result, Some(expected));

        // Test NEW and WIP in different order
        let result =
            parse_change_info("http://gerrit.local/c/project/+/48 multi word title [NEW] [WIP]");
        let expected = ChangeInfo {
            url: "http://gerrit.local/c/project/+/48".to_string(),
            commit_title: "multi word title".to_string(),
            is_new: true,
            is_wip: true,
            is_private: false,
        };
        assert_eq!(result, Some(expected));
    }

    #[test]
    fn test_parse_wip_push_output() {
        let output = r#"remote: 
remote: Processing changes: refs: 1, new: 1        
remote: Processing changes: refs: 1, new: 1        
remote: Processing changes: refs: 1, new: 1        
remote: Processing changes: refs: 1, new: 1, done            
remote: 
remote: SUCCESS        
remote: 
remote:   http://15a45d4cba1a/c/gerrit-test/+/47 hello [WIP] [NEW]        
remote:"#;

        let result = push_output(output).unwrap();

        assert!(result.success);
        assert_eq!(result.warnings.len(), 0);
        assert_eq!(result.changes.len(), 1);

        let change = &result.changes[0];
        assert_eq!(change.url, "http://15a45d4cba1a/c/gerrit-test/+/47");
        assert_eq!(change.commit_title, "hello");
        assert!(change.is_new);
        assert!(change.is_wip);

        assert!(result.processing_info.is_some());
        let processing = result.processing_info.unwrap();
        assert_eq!(processing.refs_count, 1);
        assert_eq!(processing.updated_count, None);
        assert_eq!(processing.new_count, Some(1));
    }

    #[test]
    fn test_parse_new_changes_section() {
        let output = r#"remote: Processing changes: refs: 1, new: 1, done
remote:
remote: New Changes:
remote:   http://gerrithost/#/c/RecipeBook/+/702 Change to a proper, yeast based pizza dough.
remote:"#;

        let result = push_output(output).unwrap();

        assert!(!result.success); // No explicit SUCCESS statement
        assert_eq!(result.warnings.len(), 0);
        assert_eq!(result.changes.len(), 1);

        let change = &result.changes[0];
        assert_eq!(change.url, "http://gerrithost/#/c/RecipeBook/+/702");
        assert_eq!(
            change.commit_title,
            "Change to a proper, yeast based pizza dough."
        );
        assert!(change.is_new);

        assert!(result.processing_info.is_some());
        let processing = result.processing_info.unwrap();
        assert_eq!(processing.refs_count, 1);
        assert_eq!(processing.updated_count, None);
        assert_eq!(processing.new_count, Some(1));
    }

    #[test]
    fn test_parse_updated_changes_section() {
        let output = r#"remote: Processing changes: refs: 1, updated: 1, done
remote:
remote: Updated Changes:
remote:   http://gerrithost/#/c/RecipeBook/+/702 Change to a proper, yeast based pizza dough.
remote:"#;

        let result = push_output(output).unwrap();

        assert!(!result.success); // No explicit SUCCESS statement
        assert_eq!(result.warnings.len(), 0);
        assert_eq!(result.changes.len(), 1);

        let change = &result.changes[0];
        assert_eq!(change.url, "http://gerrithost/#/c/RecipeBook/+/702");
        assert_eq!(
            change.commit_title,
            "Change to a proper, yeast based pizza dough."
        );
        assert!(!change.is_new); // Should be false for updated changes

        assert!(result.processing_info.is_some());
        let processing = result.processing_info.unwrap();
        assert_eq!(processing.refs_count, 1);
        assert_eq!(processing.updated_count, Some(1));
        assert_eq!(processing.new_count, None);
    }

    #[test]
    fn test_mixed_new_and_updated_sections() {
        let output = r#"remote: Processing changes: refs: 3, new: 2, updated: 1, done
remote:
remote: New Changes:
remote:   http://gerrithost/#/c/RecipeBook/+/703 Add new recipe
remote:   http://gerrithost/#/c/RecipeBook/+/704 Another new recipe
remote:
remote: Updated Changes:
remote:   http://gerrithost/#/c/RecipeBook/+/702 Updated existing recipe
remote:"#;

        let result = push_output(output).unwrap();

        assert_eq!(result.changes.len(), 3);

        // Check new changes
        let new_change1 = &result.changes[0];
        assert_eq!(new_change1.url, "http://gerrithost/#/c/RecipeBook/+/703");
        assert_eq!(new_change1.commit_title, "Add new recipe");
        assert!(new_change1.is_new);

        let new_change2 = &result.changes[1];
        assert_eq!(new_change2.url, "http://gerrithost/#/c/RecipeBook/+/704");
        assert_eq!(new_change2.commit_title, "Another new recipe");
        assert!(new_change2.is_new);

        // Check updated change
        let updated_change = &result.changes[2];
        assert_eq!(updated_change.url, "http://gerrithost/#/c/RecipeBook/+/702");
        assert_eq!(updated_change.commit_title, "Updated existing recipe");
        assert!(!updated_change.is_new);

        let processing = result.processing_info.unwrap();
        assert_eq!(processing.refs_count, 3);
        assert_eq!(processing.new_count, Some(2));
        assert_eq!(processing.updated_count, Some(1));
    }

    #[test]
    fn test_backward_compatibility_with_new_tag() {
        // Test that the old [NEW] tag format still works when no section headers are present
        let output = r#"remote: Processing changes: refs: 1, new: 1, done
remote: SUCCESS
remote:   http://gerrit.local/c/project/+/41 fix bug [NEW]
remote:"#;

        let result = push_output(output).unwrap();

        assert_eq!(result.changes.len(), 1);
        let change = &result.changes[0];
        assert_eq!(change.url, "http://gerrit.local/c/project/+/41");
        assert_eq!(change.commit_title, "fix bug");
        assert!(change.is_new); // Should be true from [NEW] tag
    }

    #[test]
    fn test_parse_private_push_output() {
        let output = r#"remote: Resolving deltas: 100% (2/2)
remote: Processing changes: refs: 1, new: 1, done
remote:
remote: SUCCESS
remote:
remote:   https://codereview.qt-project.org/c/qt/qtbase/+/687666 Test manual push [PRIVATE] [NEW]
remote:"#;

        let result = push_output(output).unwrap();

        assert!(result.success);
        assert_eq!(result.warnings.len(), 0);
        assert_eq!(result.changes.len(), 1);

        let change = &result.changes[0];
        assert_eq!(
            change.url,
            "https://codereview.qt-project.org/c/qt/qtbase/+/687666"
        );
        assert_eq!(change.commit_title, "Test manual push");
        assert!(change.is_new);
        assert!(change.is_private);
        assert!(!change.is_wip);

        assert!(result.processing_info.is_some());
        let processing = result.processing_info.unwrap();
        assert_eq!(processing.refs_count, 1);
        assert_eq!(processing.updated_count, None);
        assert_eq!(processing.new_count, Some(1));
    }

    #[test]
    fn test_parse_private_tag_only() {
        // Test PRIVATE tag without NEW
        let result =
            parse_change_info("http://gerrit.local/c/project/+/50 private change [PRIVATE]");
        let expected = ChangeInfo {
            url: "http://gerrit.local/c/project/+/50".to_string(),
            commit_title: "private change".to_string(),
            is_new: false,
            is_wip: false,
            is_private: true,
        };
        assert_eq!(result, Some(expected));
    }

    #[test]
    fn test_parse_all_tags_combined() {
        // Test all three tags together
        let result =
            parse_change_info("http://gerrit.local/c/project/+/51 all tags [WIP] [PRIVATE] [NEW]");
        let expected = ChangeInfo {
            url: "http://gerrit.local/c/project/+/51".to_string(),
            commit_title: "all tags".to_string(),
            is_new: true,
            is_wip: true,
            is_private: true,
        };
        assert_eq!(result, Some(expected));
    }
}
