//! Tests for Gitea API types deserialization.

use but_gitea::*;

mod deserialization {
    use super::*;

    #[test]
    fn test_deserialize_gitea_user() {
        let json = r#"
        {
            "id": 1,
            "login": "testuser",
            "full_name": "Test User",
            "email": "testuser@gitea.example.com",
            "avatar_url": "https://gitea.example.com/user/avatar/testuser/-1"
        }
        "#;
        let user: GiteaUser = serde_json::from_str(json).unwrap();
        assert_eq!(user.id, 1);
        assert_eq!(user.login, "testuser");
        assert_eq!(user.email, "testuser@gitea.example.com");
    }

    #[test]
    fn test_deserialize_gitea_pr() {
        let json = r#"
        {
            "id": 1,
            "url": "https://gitea.example.com/api/v1/repos/testuser/test-repo/pulls/42",
            "number": 42,
            "state": "open",
            "title": "Add new feature",
            "body": "This PR adds an exciting new feature!",
            "user": {
                "id": 1,
                "login": "testuser",
                "full_name": "Test User",
                "email": "testuser@gitea.example.com",
                "avatar_url": "https://gitea.example.com/user/avatar/testuser/-1"
            },
            "created_at": "2024-01-15T10:30:00Z",
            "updated_at": "2024-01-15T14:45:00Z",
            "head": {
                "label": "feature-branch",
                "ref": "feature-branch",
                "sha": "abc123def456789",
                "repo": null
            },
            "base": {
                "label": "main",
                "ref": "main",
                "sha": "789def456abc123",
                "repo": {
                    "id": 100,
                    "owner": {
                        "id": 1,
                        "login": "testuser",
                        "full_name": "Test User",
                        "email": "testuser@gitea.example.com",
                        "avatar_url": "https://gitea.example.com/user/avatar/testuser/-1"
                    },
                    "name": "test-repo",
                    "full_name": "testuser/test-repo",
                    "description": "A test repository",
                    "empty": false,
                    "private": false,
                    "fork": false,
                    "template": false,
                    "parent": null,
                    "mirror": false,
                    "size": 1024,
                    "html_url": "https://gitea.example.com/testuser/test-repo",
                    "ssh_url": "git@gitea.example.com:testuser/test-repo.git",
                    "clone_url": "https://gitea.example.com/testuser/test-repo.git",
                    "website": null,
                    "stars_count": 5,
                    "forks_count": 2,
                    "watchers_count": 5,
                    "open_issues_count": 3,
                    "default_branch": "main",
                    "created_at": "2023-06-01T09:00:00Z",
                    "updated_at": "2024-01-15T14:45:00Z"
                }
            },
            "html_url": "https://gitea.example.com/testuser/test-repo/pulls/42"
        }
        "#;
        let pr: but_gitea::GiteaPullRequest = serde_json::from_str(json).unwrap();
        assert_eq!(pr.number, 42);
        assert_eq!(pr.title, "Add new feature");
        assert_eq!(pr.head.r#ref, "feature-branch");
        assert_eq!(
            pr.base.repo.unwrap().clone_url,
            "https://gitea.example.com/testuser/test-repo.git"
        );
    }
}
