use super::*;

#[test]
fn test_deserialize_gitea_user() {
    let json = r#"
    {
        "id": 1,
        "login": "octocat",
        "full_name": "Monalisa Octocat",
        "email": "octocat@github.com",
        "avatar_url": "https://github.com/images/error/octocat_happy.gif"
    }
    "#;
    let user: GiteaUser = serde_json::from_str(json).unwrap();
    assert_eq!(user.id, 1);
    assert_eq!(user.login, "octocat");
    assert_eq!(user.email, "octocat@github.com");
}

#[test]
fn test_deserialize_gitea_pr() {
    let json = r#"
    {
        "id": 1,
        "url": "https://gitea.com/api/v1/repos/octocat/Hello-World/pulls/1347",
        "number": 1347,
        "state": "open",
        "title": "Amazing new feature",
        "body": "Please pull these awesome changes in!",
        "user": {
            "id": 1,
            "login": "octocat",
            "full_name": "Monalisa Octocat",
            "email": "octocat@github.com",
            "avatar_url": "https://github.com/images/error/octocat_happy.gif"
        },
        "created_at": "2011-01-26T19:01:12Z",
        "updated_at": "2011-01-26T19:01:12Z",
        "head": {
            "label": "new-topic",
            "ref": "new-topic",
            "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e",
            "repo": null
        },
        "base": {
            "label": "master",
            "ref": "master",
            "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e",
            "repo": {
                "id": 1,
                "owner": {
                    "id": 1,
                    "login": "octocat",
                    "full_name": "Monalisa Octocat",
                    "email": "octocat@github.com",
                    "avatar_url": "https://github.com/images/error/octocat_happy.gif"
                },
                "name": "Hello-World",
                "full_name": "octocat/Hello-World",
                "description": "This your first repo!",
                "empty": false,
                "private": false,
                "fork": false,
                "template": false,
                "parent": null,
                "mirror": false,
                "size": 0,
                "html_url": "https://github.com/octocat/Hello-World",
                "ssh_url": "git@github.com:octocat/Hello-World.git",
                "clone_url": "https://github.com/octocat/Hello-World.git",
                "website": "https://github.com/blog",
                "stars_count": 80,
                "forks_count": 9,
                "watchers_count": 80,
                "open_issues_count": 0,
                "default_branch": "master",
                "created_at": "2011-01-26T19:01:12Z",
                "updated_at": "2011-01-26T19:01:12Z"
            }
        },
        "html_url": "https://github.com/octocat/Hello-World/pull/1347"
    }
    "#;
    let pr: GiteaPullRequest = serde_json::from_str(json).unwrap();
    assert_eq!(pr.number, 1347);
    assert_eq!(pr.title, "Amazing new feature");
    assert_eq!(pr.head.r#ref, "new-topic");
    assert_eq!(
        pr.base.repo.unwrap().clone_url,
        "https://github.com/octocat/Hello-World.git"
    );
}
