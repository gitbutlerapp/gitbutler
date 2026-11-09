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

/// Read one complete HTTP request (headers + body) from `stream`.
fn read_request(stream: &mut std::net::TcpStream) -> String {
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut request = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        if let Some(end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
            let length = String::from_utf8_lossy(&request[..end])
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse().unwrap())
                })
                .unwrap_or(0);
            if request.len() >= end + 4 + length {
                break;
            }
        }
        let n = stream.read(&mut buffer).unwrap();
        assert!(n > 0, "the request ended early");
        request.extend_from_slice(&buffer[..n]);
    }
    String::from_utf8_lossy(&request).to_string()
}

/// Serve a sequence of HTTP responses, asserting each request starts with the
/// given prefix. Returns captured request bodies for assertion.
fn serve(
    responses: Vec<(&'static str, reqwest::StatusCode, String)>,
) -> (GitHubClient, std::thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let mut captured = Vec::new();
        for (expected_prefix, status, body) in responses {
            let deadline = Instant::now() + Duration::from_secs(5);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "no request came for `{expected_prefix}`"
                        );
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("failed to accept request: {error}"),
                }
            };
            let request = read_request(&mut stream);
            assert!(
                request.starts_with(expected_prefix),
                "expected `{expected_prefix}`, got {request}"
            );
            let body_text = request
                .split_once("\r\n\r\n")
                .map(|(_, b)| b.to_string())
                .unwrap_or_default();
            captured.push(body_text);
            let reason = status.canonical_reason().unwrap_or("OK");
            write!(
                stream,
                "HTTP/1.1 {} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                status.as_u16(),
                body.len()
            )
            .unwrap();
        }
        captured
    });
    let client = GitHubClient::new_with_host_override(
        &Sensitive(TOKEN.to_string()),
        &format!("http://{address}"),
    )
    .unwrap();
    (client, server)
}

fn fixture(
    status: reqwest::StatusCode,
    body: serde_json::Value,
) -> (GitHubClient, std::thread::JoinHandle<Vec<String>>) {
    serve(vec![(
        "POST /repos/o/r/pulls ",
        status,
        serde_json::to_string(&body).unwrap(),
    )])
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
        labels: &[],
        assignees: &[],
    }
}

fn successful_pr_json() -> serde_json::Value {
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
    })
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

/// Empty labels/assignees make only the PR creation request (the server
/// would panic on an unexpected second connection), and the PR decodes.
#[tokio::test(flavor = "current_thread")]
async fn successful_creation_without_metadata() {
    let (client, server) = fixture(reqwest::StatusCode::CREATED, successful_pr_json());

    let pr = client.create_pull_request(&params()).await.unwrap();

    assert_eq!(pr.number, 7, "the created pull request is decoded");
    server.join().unwrap();
}

/// Labels and assignees are applied in order after creation, with the
/// correct request bodies, and the returned PR reflects the applied metadata.
#[tokio::test(flavor = "current_thread")]
async fn labels_and_assignees_are_applied_after_creation() {
    let labels = vec!["bug".to_string(), "priority".to_string()];
    let assignees = vec!["alice".to_string()];
    let (client, server) = serve(vec![
        (
            "POST /repos/o/r/pulls ",
            reqwest::StatusCode::CREATED,
            serde_json::to_string(&successful_pr_json()).unwrap(),
        ),
        (
            "POST /repos/o/r/issues/7/labels ",
            reqwest::StatusCode::OK,
            json!([
                {"id": 1, "name": "bug", "color": "d73a4a"},
                {"id": 2, "name": "priority"}
            ])
            .to_string(),
        ),
        (
            "POST /repos/o/r/issues/7/assignees ",
            reqwest::StatusCode::CREATED,
            json!({"assignees": [{"login": "alice"}]}).to_string(),
        ),
    ]);

    let mut p = params();
    p.labels = &labels;
    p.assignees = &assignees;
    let pr = client.create_pull_request(&p).await.unwrap();
    assert_eq!(pr.number, 7);
    assert_eq!(
        pr.labels
            .iter()
            .map(|l| l.name.as_str())
            .collect::<Vec<_>>(),
        vec!["bug", "priority"],
        "the returned PR carries the applied labels"
    );

    let captured = server.join().unwrap();
    let labels_body: serde_json::Value = serde_json::from_str(&captured[1]).unwrap();
    assert_eq!(
        labels_body,
        json!({"labels": ["bug", "priority"]}),
        "the labels request body carries the requested labels"
    );
    let assignees_body: serde_json::Value = serde_json::from_str(&captured[2]).unwrap();
    assert_eq!(
        assignees_body,
        json!({"assignees": ["alice"]}),
        "the assignees request body carries the requested usernames"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn label_failure_reports_pr_was_created() {
    let labels = vec!["nonexistent".to_string()];
    let (client, server) = serve(vec![
        (
            "POST /repos/o/r/pulls ",
            reqwest::StatusCode::CREATED,
            serde_json::to_string(&successful_pr_json()).unwrap(),
        ),
        (
            "POST /repos/o/r/issues/7/labels ",
            reqwest::StatusCode::NOT_FOUND,
            json!({"message": "Not Found"}).to_string(),
        ),
    ]);

    let mut p = params();
    p.labels = &labels;
    let err = client.create_pull_request(&p).await.unwrap_err();
    let msg = format!("{err:#}");
    assert!(
        msg.contains("PR #7 was created") && msg.contains("https://github.com/o/r/pull/7"),
        "the error tells the user the PR exists: {msg}"
    );
    assert!(
        msg.contains("adding labels failed"),
        "the error says which metadata step failed: {msg}"
    );
    server.join().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn assignee_failure_reports_pr_was_created() {
    let assignees = vec!["ghost".to_string()];
    let (client, server) = serve(vec![
        (
            "POST /repos/o/r/pulls ",
            reqwest::StatusCode::CREATED,
            serde_json::to_string(&successful_pr_json()).unwrap(),
        ),
        (
            "POST /repos/o/r/issues/7/assignees ",
            reqwest::StatusCode::UNPROCESSABLE_ENTITY,
            json!({"message": "Validation Failed"}).to_string(),
        ),
    ]);

    let mut p = params();
    p.assignees = &assignees;
    let err = client.create_pull_request(&p).await.unwrap_err();
    let msg = format!("{err:#}");
    assert!(
        msg.contains("PR #7 was created") && msg.contains("https://github.com/o/r/pull/7"),
        "the error tells the user the PR exists: {msg}"
    );
    assert!(
        msg.contains("adding assignees failed"),
        "the error says which metadata step failed: {msg}"
    );
    server.join().unwrap();
}

/// GitHub silently ignores non-collaborators in the assignees request;
/// the response only lists the ones that stuck.
#[tokio::test(flavor = "current_thread")]
async fn silently_dropped_assignee_is_detected() {
    let assignees = vec!["alice".to_string(), "not-a-collaborator".to_string()];
    let (client, server) = serve(vec![
        (
            "POST /repos/o/r/pulls ",
            reqwest::StatusCode::CREATED,
            serde_json::to_string(&successful_pr_json()).unwrap(),
        ),
        (
            "POST /repos/o/r/issues/7/assignees ",
            reqwest::StatusCode::CREATED,
            json!({"assignees": [{"login": "alice"}]}).to_string(),
        ),
    ]);

    let mut p = params();
    p.assignees = &assignees;
    let err = client.create_pull_request(&p).await.unwrap_err();
    let msg = format!("{err:#}");
    assert!(
        msg.contains("PR #7 was created") && msg.contains("https://github.com/o/r/pull/7"),
        "the error mentions the PR exists: {msg}"
    );
    assert!(
        msg.contains("not-a-collaborator"),
        "the error names the dropped assignee: {msg}"
    );
    server.join().unwrap();
}
