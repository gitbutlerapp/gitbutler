use but_forgejo::{ForgejoClient, MergeStyle};
use std::io::{Read as _, Write as _};
use std::net::TcpListener;

struct MockResponse {
    status: &'static str,
    headers: &'static str,
    body: String,
}

impl MockResponse {
    fn ok(headers: &'static str, body: impl Into<String>) -> Self {
        MockResponse {
            status: "200 OK",
            headers,
            body: body.into(),
        }
    }
}

/// Serve `responses` in order, one per connection, and hand back each
/// request's first line and body.
fn mock_client(
    responses: Vec<MockResponse>,
) -> (
    ForgejoClient,
    std::thread::JoinHandle<Vec<(String, String)>>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
    let address = listener.local_addr().expect("mock server has an address");
    let handle = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for response in responses {
            let (mut stream, _) = listener.accept().expect("expected another request");
            let mut raw = Vec::new();
            let mut chunk = [0; 4096];
            let header_end = loop {
                let read = stream.read(&mut chunk).expect("mock server reads request");
                assert_ne!(read, 0, "request should include complete HTTP headers");
                raw.extend_from_slice(&chunk[..read]);
                if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                    break pos + 4;
                }
            };
            let head = String::from_utf8_lossy(&raw[..header_end]).to_string();
            let content_length = head
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())?
                })
                .unwrap_or(0);
            while raw.len() < header_end + content_length {
                let read = stream.read(&mut chunk).expect("mock server reads body");
                raw.extend_from_slice(&chunk[..read]);
            }
            let request_line = head.lines().next().unwrap_or_default().to_owned();
            let body = String::from_utf8_lossy(&raw[header_end..]).to_string();
            requests.push((request_line, body));

            write!(
                stream,
                "HTTP/1.1 {}\r\nContent-Type: application/json\r\n{}Content-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.status,
                response.headers,
                response.body.len(),
                response.body
            )
            .expect("mock server writes response");
        }
        requests
    });
    let client = ForgejoClient::new(&Default::default(), &format!("http://{address}"))
        .expect("test client is created");
    (client, handle)
}

fn pr(number: i64) -> serde_json::Value {
    serde_json::json!({
        "number": number,
        "title": format!("PR {number}"),
        "head": {"ref": format!("branch-{number}"), "sha": "0123", "repo_id": 1},
        "base": {"ref": "main", "sha": "4567", "repo_id": 1}
    })
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

#[test]
fn open_pull_requests_follow_pages_until_the_total_is_reached() {
    let page_one: Vec<_> = (1..=50).map(pr).collect();
    let (client, server) = mock_client(vec![
        MockResponse::ok(
            "X-Total-Count: 51\r\n",
            serde_json::to_string(&page_one).unwrap(),
        ),
        MockResponse::ok("X-Total-Count: 51\r\n", format!("[{}]", pr(51))),
    ]);

    let prs = runtime()
        .block_on(client.list_open_prs("alice", "repo"))
        .unwrap();
    let requests = server.join().unwrap();

    assert_eq!(prs.len(), 51, "both pages are collected");
    assert_eq!(
        requests.len(),
        2,
        "the total count ends pagination without an extra empty-page request"
    );
    assert!(
        requests[0].0.starts_with(
            "GET /api/v1/repos/alice/repo/pulls?state=open&sort=recentupdate&limit=50&page=1 "
        ),
        "first request: {}",
        requests[0].0
    );
    assert!(requests[1].0.contains("&page=2 "), "{}", requests[1].0);
}

#[test]
fn pagination_without_a_total_stops_at_an_empty_page() {
    let (client, server) = mock_client(vec![
        MockResponse::ok("", format!("[{}]", pr(1))),
        MockResponse::ok("", "[]"),
    ]);
    let prs = runtime()
        .block_on(client.list_open_prs("alice", "repo"))
        .unwrap();
    assert_eq!(prs.len(), 1);
    assert_eq!(server.join().unwrap().len(), 2);
}

#[test]
fn statuses_for_an_unknown_ref_are_none() {
    let (client, server) = mock_client(vec![MockResponse {
        status: "404 Not Found",
        headers: "",
        body: r#"{"message":"not found"}"#.into(),
    }]);
    let statuses = runtime()
        .block_on(client.list_statuses_for_ref("alice", "repo", "feature/gone"))
        .unwrap();
    let requests = server.join().unwrap();
    assert!(
        statuses.is_none(),
        "a deleted branch means 'no checks', not an error"
    );
    assert!(
        requests[0]
            .0
            .starts_with("GET /api/v1/repos/alice/repo/commits/feature%2Fgone/status?"),
        "branch names are path-encoded: {}",
        requests[0].0
    );
}

#[test]
fn statuses_for_an_unresolvable_ref_are_none_even_on_200() {
    // What Forgejo 15 actually answers for a ref that doesn't exist.
    let (client, server) = mock_client(vec![MockResponse::ok(
        "",
        r#"{"state":"","sha":"","total_count":0,"statuses":null,"repository":null}"#,
    )]);
    let statuses = runtime()
        .block_on(client.list_statuses_for_ref("alice", "repo", "gone"))
        .unwrap();
    server.join().unwrap();
    assert!(
        statuses.is_none(),
        "an empty SHA means the ref didn't resolve"
    );
}

#[test]
fn statuses_carry_the_combined_sha() {
    let (client, server) = mock_client(vec![MockResponse::ok(
        "",
        serde_json::json!({
            "sha": "abc",
            "total_count": 2,
            "statuses": [
                {"id": 9, "context": "CI / test (push)", "status": "success", "target_url": ""},
                {"id": 10, "context": "CI / lint (push)", "status": "failure", "target_url": "/alice/repo/actions/runs/3/jobs/0"}
            ]
        })
        .to_string(),
    )]);
    let statuses = runtime()
        .block_on(client.list_statuses_for_ref("alice", "repo", "main"))
        .unwrap()
        .unwrap();
    server.join().unwrap();
    assert_eq!(statuses.len(), 2);
    assert_eq!(statuses[0].sha, "abc");
    assert_eq!(statuses[0].context, "CI / test (push)");
    assert_eq!(statuses[0].target_url, None, "empty URLs are dropped");
    let target_url = statuses[1].target_url.as_deref().unwrap();
    assert!(
        target_url.starts_with("http://127.0.0.1:")
            && target_url.ends_with("/alice/repo/actions/runs/3/jobs/0"),
        "Actions' instance-relative URLs become absolute: {target_url}"
    );
}

#[test]
fn merging_posts_the_style_as_do() {
    let (client, server) = mock_client(vec![MockResponse::ok("", "")]);
    runtime()
        .block_on(client.merge_pull_request("alice", "repo", 7, MergeStyle::Rebase))
        .unwrap();
    let requests = server.join().unwrap();
    assert!(
        requests[0]
            .0
            .starts_with("POST /api/v1/repos/alice/repo/pulls/7/merge ")
    );
    assert_eq!(requests[0].1, r#"{"Do":"rebase"}"#);
}

#[test]
fn failed_mutations_keep_forgejo_s_message() {
    let (client, server) = mock_client(vec![MockResponse {
        status: "409 Conflict",
        headers: "",
        body: r#"{"message":"pull request already exists for these targets"}"#.into(),
    }]);
    let err = runtime()
        .block_on(
            client.create_pull_request(&but_forgejo::CreatePullRequestParams {
                owner: "alice",
                repo: "repo",
                title: "Add feature",
                body: "",
                head: "feature",
                base: "main",
                draft: true,
            }),
        )
        .unwrap_err();
    let requests = server.join().unwrap();
    assert!(
        format!("{err:#}").contains("already exists"),
        "the user sees why: {err:#}"
    );
    assert!(
        requests[0].1.contains(r#""title":"WIP: Add feature""#),
        "drafts are created with the WIP prefix: {}",
        requests[0].1
    );
}

/// Read-only smoke test against a real instance:
/// `FORGEJO_TEST_HOST=https://git.example.com FORGEJO_TEST_TOKEN=… FORGEJO_TEST_REPO=owner/repo \
///  cargo test -p but-forgejo -- --ignored`
#[test]
#[ignore = "needs a live Forgejo instance"]
fn live_instance_smoke_test() {
    let var = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("{name} must be set"));
    let token = but_secret::Sensitive(var("FORGEJO_TEST_TOKEN"));
    let client = ForgejoClient::new(&token, &var("FORGEJO_TEST_HOST")).unwrap();
    let repo = var("FORGEJO_TEST_REPO");
    let (owner, repo) = repo
        .split_once('/')
        .expect("FORGEJO_TEST_REPO is owner/repo");

    runtime().block_on(async {
        let user = client.get_authenticated().await.expect("token is valid");
        assert!(!user.username.is_empty());
        client
            .list_open_prs(owner, repo)
            .await
            .expect("open pull requests list");
        client.fetch_repo(owner, repo).await.expect("repo metadata");
        let default_branch_statuses = client
            .list_statuses_for_ref(owner, repo, "HEAD")
            .await
            .expect("statuses list");
        eprintln!("statuses on HEAD: {default_branch_statuses:?}");
    });
}
