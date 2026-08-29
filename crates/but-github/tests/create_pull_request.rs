use std::{
    io::{Read as _, Write as _},
    net::TcpListener,
    time::{Duration, Instant},
};

use but_github::{CreatePullRequestParams, GitHubClient};
use but_secret::Sensitive;
use serde_json::json;

const TOKEN: &str = "fixture-secret-token";
const PR_BODY: &str = "fixture private pull request body";

fn fixture(
    status: reqwest::StatusCode,
    body: serde_json::Value,
) -> (GitHubClient, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(2);
        let (mut stream, _) = loop {
            match listener.accept() {
                Ok(connection) => break connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "request timed out");
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("failed to accept request: {error}"),
            }
        };
        stream.set_nonblocking(false).unwrap();
        let mut request = Vec::new();
        while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            let mut buffer = [0; 1024];
            let read = stream.read(&mut buffer).unwrap();
            assert_ne!(read, 0, "request ended before its headers");
            request.extend_from_slice(&buffer[..read]);
        }
        let request = String::from_utf8(request).unwrap();
        let mut request_line = request.lines().next().unwrap().split_whitespace();
        assert_eq!(request_line.next(), Some("POST"), "create uses POST");
        assert_eq!(
            request_line.next(),
            Some("/repos/o/r/pulls"),
            "create uses the pull request endpoint"
        );

        let body = serde_json::to_string(&body).unwrap();
        let reason = status.canonical_reason().unwrap();
        write!(
            stream,
            "HTTP/1.1 {} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            status.as_u16(),
            body.len()
        )
        .unwrap();
    });
    let client = GitHubClient::new_with_host_override(
        &Sensitive(TOKEN.to_string()),
        &format!("http://{address}"),
    )
    .unwrap();
    (client, server)
}

fn params() -> CreatePullRequestParams<'static> {
    CreatePullRequestParams {
        title: "A pull request",
        body: PR_BODY,
        head: "topic",
        head_repo: None,
        base: "main",
        draft: false,
        owner: "o",
        repo: "r",
    }
}

#[tokio::test(flavor = "current_thread")]
async fn validation_failure_surfaces_githubs_safe_reason() {
    let (client, server) = fixture(
        reqwest::StatusCode::UNPROCESSABLE_ENTITY,
        json!({
            "message": "Validation Failed",
            "errors": [{
                "resource": "PullRequest",
                "code": "custom",
                "message": "No commits between main and topic"
            }]
        }),
    );

    let error = client.create_pull_request(&params()).await.unwrap_err();
    let error = format!("{error:#}");

    assert!(
        error.contains("No commits between main and topic"),
        "GitHub's actionable validation reason reaches the caller: {error}"
    );
    assert!(
        !error.contains(TOKEN) && !error.contains(PR_BODY),
        "credentials and submitted content stay out of the error: {error}"
    );
    server.join().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn successful_creation_still_returns_the_pull_request() {
    let (client, server) = fixture(
        reqwest::StatusCode::CREATED,
        json!({
            "html_url": "https://github.com/o/r/pull/7",
            "number": 7,
            "title": "A pull request",
            "body": PR_BODY,
            "user": null,
            "labels": [],
            "draft": false,
            "merge_commit_sha": null,
            "head": { "ref": "topic", "sha": "abc", "repo": null },
            "base": { "ref": "main", "sha": "def", "repo": null },
            "created_at": null,
            "updated_at": null,
            "merged_at": null,
            "closed_at": null,
            "requested_reviewers": []
        }),
    );

    let pull_request = client.create_pull_request(&params()).await.unwrap();

    assert_eq!(
        pull_request.number, 7,
        "the created pull request is decoded"
    );
    server.join().unwrap();
}
