use but_gitlab::{GitLabClient, GitLabProjectId};
use reqwest::Url;
use std::io::{ErrorKind, Read as _, Write as _};
use std::net::TcpListener;
use std::time::{Duration, Instant};

struct MockResponse {
    body: String,
    next_page: Option<&'static str>,
}

struct MockServer {
    handle: std::thread::JoinHandle<Vec<String>>,
}

impl MockServer {
    fn finish(self) -> Vec<String> {
        self.handle.join().expect("mock server should finish")
    }
}

fn mock_client(responses: Vec<MockResponse>) -> (GitLabClient, MockServer) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
    listener
        .set_nonblocking(true)
        .expect("mock server should be nonblocking");
    let address = listener
        .local_addr()
        .expect("mock server should have an address");
    let handle = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for response in responses {
            let deadline = Instant::now() + Duration::from_secs(2);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(err)
                        if err.kind() == ErrorKind::WouldBlock && Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(err) => panic!("expected another paginated request: {err}"),
                }
            };
            stream
                .set_nonblocking(false)
                .expect("accepted stream should be blocking");

            let mut request = Vec::new();
            let mut chunk = [0; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = stream
                    .read(&mut chunk)
                    .expect("mock server should read the request");
                assert_ne!(read, 0, "request should include complete HTTP headers");
                request.extend_from_slice(&chunk[..read]);
            }
            let request = String::from_utf8(request).expect("request should be valid UTF-8");
            let path = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .expect("request should include a path")
                .to_owned();
            requests.push(path);

            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nX-Next-Page: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.next_page.unwrap_or_default(),
                response.body.len(),
                response.body
            )
            .expect("mock server should write the response");
        }
        requests
    });

    let token = Default::default();
    let client = GitLabClient::new_with_host_override(&token, &format!("http://{address}"))
        .expect("test client should be created");
    (client, MockServer { handle })
}

fn mr(iid: i64, source_branch: &str) -> String {
    format!(
        r#"{{"web_url":"https://gitlab.example/mr/{iid}","iid":{iid},"title":"MR {iid}","description":null,"author":null,"labels":[],"draft":false,"source_branch":"{source_branch}","target_branch":"main","sha":"0123456789abcdef0123456789abcdef01234567","merge_commit_sha":null,"squash_commit_sha":null,"created_at":null,"updated_at":null,"merged_at":null,"closed_at":null,"project_id":7,"source_project_id":7,"target_project_id":7,"assignees":[],"reviewers":[],"merge_when_pipeline_succeeds":false}}"#
    )
}

fn page(mrs: &[String]) -> String {
    format!("[{}]", mrs.join(","))
}

fn assert_requests(
    requests: &[String],
    endpoint: &str,
    expected_query: &[(&str, &str)],
    pages: &[&str],
) {
    assert_eq!(
        requests.len(),
        pages.len(),
        "one request should be made for each advertised page"
    );
    for (request, page) in requests.iter().zip(pages) {
        let url = Url::parse(&format!("http://localhost{request}"))
            .expect("MR listing request should be a valid URL");
        assert_eq!(
            url.path(),
            endpoint,
            "request should use the expected MR listing endpoint"
        );
        let mut actual_query = url
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();
        let mut expected_query = expected_query
            .iter()
            .copied()
            .chain([("page", *page), ("per_page", "100")])
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect::<Vec<_>>();
        actual_query.sort_unstable();
        expected_query.sort_unstable();
        assert_eq!(
            actual_query, expected_query,
            "MR listing should contain exactly the expected decoded query pairs: {request}"
        );
    }
}

fn run(future: impl std::future::Future<Output = ()>) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime should be created")
        .block_on(future);
}

#[test]
fn list_open_mrs_includes_a_page_two_match_without_duplicates() {
    run(async {
        let first = mr(1, "other");
        let duplicate = mr(1, "other");
        let matching = mr(2, "mm/15-client");
        let (client, server) = mock_client(vec![
            MockResponse {
                body: page(&[first]),
                next_page: Some("2"),
            },
            MockResponse {
                body: page(&[duplicate, matching]),
                next_page: None,
            },
        ]);

        let mrs = client
            .list_open_mrs(GitLabProjectId::new("group", "repo"))
            .await
            .expect("all open MR pages should be listed");
        assert_eq!(
            mrs.iter().map(|mr| mr.iid).collect::<Vec<_>>(),
            vec![1, 2],
            "overlapping pages should not duplicate merge requests"
        );
        assert_eq!(
            mrs.iter()
                .find(|mr| mr.source_branch == "mm/15-client")
                .map(|mr| mr.iid),
            Some(2),
            "an existing page-two MR should be visible to branch association"
        );
        let requests = server.finish();
        assert_requests(
            &requests,
            "/api/v4/projects/group%2Frepo/merge_requests",
            &[("state", "opened"), ("order_by", "created_at")],
            &["1", "2"],
        );
    });
}

#[test]
fn list_mrs_for_target_includes_page_two() {
    run(async {
        let (client, server) = mock_client(vec![
            MockResponse {
                body: page(&[mr(1, "first")]),
                next_page: Some("2"),
            },
            MockResponse {
                body: page(&[mr(2, "second")]),
                next_page: None,
            },
        ]);

        let mrs = client
            .list_mrs_for_target(GitLabProjectId::new("group", "repo"), "main")
            .await
            .expect("all target MR pages should be listed");
        assert_eq!(
            mrs.iter().map(|mr| mr.iid).collect::<Vec<_>>(),
            vec![1, 2],
            "target listings should include page two"
        );
        let requests = server.finish();
        assert_requests(
            &requests,
            "/api/v4/projects/group%2Frepo/merge_requests",
            &[
                ("state", "all"),
                ("target_branch", "main"),
                ("order_by", "updated_at"),
                ("sort", "desc"),
            ],
            &["1", "2"],
        );
    });
}

#[test]
fn list_mrs_for_commit_includes_page_two() {
    run(async {
        let (client, server) = mock_client(vec![
            MockResponse {
                body: page(&[mr(1, "first")]),
                next_page: Some("2"),
            },
            MockResponse {
                body: page(&[mr(2, "second")]),
                next_page: None,
            },
        ]);

        let mrs = client
            .list_mrs_for_commit(
                GitLabProjectId::new("group", "repo"),
                "0123456789abcdef0123456789abcdef01234567",
            )
            .await
            .expect("all commit MR pages should be listed");
        assert_eq!(
            mrs.iter().map(|mr| mr.iid).collect::<Vec<_>>(),
            vec![1, 2],
            "commit listings should include page two"
        );
        assert_requests(
            &server.finish(),
            "/api/v4/projects/group%2Frepo/repository/commits/0123456789abcdef0123456789abcdef01234567/merge_requests",
            &[],
            &["1", "2"],
        );
    });
}

#[test]
fn list_open_mrs_stops_after_a_normal_first_page() {
    run(async {
        let (client, server) = mock_client(vec![MockResponse {
            body: page(&[mr(1, "first")]),
            next_page: None,
        }]);

        let mrs = client
            .list_open_mrs(GitLabProjectId::new("group", "repo"))
            .await
            .expect("a normal first page should be listed");
        assert_eq!(mrs.len(), 1, "the first page should be returned unchanged");
        assert_requests(
            &server.finish(),
            "/api/v4/projects/group%2Frepo/merge_requests",
            &[("state", "opened"), ("order_by", "created_at")],
            &["1"],
        );
    });
}

#[test]
fn list_open_mrs_stops_on_an_empty_page() {
    run(async {
        let (client, server) = mock_client(vec![
            MockResponse {
                body: page(&[mr(1, "first")]),
                next_page: Some("2"),
            },
            MockResponse {
                body: page(&[]),
                next_page: Some("3"),
            },
        ]);

        let mrs = client
            .list_open_mrs(GitLabProjectId::new("group", "repo"))
            .await
            .expect("an empty page should terminate pagination");
        assert_eq!(
            mrs.iter().map(|mr| mr.iid).collect::<Vec<_>>(),
            vec![1],
            "an empty page should preserve earlier results"
        );
        assert_requests(
            &server.finish(),
            "/api/v4/projects/group%2Frepo/merge_requests",
            &[("state", "opened"), ("order_by", "created_at")],
            &["1", "2"],
        );
    });
}

#[test]
fn list_open_mrs_rejects_a_repeated_page() {
    run(async {
        let (client, server) = mock_client(vec![
            MockResponse {
                body: page(&[mr(1, "first")]),
                next_page: Some("2"),
            },
            MockResponse {
                body: page(&[mr(2, "second")]),
                next_page: Some("2"),
            },
        ]);

        let error = client
            .list_open_mrs(GitLabProjectId::new("group", "repo"))
            .await
            .expect_err("a repeated page token should terminate with an error");
        assert!(
            error.to_string().contains("unsafe pagination state"),
            "repeated pagination should return a bounded error: {error:#}"
        );
        assert_requests(
            &server.finish(),
            "/api/v4/projects/group%2Frepo/merge_requests",
            &[("state", "opened"), ("order_by", "created_at")],
            &["1", "2"],
        );
    });
}
