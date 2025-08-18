//! Utilities for remote reference detection and tracking.
//!
//! This module consolidates GitButler's remote reference detection logic to avoid duplication
//! across different parts of the codebase while maintaining the special handling needed for
//! GitButler virtual branches that may not have proper tracking configuration.

use gix::Repository;

/// Common remote names that GitButler typically works with for fallback detection.
pub const COMMON_REMOTES: &[&str] = &["origin", "upstream"];

/// Checks if remote references exist for a branch even without tracking setup.
/// This is GitButler's fallback mechanism for virtual branches that don't have
/// proper tracking configuration.
///
/// Returns `true` if any remote reference is found for the given branch.
pub fn has_remote_refs(repo: &Repository, branch_name: &str) -> bool {
    for remote_name in COMMON_REMOTES {
        let remote_ref_name = format!("refs/remotes/{remote_name}/{branch_name}");

        match gix::refs::FullName::try_from(remote_ref_name.clone()) {
            Ok(remote_ref_name) => match repo.try_find_reference(remote_ref_name.as_ref()) {
                Ok(Some(_)) => {
                    tracing::debug!(
                        branch = branch_name,
                        remote = remote_name,
                        remote_ref = remote_ref_name.as_bstr().to_string(),
                        "GitButler fallback found remote reference"
                    );
                    return true;
                }
                Ok(None) => {
                    tracing::debug!(
                        branch = branch_name,
                        remote = remote_name,
                        remote_ref = remote_ref_name.as_bstr().to_string(),
                        "GitButler fallback: remote reference not found"
                    );
                }
                Err(err) => {
                    tracing::warn!(
                        branch = branch_name,
                        remote = remote_name,
                        remote_ref = remote_ref_name.as_bstr().to_string(),
                        error = %err,
                        "GitButler fallback: error checking remote reference"
                    );
                }
            },
            Err(err) => {
                tracing::warn!(
                    branch = branch_name,
                    remote = remote_name,
                    invalid_ref = remote_ref_name,
                    error = %err,
                    "GitButler fallback: invalid remote reference name"
                );
            }
        }
    }

    false
}

/// Attempts to find a remote reference name using GitButler's fallback pattern.
/// This function tries to locate a remote reference for the given branch using
/// common remote names when proper tracking isn't configured.
///
/// Returns the reference name if found.
pub fn find_remote_ref_name(repo: &Repository, branch_name: &str) -> Option<String> {
    for remote_name in COMMON_REMOTES {
        let remote_ref_name = format!("refs/remotes/{remote_name}/{branch_name}");

        match gix::refs::FullName::try_from(remote_ref_name.clone()) {
            Ok(remote_ref_fullname) => {
                match repo.try_find_reference(remote_ref_fullname.as_ref()) {
                    Ok(Some(_)) => {
                        tracing::debug!(
                            branch = branch_name,
                            remote = remote_name,
                            remote_ref = remote_ref_name,
                            "GitButler fallback found remote reference name"
                        );
                        return Some(remote_ref_name);
                    }
                    Ok(None) => {
                        tracing::debug!(
                            branch = branch_name,
                            remote = remote_name,
                            remote_ref = remote_ref_name,
                            "GitButler fallback: remote reference not found"
                        );
                    }
                    Err(err) => {
                        tracing::debug!(
                            branch = branch_name,
                            remote = remote_name,
                            remote_ref = remote_ref_name,
                            error = %err,
                            "GitButler fallback: error finding remote reference"
                        );
                    }
                }
            }
            Err(err) => {
                tracing::warn!(
                    branch = branch_name,
                    remote = remote_name,
                    invalid_ref = remote_ref_name,
                    error = %err,
                    "GitButler fallback: invalid remote reference name"
                );
            }
        }
    }

    None
}

/// Constructs a remote reference name for the given branch and remote.
/// This is a utility function used across GitButler's remote detection logic.
pub fn construct_remote_ref_name(remote: &str, branch: &str) -> String {
    format!("refs/remotes/{remote}/{branch}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_construct_remote_ref_name() {
        assert_eq!(
            construct_remote_ref_name("origin", "main"),
            "refs/remotes/origin/main"
        );
        assert_eq!(
            construct_remote_ref_name("upstream", "feature-branch"),
            "refs/remotes/upstream/feature-branch"
        );
    }

    #[test]
    fn test_common_remotes_are_defined() {
        assert!(COMMON_REMOTES.contains(&"origin"));
        assert!(COMMON_REMOTES.contains(&"upstream"));
    }
}
