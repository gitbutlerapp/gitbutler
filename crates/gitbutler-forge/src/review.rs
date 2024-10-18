use std::path;

use gitbutler_fs::list_files;

use crate::forge::ForgeType;

/// Get a list of available review template paths for a project
///
/// The paths are relative to the root path
pub fn available_review_templates(root_path: &path::Path, forge_type: &ForgeType) -> Vec<String> {
    let (is_review_template, get_root) = match forge_type {
        ForgeType::GitHub => (
            is_review_template_github as fn(&str) -> bool,
            get_github_directory_path as fn(&path::Path) -> path::PathBuf,
        ),
        ForgeType::GitLab => (
            is_review_template_gitlab as fn(&str) -> bool,
            get_gitlab_directory_path as fn(&path::Path) -> path::PathBuf,
        ),
        ForgeType::Bitbucket => (
            is_review_template_bitbucket as fn(&str) -> bool,
            get_bitbucket_directory_path as fn(&path::Path) -> path::PathBuf,
        ),
        ForgeType::Azure => (
            is_review_template_azure as fn(&str) -> bool,
            get_azure_directory_path as fn(&path::Path) -> path::PathBuf,
        ),
    };

    let forge_root_path = get_root(root_path);
    let forge_root_path = forge_root_path.as_path();

    let walked_paths = list_files(forge_root_path, &[forge_root_path]).unwrap_or_default();

    let mut available_paths = Vec::new();
    for entry in walked_paths {
        let path_entry = entry.as_path();
        let path_str = path_entry.to_string_lossy();

        if is_review_template(&path_str) {
            if let Ok(template_path) = forge_root_path.join(path_entry).strip_prefix(root_path) {
                available_paths.push(template_path.to_string_lossy().to_string());
            }
        }
    }

    available_paths
}

fn get_github_directory_path(root_path: &path::Path) -> path::PathBuf {
    let mut path = root_path.to_path_buf();
    path.push(".github");
    path
}

fn is_review_template_github(path_str: &str) -> bool {
    path_str == "PULL_REQUEST_TEMPLATE.md"
        || path_str == "pull_request_template.md"
        || path_str.contains("PULL_REQUEST_TEMPLATE/")
}

fn get_gitlab_directory_path(root_path: &path::Path) -> path::PathBuf {
    // TODO: implement
    root_path.to_path_buf()
}

fn is_review_template_gitlab(_path_str: &str) -> bool {
    // TODO: implement
    false
}

fn get_bitbucket_directory_path(root_path: &path::Path) -> path::PathBuf {
    // TODO: implement
    root_path.to_path_buf()
}

fn is_review_template_bitbucket(_path_str: &str) -> bool {
    // TODO: implement
    false
}

fn get_azure_directory_path(root_path: &path::Path) -> path::PathBuf {
    // TODO: implement
    root_path.to_path_buf()
}

fn is_review_template_azure(_path_str: &str) -> bool {
    // TODO: implement
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_review_template_github() {
        assert!(is_review_template_github("PULL_REQUEST_TEMPLATE.md"));
        assert!(is_review_template_github("pull_request_template.md"));
        assert!(is_review_template_github("PULL_REQUEST_TEMPLATE/"));
        assert!(!is_review_template_github("README.md"));
    }
}
