use std::path::{self};

use gitbutler_fs::list_files;

use crate::forge::ForgeName;

/// Get a list of available review template paths for a project
///
/// The paths are relative to the root path
pub fn available_review_templates(root_path: &path::Path, forge_name: &ForgeName) -> Vec<String> {
    let ReviewTemplateFunctions {
        is_review_template,
        get_root,
        supported_template_directories,
        ..
    } = get_review_template_functions(forge_name);

    let forge_root_path = get_root(root_path);
    let forge_root_path = forge_root_path.as_path();

    // let walked_paths = list_files(forge_root_path, &[forge_root_path]).unwrap_or_default();
    let available_paths = supported_template_directories
        .iter()
        .flat_map(|dir| match dir {
            SupportedTemplateDirectory::ProjectRoot => {
                list_files(root_path, &[root_path], false, Some(root_path)).unwrap_or_default()
            }
            SupportedTemplateDirectory::ForgeRoot => {
                list_files(forge_root_path, &[root_path], true, Some(root_path)).unwrap_or_default()
            }
            SupportedTemplateDirectory::Custom(custom_dir) => {
                let custom_path = root_path.join(custom_dir);
                list_files(custom_path.as_path(), &[root_path], true, Some(root_path))
                    .unwrap_or_default()
            }
        })
        .filter_map(|entry| {
            let path_entry = entry.as_path();
            let path_str = path_entry.to_string_lossy();

            if is_review_template(&path_str) {
                return Some(path_str.to_string());
            }
            None
        })
        .collect();

    available_paths
}

pub enum SupportedTemplateDirectory {
    ProjectRoot,
    ForgeRoot,
    Custom(&'static str),
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
    pub is_valid_review_template_path: fn(&path::Path) -> bool,
    /// The supported template directories
    pub supported_template_directories: &'static [SupportedTemplateDirectory],
}

pub fn get_review_template_functions(forge_name: &ForgeName) -> ReviewTemplateFunctions {
    match forge_name {
        ForgeName::GitHub => ReviewTemplateFunctions {
            is_review_template: is_review_template_github,
            get_root: get_github_directory_path,
            is_valid_review_template_path: is_valid_review_template_path_github,
            supported_template_directories: &[
                SupportedTemplateDirectory::ForgeRoot,
                SupportedTemplateDirectory::ProjectRoot,
                SupportedTemplateDirectory::Custom("docs"),
            ],
        },
        ForgeName::GitLab => ReviewTemplateFunctions {
            is_review_template: is_review_template_gitlab,
            get_root: get_gitlab_directory_path,
            is_valid_review_template_path: is_valid_review_template_path_gitlab,
            supported_template_directories: &[SupportedTemplateDirectory::ForgeRoot],
        },
        ForgeName::Bitbucket => ReviewTemplateFunctions {
            is_review_template: is_review_template_bitbucket,
            get_root: get_bitbucket_directory_path,
            is_valid_review_template_path: is_valid_review_template_path_bitbucket,
            supported_template_directories: &[SupportedTemplateDirectory::ForgeRoot],
        },
        ForgeName::Azure => ReviewTemplateFunctions {
            is_review_template: is_review_template_azure,
            get_root: get_azure_directory_path,
            is_valid_review_template_path: is_valid_review_template_path_azure,
            supported_template_directories: &[SupportedTemplateDirectory::ForgeRoot],
        },
    }
}

fn get_github_directory_path(root_path: &path::Path) -> path::PathBuf {
    let mut path = root_path.to_path_buf();
    path.push(".github");
    path
}

fn is_review_template_github(path_str: &str) -> bool {
    let normalized_path = path_str.replace('\\', "/");
    normalized_path == "PULL_REQUEST_TEMPLATE.md"
        || normalized_path == "pull_request_template.md"
        || normalized_path.contains(".github/PULL_REQUEST_TEMPLATE")
            && normalized_path.ends_with(".md")
        || normalized_path.contains(".github/pull_request_template")
            && normalized_path.ends_with(".md")
        || normalized_path.contains("docs/PULL_REQUEST_TEMPLATE")
            && normalized_path.ends_with(".md")
        || normalized_path.contains("docs/pull_request_template")
            && normalized_path.ends_with(".md")
}

fn is_valid_review_template_path_github(path: &path::Path) -> bool {
    is_review_template_github(path.to_str().unwrap_or_default())
}

fn get_gitlab_directory_path(root_path: &path::Path) -> path::PathBuf {
    // TODO: implement
    root_path.to_path_buf()
}

fn is_review_template_gitlab(_path_str: &str) -> bool {
    // TODO: implement
    false
}

fn is_valid_review_template_path_gitlab(_path: &path::Path) -> bool {
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

fn is_valid_review_template_path_bitbucket(_path: &path::Path) -> bool {
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

fn is_valid_review_template_path_azure(_path: &path::Path) -> bool {
    // TODO: implement
    false
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn test_is_valid_review_template_path_github() {
        let valid_review_template_path_1 = Path::new(".github/PULL_REQUEST_TEMPLATE.md");
        let valid_review_template_path_2 = Path::new(".github/pull_request_template.md");
        let valid_review_template_path_3 = Path::new(".github/PULL_REQUEST_TEMPLATE/something.md");
        let valid_review_template_path_4 = Path::new(".docs/PULL_REQUEST_TEMPLATE.md");
        let valid_review_template_path_5 = Path::new("PULL_REQUEST_TEMPLATE.md");
        let invalid_review_template_path = Path::new("README.md");

        assert!(is_valid_review_template_path_github(
            valid_review_template_path_1,
        ));
        assert!(is_valid_review_template_path_github(
            valid_review_template_path_2,
        ));
        assert!(is_valid_review_template_path_github(
            valid_review_template_path_3,
        ));
        assert!(is_valid_review_template_path_github(
            valid_review_template_path_4,
        ));
        assert!(is_valid_review_template_path_github(
            valid_review_template_path_5,
        ));
        assert!(!is_valid_review_template_path_github(
            invalid_review_template_path,
        ));
    }

    #[test]
    fn test_is_valid_review_template_path_github_windows() {
        let valid_review_template_path_1 = Path::new(".github\\PULL_REQUEST_TEMPLATE.md");
        let valid_review_template_path_2 = Path::new(".github\\pull_request_template.md");
        let valid_review_template_path_3 =
            Path::new(".github\\PULL_REQUEST_TEMPLATE\\something.md");
        let valid_review_template_path_4 = Path::new(".docs\\PULL_REQUEST_TEMPLATE.md");
        let valid_review_template_path_5 = Path::new("PULL_REQUEST_TEMPLATE.md");
        let invalid_review_template_path = Path::new("README.md");

        assert!(is_valid_review_template_path_github(
            valid_review_template_path_1,
        ));
        assert!(is_valid_review_template_path_github(
            valid_review_template_path_2,
        ));
        assert!(is_valid_review_template_path_github(
            valid_review_template_path_3,
        ));
        assert!(is_valid_review_template_path_github(
            valid_review_template_path_4,
        ));
        assert!(is_valid_review_template_path_github(
            valid_review_template_path_5,
        ));
        assert!(!is_valid_review_template_path_github(
            invalid_review_template_path,
        ));
    }
}
