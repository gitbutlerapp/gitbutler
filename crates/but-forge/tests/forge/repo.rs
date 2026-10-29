use but_forge::{ForgeName, RepoInfo, get_repo_info};

use crate::test_support::{accounts, assert_auth_error, repo};

#[test]
fn repo_metadata_rejects_wrong_host_accounts_before_network() {
    let (_tmp, storage, accounts) = accounts();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    for account in &accounts {
        let repo = repo(account.forge_name(), "forge.example.com");
        for preferred in std::iter::once(None).chain(accounts.iter().cloned().map(Some)) {
            let error = runtime
                .block_on(get_repo_info(&preferred, &repo, &storage))
                .unwrap_err();
            assert_auth_error(error);
        }
    }
    let file = tempfile::NamedTempFile::new().unwrap();
    let storage = but_forge_storage::Controller::from_path(file.path());
    let azure = repo(ForgeName::Azure, "dev.azure.com");
    let error = runtime
        .block_on(get_repo_info(&None, &azure, &storage))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Fetching repo info for forge Azure is not implemented yet.",
        "unsupported Azure must not touch account storage"
    );
}

fn bb_repo(permission: Option<&str>, is_fork: bool) -> but_bitbucket::BitbucketRepo {
    but_bitbucket::BitbucketRepo {
        is_fork,
        is_private: false,
        permission: permission.map(str::to_owned),
    }
}

#[test]
fn bitbucket_permission_maps_to_repo_permissions() {
    let admin = RepoInfo::from(bb_repo(Some("admin"), false)).permissions;
    assert_eq!(
        admin.map(|p| (p.admin, p.push, p.pull)),
        Some((true, true, true))
    );

    let write = RepoInfo::from(bb_repo(Some("write"), false)).permissions;
    assert_eq!(
        write.map(|p| (p.admin, p.push, p.pull)),
        Some((false, true, true))
    );

    let read = RepoInfo::from(bb_repo(Some("read"), false)).permissions;
    assert_eq!(
        read.map(|p| (p.admin, p.push, p.pull)),
        Some((false, false, true))
    );
}

#[test]
fn bitbucket_unknown_permission_defaults_to_optimistic_push() {
    let info = RepoInfo::from(bb_repo(None, false));
    let perms = info.permissions.expect("should default to Some, not None");
    assert!(perms.push, "unknown permission must default to push: true");
    assert!(perms.pull);
}

#[test]
fn bitbucket_fork_flag_is_propagated() {
    assert!(RepoInfo::from(bb_repo(Some("read"), true)).fork);
    assert!(!RepoInfo::from(bb_repo(Some("read"), false)).fork);
}

fn gl_project(visibility: Option<&str>) -> but_gitlab::GitLabProject {
    but_gitlab::GitLabProject {
        id: 1,
        path_with_namespace: "group/repo".to_owned(),
        ssh_url_to_repo: String::new(),
        http_url_to_repo: String::new(),
        default_branch: None,
        forked_from_project_id: None,
        visibility: visibility.map(str::to_owned),
        remove_source_branch_after_merge: None,
        access_level: None,
    }
}

#[test]
fn gitlab_counts_everything_but_public_as_private() {
    assert_eq!(
        RepoInfo::from(gl_project(Some("private"))).private,
        Some(true)
    );
    // Visible to everyone signed in to the instance, which is not the same
    // as visible to anyone.
    assert_eq!(
        RepoInfo::from(gl_project(Some("internal"))).private,
        Some(true)
    );
    assert_eq!(
        RepoInfo::from(gl_project(Some("public"))).private,
        Some(false)
    );
}

#[test]
fn gitlab_without_visibility_reports_unknown_rather_than_public() {
    assert_eq!(RepoInfo::from(gl_project(None)).private, None);
}
