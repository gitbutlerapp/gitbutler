use std::{
    io::{Read as _, Write as _},
    net::TcpListener,
    time::{Duration, Instant},
};

use but_github::GitHubClient;
use but_secret::Sensitive;
use serde_json::json;

#[derive(Clone)]
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
                    ("direction".to_string(), "desc".to_string()),
                    ("page".to_string(), response.page.to_string()),
                    ("per_page".to_string(), "100".to_string()),
                    ("sort".to_string(), "updated".to_string()),
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
    let first_page = (1..=100).map(pull).collect();
    let (client, server) = fixture(vec![
        MockResponse::ok(1, first_page),
        MockResponse::ok(2, vec![pull(101)]),
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

#[tokio::test(flavor = "current_thread")]
async fn list_open_pulls_checks_after_an_exact_page() {
    let (client, server) = fixture(vec![
        MockResponse::ok(1, (1..=100).map(pull).collect()),
        MockResponse::ok(2, Vec::new()),
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
async fn list_open_pulls_retries_after_insertion_drift() {
    let (client, server) = fixture(vec![
        MockResponse::ok(1, (1..=100).map(pull).collect()),
        MockResponse::ok(2, vec![pull(100), pull(102)]),
        MockResponse::ok(1, (1..=100).map(pull).collect()),
        MockResponse::ok(2, vec![pull(101), pull(102)]),
        MockResponse::ok(1, (1..=100).map(pull).collect()),
        MockResponse::ok(2, vec![pull(101), pull(102)]),
    ]);

    let pulls = client.list_open_pulls("o", "r").await.unwrap();
    let numbers: Vec<_> = pulls.iter().map(|pull| pull.number).collect();

    assert_eq!(
        numbers,
        (1..=102).collect::<Vec<_>>(),
        "the retry returns the complete listing rather than stale first-scan data"
    );
    server.join().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn list_open_pulls_retries_after_removal_page_drift() {
    let current_first_page = (1..=100).map(pull).collect::<Vec<_>>();
    let current_second_page = (102..=201).map(pull).collect::<Vec<_>>();
    let current_third_page = vec![pull(202), pull(203)];
    let (client, server) = fixture(vec![
        MockResponse::ok(1, (1..=100).map(pull).collect()),
        MockResponse::ok(2, (101..=200).map(pull).collect()),
        MockResponse::ok(3, current_third_page.clone()),
        MockResponse::ok(1, current_first_page.clone()),
        MockResponse::ok(2, current_second_page),
        MockResponse::ok(3, current_third_page.clone()),
        MockResponse::ok(1, current_first_page),
        MockResponse::ok(2, (102..=201).map(pull).collect()),
        MockResponse::ok(3, current_third_page),
    ]);

    let pulls = client.list_open_pulls("o", "r").await.unwrap();
    let numbers: Vec<_> = pulls.iter().map(|pull| pull.number).collect();

    assert_eq!(
        numbers,
        (1..=100).chain(102..=203).collect::<Vec<_>>(),
        "the retry drops the closed pull and restores the shifted pull"
    );
    server.join().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn list_open_pulls_fails_after_repeated_page_drift() {
    let responses = vec![
        MockResponse::ok(1, (1..=100).map(pull).collect()),
        MockResponse::ok(2, vec![pull(100), pull(102)]),
    ];
    let mut repeated = responses.clone();
    repeated.extend(responses);
    let (client, server) = fixture(repeated);

    let error = client.list_open_pulls("o", "r").await.unwrap_err();

    assert!(
        error
            .to_string()
            .contains("Open pull request listing changed while paginating"),
        "repeated drift must error instead of returning a partial listing: {error:#}"
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
    let mut scan = (0..100)
        .map(|page| {
            MockResponse::ok(
                page + 1,
                (page * 100 + 1..=(page + 1) * 100)
                    .map(|number| pull(number as i64))
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    scan.push(MockResponse::ok(101, Vec::new()));
    let mut responses = scan.clone();
    responses.extend(scan);
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
async fn list_open_pulls_fails_when_page_101_is_nonempty() {
    let mut responses = (0..100)
        .map(|page| {
            MockResponse::ok(
                page + 1,
                (page * 100 + 1..=(page + 1) * 100)
                    .map(|number| pull(number as i64))
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    responses.push(MockResponse::ok(101, vec![pull(10_001)]));
    let (client, server) = fixture(responses);

    let error = client.list_open_pulls("o", "r").await.unwrap_err();

    assert!(
        error
            .to_string()
            .contains("Open pull request listing exceeded 100 pages"),
        "a nonempty page 101 exceeds the safety bound: {error:#}"
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
