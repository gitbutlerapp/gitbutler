/// Generate branch URL for GitHub remote
pub fn generate_github_branch_url(remote_url: &str, branch_name: &str) -> Option<String> {
    if let Some(repo_path) = extract_github_repo_path(remote_url) {
        Some(format!(
            "https://github.com/{}/tree/{}",
            repo_path, branch_name
        ))
    } else {
        None
    }
}

/// Generate commit URL for GitHub remote
pub fn generate_github_commit_url(remote_url: &str, commit_id: &str) -> Option<String> {
    if let Some(repo_path) = extract_github_repo_path(remote_url) {
        Some(format!(
            "https://github.com/{}/commit/{}",
            repo_path, commit_id
        ))
    } else {
        None
    }
}

/// Generate branch URL for GitLab remote
pub fn generate_gitlab_branch_url(remote_url: &str, branch_name: &str) -> Option<String> {
    if let Some((host, repo_path)) = extract_gitlab_info(remote_url) {
        Some(format!(
            "https://{}/{}/-/tree/{}",
            host, repo_path, branch_name
        ))
    } else {
        None
    }
}

/// Generate commit URL for GitLab remote
pub fn generate_gitlab_commit_url(remote_url: &str, commit_id: &str) -> Option<String> {
    if let Some((host, repo_path)) = extract_gitlab_info(remote_url) {
        Some(format!(
            "https://{}/{}/-/commit/{}",
            host, repo_path, commit_id
        ))
    } else {
        None
    }
}

/// Generate change URL for Gerrit remote
pub fn generate_gerrit_change_url(remote_url: &str) -> Option<String> {
    if let Some(base_url) = extract_gerrit_base_url(remote_url) {
        Some(format!("{}/dashboard/self", base_url))
    } else {
        None
    }
}

/// Generate appropriate URL for the remote and branch
pub fn generate_branch_url(remote_url: &str, branch_name: &str, is_gerrit: bool) -> Option<String> {
    if is_gerrit {
        generate_gerrit_change_url(remote_url)
    } else if remote_url.contains("github.com") {
        generate_github_branch_url(remote_url, branch_name)
    } else if remote_url.contains("gitlab") {
        generate_gitlab_branch_url(remote_url, branch_name)
    } else {
        None
    }
}

/// Generate appropriate URL for the remote and commit
pub fn generate_commit_url(remote_url: &str, commit_id: &str, is_gerrit: bool) -> Option<String> {
    if is_gerrit {
        generate_gerrit_change_url(remote_url)
    } else if remote_url.contains("github.com") {
        generate_github_commit_url(remote_url, commit_id)
    } else if remote_url.contains("gitlab") {
        generate_gitlab_commit_url(remote_url, commit_id)
    } else {
        None
    }
}

/// Extract GitHub repository path from various URL formats
fn extract_github_repo_path(url: &str) -> Option<String> {
    // Handle GitHub URLs: git@github.com:user/repo.git or https://github.com/user/repo.git
    if !url.contains("github.com") {
        return None;
    }

    // Find the part after github.com
    let after_github = if let Some(pos) = url.find("github.com:") {
        &url[pos + 11..] // Skip "github.com:"
    } else if let Some(pos) = url.find("github.com/") {
        &url[pos + 11..] // Skip "github.com/"
    } else {
        return None;
    };

    // Extract user/repo, removing .git suffix if present
    let repo_path = after_github.trim_end_matches(".git");
    let parts: Vec<&str> = repo_path.splitn(3, '/').collect();
    if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
        Some(format!("{}/{}", parts[0], parts[1]))
    } else {
        None
    }
}

/// Extract GitLab host and repository path from various URL formats
fn extract_gitlab_info(url: &str) -> Option<(String, String)> {
    // Handle GitLab URLs: git@gitlab.example.com:user/repo.git or https://gitlab.example.com/user/repo.git
    if !url.contains("gitlab") {
        return None;
    }

    // Extract host and path
    if let Some(colon_pos) = url.rfind(':') {
        if let Some(at_pos) = url[..colon_pos].rfind('@') {
            // SSH format: git@gitlab.example.com:user/repo.git
            let host = &url[at_pos + 1..colon_pos];
            let path_part = &url[colon_pos + 1..];
            let repo_path = path_part.trim_end_matches(".git");
            let parts: Vec<&str> = repo_path.splitn(3, '/').collect();
            if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
                return Some((host.to_string(), format!("{}/{}", parts[0], parts[1])));
            }
        }
    } else if url.starts_with("https://") {
        // HTTPS format: https://gitlab.example.com/user/repo.git
        let after_https = &url[8..]; // Skip "https://"
        if let Some(slash_pos) = after_https.find('/') {
            let host = &after_https[..slash_pos];
            let path_part = &after_https[slash_pos + 1..];
            let repo_path = path_part.trim_end_matches(".git");
            let parts: Vec<&str> = repo_path.splitn(3, '/').collect();
            if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
                return Some((host.to_string(), format!("{}/{}", parts[0], parts[1])));
            }
        }
    }
    None
}

/// Extract Gerrit base URL from remote URL
fn extract_gerrit_base_url(url: &str) -> Option<String> {
    // Handle Gerrit URLs: ssh://user@gerrit.example.com:29418/project.git or https://gerrit.example.com/a/project.git
    if url.contains("gerrit") || url.contains(":29418") {
        if url.starts_with("ssh://") {
            // SSH format: ssh://user@gerrit.example.com:29418/project.git
            let after_ssh = &url[6..]; // Skip "ssh://"
            if let Some(at_pos) = after_ssh.find('@') {
                let after_at = &after_ssh[at_pos + 1..];
                if let Some(colon_pos) = after_at.find(':') {
                    let host = &after_at[..colon_pos];
                    return Some(format!("https://{}", host));
                }
            }
        } else if url.starts_with("https://") {
            // HTTPS format: https://gerrit.example.com/a/project.git
            let after_https = &url[8..]; // Skip "https://"
            if let Some(slash_pos) = after_https.find('/') {
                let host = &after_https[..slash_pos];
                return Some(format!("https://{}", host));
            }
        }
    }
    None
}