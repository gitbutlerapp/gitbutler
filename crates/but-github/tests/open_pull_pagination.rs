use std::{
    io::{Read as _, Write as _},
    net::TcpListener,
    time::{Duration, Instant},
};

use but_github::GitHubClient;
use but_secret::Sensitive;
use serde_json::json;

struct MockResponse {
    page: usize,
    status: reqwest::StatusCode,
    body: serde_json::Value,
}

impl MockResponse {
    fn ok(page: usize, body: Vec<serde_json::Value>) -> Self {
        Self {
            page,
            status: reqwest::StatusCode::OK,
            body: body.into(),
        }
    }
}

fn pull(number: i64) -> serde_json::Value {
    json!({
        "html_url": format!("https://github.com/o/r/pull/{number}"),
        "number": number,
        "title": format!("PR {number}"),
        "body": null,
        "user": null,
        "labels": [],
        "draft": false,
        "merge_commit_sha": null,
        "head": { "ref": format!("branch-{number}"), "sha": format!("sha-{number}"), "repo": null },
        "base": { "ref": "main", "sha": "base", "repo": null },
        "created_at": null,
        "updated_at": null,
        "merged_at": null,
        "closed_at": null,
        "requested_reviewers": []
    })
}

fn fixture(responses: Vec<MockResponse>) -> (GitHubClient, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        for (index, response) in responses.into_iter().enumerate() {
            let deadline = Instant::now() + Duration::from_secs(2);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "request {} timed out", index + 1);
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("failed to accept request: {error}"),
                }
            };
            // Accepted streams inherit non-blocking from the listener on some
            // platforms; reads below expect to block until data arrives.
            stream.set_nonblocking(false).unwrap();
            let mut request = Vec::new();
            while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                let mut buffer = [0; 1024];
                let read = stream.read(&mut buffer).unwrap();
                assert_ne!(read, 0, "request {} ended before its headers", index + 1);
                request.extend_from_slice(&buffer[..read]);
            }
            let request = String::from_utf8(request).unwrap();
            let path = request.lines().next().unwrap().split_whitespace().nth(1);
            let url = reqwest::Url::parse(&format!("http://localhost{}", path.unwrap())).unwrap();
            let mut query = url.query_pairs().into_owned().collect::<Vec<_>>();
            query.sort();
            assert_eq!(
                url.path(),
                "/repos/o/r/pulls",
                "every request uses the pull request endpoint"
            );
            assert_eq!(
                query,
                vec![
                    ("direction".to_string(), "asc".to_string()),
                    ("page".to_string(), response.page.to_string()),
                    ("per_page".to_string(), "100".to_string()),
                    ("sort".to_string(), "created".to_string()),
                    ("state".to_string(), "open".to_string()),
                ],
                "every page keeps the open pull request query parameters"
            );
            let body = serde_json::to_string(&response.body).unwrap();
            let reason = response.status.canonical_reason().unwrap();
            write!(
                stream,
                "HTTP/1.1 {} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                response.status.as_u16(),
                body.len()
            )
            .unwrap();
        }
    });
    let client = GitHubClient::new_with_host_override(
        &Sensitive("token".to_string()),
        &format!("http://{address}"),
    )
    .unwrap();
    (client, server)
}

#[tokio::test(flavor = "current_thread")]
async fn list_open_pulls_fetches_more_than_one_page() {
    let (client, server) = fixture(vec![
        MockResponse::ok(1, (1..=100).map(pull).collect()),
        MockResponse::ok(2, vec![pull(101)]),
    ]);

    let pulls = client.list_open_pulls("o", "r").await.unwrap();

    assert_eq!(
        pulls.len(),
        101,
        "every open pull request page must be returned"
    );
    server.join().unwrap();
}

fn full_pages(count: usize) -> Vec<MockResponse> {
    (0..count)
        .map(|page| {
            MockResponse::ok(
                page + 1,
                (page * 100 + 1..=(page + 1) * 100)
                    .map(|number| pull(number as i64))
                    .collect(),
            )
        })
        .collect()
}

#[tokio::test(flavor = "current_thread")]
async fn list_open_pulls_stops_after_an_empty_second_page() {
    let (client, server) = fixture(vec![
        MockResponse::ok(1, (1..=100).map(pull).collect()),
        MockResponse::ok(2, Vec::new()),
    ]);

    let pulls = client.list_open_pulls("o", "r").await.unwrap();

    assert_eq!(
        pulls.len(),
        100,
        "an exact page has no missing or extra pulls"
    );
    server.join().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn list_open_pulls_deduplicates_a_page_boundary_shift() {
    let mut repeated = pull(100);
    repeated["title"] = "PR 100 refetched".into();
    let (client, server) = fixture(vec![
        MockResponse::ok(1, (1..=100).map(pull).collect()),
        MockResponse::ok(2, vec![repeated, pull(102)]),
    ]);

    let pulls = client.list_open_pulls("o", "r").await.unwrap();
    let numbers: Vec<_> = pulls.iter().map(|pull| pull.number).collect();

    assert_eq!(
        numbers,
        (1..=100).chain([102]).collect::<Vec<_>>(),
        "a pull repeated by a mid-scan page shift appears once"
    );
    let boundary = pulls.iter().find(|pull| pull.number == 100).unwrap();
    assert_eq!(
        boundary.title, "PR 100 refetched",
        "the later fetch of a duplicated pull is fresher and wins"
    );
    server.join().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn list_open_pulls_tolerates_a_skipped_pull_after_a_mid_scan_close() {
    let (client, server) = fixture(vec![
        MockResponse::ok(1, (1..=100).map(pull).collect()),
        MockResponse::ok(2, vec![pull(102), pull(103)]),
    ]);

    let pulls = client.list_open_pulls("o", "r").await.unwrap();
    let numbers: Vec<_> = pulls.iter().map(|pull| pull.number).collect();

    assert_eq!(
        numbers,
        (1..=100).chain([102, 103]).collect::<Vec<_>>(),
        "a pull shifted across an already-fetched boundary is absent until the next refresh instead of failing the listing"
    );
    server.join().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn list_open_pulls_returns_most_recently_updated_first() {
    let updated = |number: i64, timestamp: &str| {
        let mut pull = pull(number);
        pull["updated_at"] = timestamp.into();
        pull
    };
    let (client, server) = fixture(vec![MockResponse::ok(
        1,
        vec![
            updated(1, "2026-08-20T00:00:00Z"),
            updated(2, "2026-08-24T00:00:00Z"),
            updated(3, "2026-08-22T00:00:00Z"),
        ],
    )]);

    let pulls = client.list_open_pulls("o", "r").await.unwrap();
    let numbers: Vec<_> = pulls.iter().map(|pull| pull.number).collect();

    assert_eq!(
        numbers,
        vec![2, 3, 1],
        "consumers picking one review per branch rely on the freshest pull coming first"
    );
    server.join().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn list_open_pulls_stops_after_a_short_first_page() {
    let (client, server) = fixture(vec![MockResponse::ok(1, vec![pull(1)])]);

    let pulls = client.list_open_pulls("o", "r").await.unwrap();

    assert_eq!(pulls.len(), 1, "a short page is complete");
    server.join().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn list_open_pulls_accepts_exactly_the_page_limit() {
    let mut responses = full_pages(100);
    responses.push(MockResponse::ok(101, Vec::new()));
    let (client, server) = fixture(responses);

    let pulls = client.list_open_pulls("o", "r").await.unwrap();

    assert_eq!(
        pulls.len(),
        10_000,
        "a full page 100 is complete when page 101 is empty"
    );
    server.join().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn list_open_pulls_accepts_a_short_page_past_the_limit() {
    let mut responses = full_pages(100);
    responses.push(MockResponse::ok(101, vec![pull(10_001)]));
    let (client, server) = fixture(responses);

    let pulls = client.list_open_pulls("o", "r").await.unwrap();

    assert_eq!(
        pulls.len(),
        10_001,
        "a short page 101 completes the listing instead of tripping the bound"
    );
    server.join().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn list_open_pulls_fails_when_page_101_is_full() {
    let (client, server) = fixture(full_pages(101));

    let error = client.list_open_pulls("o", "r").await.unwrap_err();

    assert!(
        error
            .to_string()
            .contains("Open pull request listing exceeded 100 pages"),
        "101 full pages mean the listing may extend past the safety bound: {error:#}"
    );
    server.join().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn list_open_pulls_propagates_a_later_page_failure() {
    let (client, server) = fixture(vec![
        MockResponse::ok(1, (1..=100).map(pull).collect()),
        MockResponse {
            page: 2,
            status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            body: json!({ "message": "fixture failure" }),
        },
    ]);

    let error = client.list_open_pulls("o", "r").await.unwrap_err();

    assert!(
        format!("{error:#}").contains("HTTP 500"),
        "a later page error is returned instead of partial pulls: {error:#}"
    );
    server.join().unwrap();
}
