use std::path::{self, Path};

use gitbutler_fs::list_files;

use crate::forge::ForgeName;

/// Get a list of available review template paths for a project
///
/// The paths are relative to the root path
pub fn available_review_templates(root_path: &path::Path, forge_name: &ForgeName) -> Vec<String> {
    dbg!(&forge_name);
    dbg!(&root_path);
    let ReviewTemplateFunctions {
        is_review_template,
        get_root,
        ..
    } = get_review_template_functions(forge_name);

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

pub struct ReviewTemplateFunctions {
    /// Check if a file is a review template
    pub is_review_template: fn(&str) -> bool,
    /// Get the forge directory path
    pub get_root: fn(&path::Path) -> path::PathBuf,
    /// Check if a relative path is a valid review template path
    ///
    /// First argument is the relative path to the file
    /// Second argument is the root path of the project
    pub is_valid_review_template_path: fn(&path::Path, &path::Path) -> bool,
}

pub fn get_review_template_functions(forge_name: &ForgeName) -> ReviewTemplateFunctions {
    match forge_name {
        ForgeName::GitHub => ReviewTemplateFunctions {
            is_review_template: is_review_template_github,
            get_root: get_github_directory_path,
            is_valid_review_template_path: is_valid_review_template_path_github,
        },
        ForgeName::GitLab => ReviewTemplateFunctions {
            is_review_template: is_review_template_gitlab,
            get_root: get_gitlab_directory_path,
            is_valid_review_template_path: is_valid_review_template_path_gitlab,
        },
        ForgeName::Bitbucket => ReviewTemplateFunctions {
            is_review_template: is_review_template_bitbucket,
            get_root: get_bitbucket_directory_path,
            is_valid_review_template_path: is_valid_review_template_path_bitbucket,
        },
        ForgeName::Azure => ReviewTemplateFunctions {
            is_review_template: is_review_template_azure,
            get_root: get_azure_directory_path,
            is_valid_review_template_path: is_valid_review_template_path_azure,
        },
    }
}

fn get_github_directory_path(root_path: &path::Path) -> path::PathBuf {
    let mut path = root_path.to_path_buf();
    path.push(".github");
    path
}

fn is_review_template_github(path_str: &str) -> bool {
    path_str == "PULL_REQUEST_TEMPLATE.md"
        || path_str == "pull_request_template.md"
        || path_str.contains("PULL_REQUEST_TEMPLATE/") && path_str.ends_with(".md")
}

fn is_valid_review_template_path_github(path: &path::Path, root_path: &path::Path) -> bool {
    let absolute_path = Path::new(root_path).join(path);
    let forge_root_path = get_github_directory_path(root_path);

    if let Ok(template_path) = absolute_path.strip_prefix(forge_root_path) {
        is_review_template_github(&template_path.to_string_lossy())
    } else {
        false
    }
}

fn get_gitlab_directory_path(root_path: &path::Path) -> path::PathBuf {
    // TODO: implement
    root_path.to_path_buf()
}

fn is_review_template_gitlab(_path_str: &str) -> bool {
    // TODO: implement
    false
}

fn is_valid_review_template_path_gitlab(_path: &path::Path, _root_path: &path::Path) -> bool {
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

fn is_valid_review_template_path_bitbucket(_path: &path::Path, _root_path: &path::Path) -> bool {
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

fn is_valid_review_template_path_azure(_path: &path::Path, _root_path: &path::Path) -> bool {
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
        assert!(is_review_template_github("PULL_REQUEST_TEMPLATE/other.md"));
        assert!(!is_review_template_github("README.md"));
    }

    #[test]
    fn test_is_valid_review_template_path_github() {
        let root_path = Path::new("/tmp/my-project/");
        let valid_review_template_path_1 = Path::new(".github/PULL_REQUEST_TEMPLATE.md");
        let valid_review_template_path_2 = Path::new(".github/pull_request_template.md");
        let valid_review_template_path_3 = Path::new(".github/PULL_REQUEST_TEMPLATE/something.md");
        let invalid_review_template_path = Path::new("README.md");

        assert!(is_valid_review_template_path_github(
            valid_review_template_path_1,
            root_path
        ));
        assert!(is_valid_review_template_path_github(
            valid_review_template_path_2,
            root_path
        ));
        assert!(is_valid_review_template_path_github(
            valid_review_template_path_3,
            root_path
        ));
        assert!(!is_valid_review_template_path_github(
            invalid_review_template_path,
            root_path
        ));
    }
}
