//! Helper utilities for opening Git repositories with proper trust error handling.

use std::path::Path;

use anyhow::{Context, Result};
use gitbutler_error::error;

/// A result type that can indicate if a repository opening failed due to trust issues
#[derive(Debug)]
pub enum RepositoryOpenResult {
    /// Repository opened successfully
    Success(gix::Repository),
    /// Repository opening failed due to trust/security issues
    TrustError {
        /// The original error that occurred
        error: gix::open::Error,
        /// The path that was being opened
        path: std::path::PathBuf,
    },
    /// Repository opening failed for other reasons
    OtherError(gix::open::Error),
}

/// Opens a Git repository with isolated options and classifies trust-related errors.
/// 
/// This function first attempts to open the repository with isolated security options.
/// If it fails with what appears to be a trust-related error, it returns a specific
/// `TrustError` variant that can be handled appropriately by the caller.
///
/// # Arguments
/// * `path` - Path to the Git repository to open
///
/// # Returns
/// * `RepositoryOpenResult` indicating success, trust error, or other error
pub fn open_repository_with_trust_check<P: AsRef<Path>>(path: P) -> RepositoryOpenResult {
    let path = path.as_ref();
    let path_buf = path.to_path_buf();
    
    match gix::open_opts(path, gix::open::Options::isolated()) {
        Ok(repo) => RepositoryOpenResult::Success(repo),
        Err(err) => {
            // Check if this is a trust-related error
            if is_trust_related_error(&err) {
                RepositoryOpenResult::TrustError {
                    error: err,
                    path: path_buf,
                }
            } else {
                RepositoryOpenResult::OtherError(err)
            }
        }
    }
}

/// Opens a repository with full trust, bypassing security checks.
/// 
/// This should only be called when the user has explicitly chosen to trust the repository
/// after being informed of the potential security implications.
///
/// # Arguments
/// * `path` - Path to the Git repository to open
///
/// # Returns
/// * `Result<gix::Repository>` - The opened repository or an error
pub fn open_repository_with_full_trust<P: AsRef<Path>>(path: P) -> Result<gix::Repository> {
    gix::open_opts(path.as_ref(), gix::open::Options::isolated().with(gix::sec::Trust::Full))
        .map_err(anyhow::Error::from)
}

/// Converts a repository opening result to an anyhow Result with appropriate error context.
/// 
/// This function converts trust errors to the GitRepositoryTrust error code for frontend handling.
#[allow(dead_code)] // This function may be used by other parts of the codebase in the future
pub fn convert_to_anyhow_result(result: RepositoryOpenResult) -> Result<gix::Repository> {
    match result {
        RepositoryOpenResult::Success(repo) => Ok(repo),
        RepositoryOpenResult::TrustError { error, path } => {
            Err(anyhow::Error::from(error))
                .context(error::Context::new_static(
                    error::Code::GitRepositoryTrust,
                    "Repository opening failed due to Git security/trust settings. The repository may be in an untrusted location.",
                ))
                .with_context(|| format!("Failed to open repository at '{}'", path.display()))
        }
        RepositoryOpenResult::OtherError(error) => {
            Err(anyhow::Error::from(error))
                .context(error::Context::new("must be a Git repository"))
        }
    }
}

/// Determines if a gix open error is related to trust/security issues.
/// 
/// This function examines the error to determine if it's caused by Git's security
/// mechanisms that prevent opening repositories in untrusted locations.
fn is_trust_related_error(error: &gix::open::Error) -> bool {
    match error {
        // Config errors often contain trust-related issues
        gix::open::Error::Config(config_err) => {
            let error_string = format!("{:?}", config_err);
            // Look for common trust-related error patterns
            error_string.contains("safe.directory") 
                || error_string.contains("trust") 
                || error_string.contains("ownership")
                || error_string.contains("dubious")
        }
        // Discovery errors can also be trust-related
        gix::open::Error::NotARepository { .. } => {
            // Some trust errors manifest as "not a repository" errors
            let error_string = format!("{:?}", error);
            error_string.contains("safe.directory") 
                || error_string.contains("trust")
                || error_string.contains("ownership")
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_open_valid_repository() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        
        // Create a valid git repository
        std::fs::create_dir_all(&repo_path).unwrap();
        gix::init(&repo_path).unwrap();
        
        let result = open_repository_with_trust_check(&repo_path);
        match result {
            RepositoryOpenResult::Success(_) => {
                // This is expected for a valid repository
            }
            other => panic!("Expected success, got: {:?}", other),
        }
    }

    #[test]
    fn test_convert_to_anyhow_result_success() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        
        // Create a valid git repository
        std::fs::create_dir_all(&repo_path).unwrap();
        let repo = gix::init(&repo_path).unwrap();
        
        let result = RepositoryOpenResult::Success(repo);
        let converted = convert_to_anyhow_result(result);
        assert!(converted.is_ok());
    }

    #[test]
    fn test_convert_to_anyhow_result_trust_error() {
        use gitbutler_error::error::AnyhowContextExt;
        
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        let fake_error = gix::open::Error::NotARepository { path: repo_path.clone() };
        
        let result = RepositoryOpenResult::TrustError {
            error: fake_error,
            path: repo_path.clone(),
        };
        
        let converted = convert_to_anyhow_result(result);
        assert!(converted.is_err());
        
        let err = converted.unwrap_err();
        let context = err.custom_context();
        if let Some(ctx) = context {
            assert_eq!(ctx.code, gitbutler_error::error::Code::GitRepositoryTrust);
        }
    }
}