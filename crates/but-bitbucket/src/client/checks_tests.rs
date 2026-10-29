use super::*;
use std::io::{ErrorKind, Read as _, Write as _};
use std::net::TcpListener;
use std::time::{Duration, Instant};

struct MockResponse {
    path: &'static str,
    status: reqwest::StatusCode,
    body: &'static str,
}

fn mock_client(responses: Vec<MockResponse>) -> (BitbucketClient, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        for expected in responses {
            let deadline = Instant::now() + Duration::from_secs(2);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(err)
                        if err.kind() == ErrorKind::WouldBlock && Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(err) => panic!("expected request to {}: {err}", expected.path),
                }
            };
            stream.set_nonblocking(false).unwrap();

            let mut request = Vec::new();
            let mut chunk = [0; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = stream.read(&mut chunk).unwrap();
                assert_ne!(read, 0, "request should include complete HTTP headers");
                request.extend_from_slice(&chunk[..read]);
            }
            let request = String::from_utf8(request).unwrap();
            let mut request_line = request.lines().next().unwrap().split_whitespace();
            assert_eq!(
                request_line.next(),
                Some("GET"),
                "checks client uses GET requests"
            );
            assert_eq!(
                request_line.next(),
                Some(expected.path),
                "checks client requests the expected endpoint"
            );

            let reason = expected.status.canonical_reason().unwrap_or("Unknown");
            write!(
                stream,
                "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                expected.status.as_u16(),
                reason,
                expected.body.len(),
                expected.body
            )
            .unwrap();
        }
    });
    let mut client =
        BitbucketClient::new("user@example.com", &Sensitive("test-token".to_string())).unwrap();
    client.base_url = format!("http://{addr}");
    (client, server)
}

#[tokio::test(flavor = "current_thread")]
async fn lists_checks_directly_for_a_branch() {
    let (client, server) = mock_client(vec![MockResponse {
        path: "/repositories/workspace/repo/commit/feature/statuses?pagelen=100",
        status: reqwest::StatusCode::OK,
        body: r#"{"values":[{"key":"build","state":"SUCCESSFUL","commit":{"hash":"0123456789abcdef0123456789abcdef01234567"}}]}"#,
    }]);

    let checks = client
        .list_checks_for_ref("workspace", "repo", "feature")
        .await
        .expect("resolved branch should list checks")
        .expect("resolved branch should be authoritative");
    assert_eq!(checks.len(), 1, "the returned build status is preserved");
    assert_eq!(
        checks[0].commit_hash, "0123456789abcdef0123456789abcdef01234567",
        "the resolved commit hash comes from the status response"
    );
    server.join().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn slash_branch_is_encoded_for_the_statuses_endpoint() {
    let (client, server) = mock_client(vec![MockResponse {
        path: "/repositories/workspace/repo/commit/feature%2Flogin/statuses?pagelen=100",
        status: reqwest::StatusCode::OK,
        body: r#"{"values":[]}"#,
    }]);

    let checks = client
        .list_checks_for_ref("workspace", "repo", "feature/login")
        .await
        .expect("a slash branch should resolve through the statuses endpoint")
        .expect("a slash branch with no statuses is authoritative");
    assert!(
        checks.is_empty(),
        "a slash branch without statuses has no checks"
    );
    server.join().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn empty_statuses_are_authoritative() {
    let (client, server) = mock_client(vec![MockResponse {
        path: "/repositories/workspace/repo/commit/feature/statuses?pagelen=100",
        status: reqwest::StatusCode::OK,
        body: r#"{"values":[]}"#,
    }]);

    let checks = client
        .list_checks_for_ref("workspace", "repo", "feature")
        .await
        .expect("an existing ref without statuses should succeed")
        .expect("an existing ref without statuses is authoritative");
    assert!(
        checks.is_empty(),
        "an existing ref without statuses has no checks"
    );
    server.join().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn status_without_a_commit_hash_is_rejected() {
    let (client, server) = mock_client(vec![MockResponse {
        path: "/repositories/workspace/repo/commit/feature/statuses?pagelen=100",
        status: reqwest::StatusCode::OK,
        body: r#"{"values":[{"key":"build","state":"SUCCESSFUL"}]}"#,
    }]);

    let err = client
        .list_checks_for_ref("workspace", "repo", "feature")
        .await
        .expect_err("a status without its resolved commit hash is malformed");
    assert!(
        format!("{err:#}").contains("missing field `commit`"),
        "a malformed status must not use the branch name as a commit hash: {err:#}"
    );
    server.join().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn missing_ref_is_unresolved_when_the_repository_is_accessible() {
    let (client, server) = mock_client(vec![
        MockResponse {
            path: "/repositories/workspace/repo/commit/deleted-branch/statuses?pagelen=100",
            status: reqwest::StatusCode::NOT_FOUND,
            body: "{}",
        },
        MockResponse {
            path: "/repositories/workspace/repo",
            status: reqwest::StatusCode::OK,
            body: "{}",
        },
    ]);

    let checks = client
        .list_checks_for_ref("workspace", "repo", "deleted-branch")
        .await
        .expect("an accessible repository with a missing ref is not an API failure");
    assert!(
        checks.is_none(),
        "a missing ref must not replace authoritative cached checks"
    );
    server.join().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn inaccessible_repository_is_not_misclassified_as_a_missing_ref() {
    let (client, server) = mock_client(vec![
        MockResponse {
            path: "/repositories/workspace/repo/commit/feature/statuses?pagelen=100",
            status: reqwest::StatusCode::NOT_FOUND,
            body: "{}",
        },
        MockResponse {
            path: "/repositories/workspace/repo",
            status: reqwest::StatusCode::NOT_FOUND,
            body: "{}",
        },
        MockResponse {
            path: "/user",
            status: reqwest::StatusCode::OK,
            body: "{}",
        },
    ]);

    let err = client
        .list_checks_for_ref("workspace", "repo", "feature")
        .await
        .expect_err("an inaccessible repository should remain an error");
    assert!(
        err.to_string().contains("inaccessible to this token"),
        "repository access errors should be actionable: {err:#}"
    );
    server.join().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn repository_probe_server_error_is_not_misclassified_as_an_access_failure() {
    let (client, server) = mock_client(vec![
        MockResponse {
            path: "/repositories/workspace/repo/commit/feature/statuses?pagelen=100",
            status: reqwest::StatusCode::NOT_FOUND,
            body: "{}",
        },
        MockResponse {
            path: "/repositories/workspace/repo",
            status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            body: "{}",
        },
    ]);

    let err = client
        .list_checks_for_ref("workspace", "repo", "feature")
        .await
        .expect_err("a repository probe server failure should remain an error");
    assert!(
        format!("{err:#}").contains("HTTP 500 Internal Server Error"),
        "the repository probe preserves its server status: {err:#}"
    );
    assert!(
        !err.to_string().contains("missing required scope"),
        "a server failure must not be presented as a scope problem: {err:#}"
    );
    server.join().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn repository_scope_failure_is_distinct_from_a_missing_ref() {
    let (client, server) = mock_client(vec![
        MockResponse {
            path: "/repositories/workspace/repo/commit/feature/statuses?pagelen=100",
            status: reqwest::StatusCode::FORBIDDEN,
            body: "{}",
        },
        MockResponse {
            path: "/user",
            status: reqwest::StatusCode::OK,
            body: "{}",
        },
    ]);

    let err = client
        .list_checks_for_ref("workspace", "repo", "feature")
        .await
        .expect_err("repository scope failure should remain an error");
    assert!(
        err.to_string().contains("read:repository:bitbucket"),
        "scope errors should name the required scope: {err:#}"
    );
    server.join().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn invalid_credentials_are_distinct_from_repository_access() {
    let (client, server) = mock_client(vec![
        MockResponse {
            path: "/repositories/workspace/repo/commit/feature/statuses?pagelen=100",
            status: reqwest::StatusCode::UNAUTHORIZED,
            body: "{}",
        },
        MockResponse {
            path: "/user",
            status: reqwest::StatusCode::UNAUTHORIZED,
            body: "{}",
        },
    ]);

    let err = client
        .list_checks_for_ref("workspace", "repo", "feature")
        .await
        .expect_err("invalid credentials should remain an error");
    assert!(
        err.to_string().contains("invalid or expired"),
        "credential errors should not be presented as access failures: {err:#}"
    );
    server.join().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn user_scope_failure_is_distinct_from_invalid_credentials() {
    let (client, server) = mock_client(vec![
        MockResponse {
            path: "/repositories/workspace/repo/commit/feature/statuses?pagelen=100",
            status: reqwest::StatusCode::FORBIDDEN,
            body: "{}",
        },
        MockResponse {
            path: "/user",
            status: reqwest::StatusCode::FORBIDDEN,
            body: "{}",
        },
    ]);

    let err = client
        .list_checks_for_ref("workspace", "repo", "feature")
        .await
        .expect_err("missing user scope should remain an error");
    assert!(
        err.to_string().contains("read:user:bitbucket"),
        "user scope errors should name the required scope: {err:#}"
    );
    server.join().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn unrelated_status_errors_are_preserved() {
    let (client, server) = mock_client(vec![MockResponse {
        path: "/repositories/workspace/repo/commit/feature/statuses?pagelen=100",
        status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
        body: "{}",
    }]);

    let err = client
        .list_checks_for_ref("workspace", "repo", "feature")
        .await
        .expect_err("server failures should remain errors");
    assert_eq!(
        err.to_string(),
        "Bitbucket request failed: 500 Internal Server Error",
        "unrelated status failures keep their original error"
    );
    server.join().unwrap();
}
