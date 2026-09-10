use std::io::{Read as _, Write as _};
use std::net::TcpListener;

use but_error::AnyhowContextExt as _;
use but_gitlab::{
    GitLabProjectId, GitlabAccountIdentifier, get_gl_user, list_known_gitlab_accounts,
    store_selfhosted_pat,
};
use but_secret::Sensitive;

/// A process-wide in-memory keyring, so a token persisted by one call can be
/// read back by the next. `keyring::mock` keeps data per entry, which cannot.
mod memory_keyring {
    use std::any::Any;
    use std::collections::BTreeMap;
    use std::sync::{LazyLock, Mutex, Once};

    use keyring::credential::{CredentialApi, CredentialBuilderApi};

    static STORE: LazyLock<Mutex<BTreeMap<String, Vec<u8>>>> = LazyLock::new(Default::default);

    struct Entry(String);

    impl CredentialApi for Entry {
        fn set_secret(&self, secret: &[u8]) -> keyring::Result<()> {
            STORE
                .lock()
                .unwrap()
                .insert(self.0.clone(), secret.to_vec());
            Ok(())
        }

        fn get_secret(&self) -> keyring::Result<Vec<u8>> {
            STORE
                .lock()
                .unwrap()
                .get(&self.0)
                .cloned()
                .ok_or(keyring::Error::NoEntry)
        }

        fn delete_credential(&self) -> keyring::Result<()> {
            STORE.lock().unwrap().remove(&self.0);
            Ok(())
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    struct Builder;

    impl CredentialBuilderApi for Builder {
        fn build(
            &self,
            _target: Option<&str>,
            service: &str,
            user: &str,
        ) -> keyring::Result<Box<keyring::Credential>> {
            Ok(Box::new(Entry(format!("{service}\0{user}"))))
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    pub fn install() {
        static ONCE: Once = Once::new();
        ONCE.call_once(|| keyring::set_default_credential_builder(Box::new(Builder)));
    }
}

struct MockServer(std::thread::JoinHandle<()>);

impl MockServer {
    fn finish(self) {
        self.0.join().expect("mock server should finish");
    }
}

/// Read one request's headers and return its target (path and query).
fn request_target(stream: &mut std::net::TcpStream) -> String {
    let mut request = Vec::new();
    let mut chunk = [0; 1024];
    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
        let read = stream.read(&mut chunk).expect("request should be readable");
        assert_ne!(read, 0, "request should include complete HTTP headers");
        request.extend_from_slice(&chunk[..read]);
    }
    let request = String::from_utf8(request).expect("request should be valid UTF-8");
    request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .expect("request should include a target")
        .to_owned()
}

/// Serve `responses` in order, one per connection, to `GET /api/v4/user`.
fn mock_gitlab(responses: Vec<(u16, &'static str)>) -> (String, MockServer) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
    let address = listener
        .local_addr()
        .expect("mock server should have an address");
    let handle = std::thread::spawn(move || {
        for (status, body) in responses {
            let (mut stream, _) = listener.accept().expect("mock server should accept");
            assert_eq!(
                request_target(&mut stream),
                "/api/v4/user",
                "PAT validation should request the authenticated user"
            );
            write!(
                stream,
                "HTTP/1.1 {status} Mock\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .expect("mock response should be writable");
        }
    });
    (format!("http://{address}"), MockServer(handle))
}

const ALICE: &str = r#"{"username":"alice","name":"Alice","email":null,"avatar_url":null}"#;
const REJECTED: &str = r#"{"message":"rejected"}"#;

fn mock_tls_failure() -> (String, MockServer) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
    let address = listener
        .local_addr()
        .expect("mock server should have an address");
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("mock server should accept");
        let mut client_hello = [0; 1024];
        assert_ne!(
            stream
                .read(&mut client_hello)
                .expect("TLS handshake should be readable"),
            0,
            "client should start a TLS handshake"
        );
        stream
            .write_all(b"not a TLS response")
            .expect("invalid TLS response should be writable");
    });
    (format!("https://{address}"), MockServer(handle))
}

fn run(future: impl std::future::Future<Output = ()>) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime should be created")
        .block_on(future);
}

fn assert_no_credentials(storage: &but_forge_storage::Controller) {
    assert!(
        list_known_gitlab_accounts(storage)
            .expect("stored accounts should be readable")
            .is_empty(),
        "failed validation must not store credentials"
    );
}

#[test]
fn self_hosted_pat_validation_distinguishes_auth_transport_and_success() {
    memory_keyring::install();

    run(async {
        for (status, expected_code) in [(401, "GitLabUnauthorized"), (403, "GitLabForbidden")] {
            let (host, server) = mock_gitlab(vec![(status, REJECTED)]);
            let dir = tempfile::tempdir().expect("temporary storage should be created");
            let storage = but_forge_storage::Controller::from_path(dir.path());
            let error = store_selfhosted_pat(&host, &Sensitive("bad-token".into()), &storage)
                .await
                .expect_err("rejected PAT should fail validation");
            server.finish();

            assert_eq!(
                error.custom_context_or_error_chain().code.to_string(),
                expected_code,
                "HTTP auth status should survive the API boundary"
            );
            assert_no_credentials(&storage);
        }

        let (host, server) = mock_tls_failure();
        let dir = tempfile::tempdir().expect("temporary storage should be created");
        let storage = but_forge_storage::Controller::from_path(dir.path());
        let error = store_selfhosted_pat(&host, &Sensitive("token".into()), &storage)
            .await
            .expect_err("TLS failure should fail validation");
        server.finish();
        assert_eq!(
            error.custom_context_or_error_chain().code.to_string(),
            "Unknown",
            "TLS failure should retain the existing fallback classification"
        );
        assert_no_credentials(&storage);

        let (host, server) = mock_gitlab(vec![(200, ALICE)]);
        let dir = tempfile::tempdir().expect("temporary storage should be created");
        let storage = but_forge_storage::Controller::from_path(dir.path());
        let response = store_selfhosted_pat(&host, &Sensitive("good-token".into()), &storage)
            .await
            .expect("valid PAT should be stored");
        server.finish();
        assert_eq!(
            response.username, "alice",
            "authenticated user should be returned"
        );
        assert_eq!(
            list_known_gitlab_accounts(&storage)
                .expect("stored accounts should be readable")
                .len(),
            1,
            "successful validation should store credentials"
        );
    });
}

#[test]
fn self_hosted_pat_validation_rejects_host_without_scheme_before_any_request() {
    memory_keyring::install();

    run(async {
        for host in [
            "gitlab.example.com",
            "gitlab.example.com/api/v4",
            "localhost:8080",
        ] {
            let dir = tempfile::tempdir().expect("temporary storage should be created");
            let storage = but_forge_storage::Controller::from_path(dir.path());
            let error = store_selfhosted_pat(host, &Sensitive("token".into()), &storage)
                .await
                .expect_err("host without a scheme should fail validation");
            let context = error.custom_context_or_error_chain();
            assert_eq!(
                context.code.to_string(),
                "GitLabInvalidHost",
                "a host without a scheme should be classified for the host field"
            );
            assert!(
                context.to_string().contains(host),
                "error should name the host, got: {context}"
            );
            assert_no_credentials(&storage);
        }
    });
}

#[test]
fn stored_account_refresh_classifies_auth_rejection_and_clears_cache() {
    memory_keyring::install();

    run(async {
        for (status, expected_code) in [(401, "GitLabUnauthorized"), (403, "GitLabForbidden")] {
            let (host, server) = mock_gitlab(vec![(200, ALICE), (status, REJECTED)]);
            let dir = tempfile::tempdir().expect("temporary storage should be created");
            let storage = but_forge_storage::Controller::from_path(dir.path());
            store_selfhosted_pat(&host, &Sensitive("token".into()), &storage)
                .await
                .expect("valid PAT should be stored");
            let account = GitlabAccountIdentifier::selfhosted("alice", &host);
            let cached_profile = || {
                storage
                    .cached_profile(&account.cache_key())
                    .expect("cached profile should be readable")
            };
            assert!(
                cached_profile().is_some(),
                "successful validation should cache the profile"
            );

            let error = get_gl_user(&account, &storage)
                .await
                .expect_err("rejected stored token should fail the refresh");
            server.finish();
            assert_eq!(
                error.custom_context_or_error_chain().code.to_string(),
                expected_code,
                "a stored-token rejection should carry the same code as PAT validation"
            );
            assert!(
                cached_profile().is_none(),
                "auth rejection should clear the cached profile"
            );
        }
    });
}

/// Forgetting must not depend on the token: a different build kind may have stored it,
/// or the keychain may refuse the read, and the account still has to go.
#[test]
fn forget_removes_an_account_whose_token_is_missing() {
    memory_keyring::install();
    let dir = tempfile::tempdir().unwrap();
    let storage = but_forge_storage::Controller::from_path(dir.path());
    storage
        .add_gitlab_account(&but_forge_storage::settings::GitLabAccount::Pat {
            username: "alice".into(),
            access_token_key: "gitlab_pat_alice".into(),
        })
        .unwrap();
    let account = GitlabAccountIdentifier::pat("alice");
    assert_eq!(
        list_known_gitlab_accounts(&storage).unwrap(),
        std::slice::from_ref(&account)
    );

    but_gitlab::forget_gl_access_token(&account, &storage).unwrap();

    assert_eq!(list_known_gitlab_accounts(&storage).unwrap(), []);
}

/// One canned reply for the read mock; `next_page` is served as `X-Next-Page`.
struct Reply {
    status: u16,
    body: String,
    next_page: Option<&'static str>,
}

fn reply(status: u16, body: impl Into<String>) -> Reply {
    Reply {
        status,
        body: body.into(),
        next_page: None,
    }
}

fn page(body: impl Into<String>, next_page: &'static str) -> Reply {
    Reply {
        next_page: Some(next_page),
        ..reply(200, body)
    }
}

struct ReadMockServer(std::thread::JoinHandle<Vec<String>>);

impl ReadMockServer {
    /// The request targets served, in order.
    fn finish(self) -> Vec<String> {
        self.0.join().expect("read mock server should finish")
    }
}

/// Serve `replies` in order, one per connection, to any path. A reply the
/// client never asks for fails the test instead of hanging it.
fn mock_gitlab_reads(replies: Vec<Reply>) -> (String, ReadMockServer) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
    listener
        .set_nonblocking(true)
        .expect("mock server should be nonblocking");
    let address = listener
        .local_addr()
        .expect("mock server should have an address");
    let handle = std::thread::spawn(move || {
        let mut targets = Vec::new();
        for reply in replies {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(err)
                        if err.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                    Err(err) => panic!("expected another request: {err}"),
                }
            };
            stream
                .set_nonblocking(false)
                .expect("accepted stream should be blocking");
            targets.push(request_target(&mut stream));
            write!(
                stream,
                "HTTP/1.1 {} Mock\r\nContent-Type: application/json\r\nX-Next-Page: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                reply.status,
                reply.next_page.unwrap_or_default(),
                reply.body.len(),
                reply.body
            )
            .expect("mock response should be writable");
        }
        targets
    });
    (format!("http://{address}"), ReadMockServer(handle))
}

const TOKEN: &str = "synthetic-token-that-must-not-leak";
const MERGE_STATUS: &str = r#"{"merge_status":"can_be_merged","user_notes_count":0}"#;
const PIPELINE: &str = r#"{"id":5,"status":"success","web_url":null}"#;
const PROJECT: &str = r#"{"id":7,"path_with_namespace":"group/repo","ssh_url_to_repo":"git@gitlab.example:group/repo.git","http_url_to_repo":"https://gitlab.example/group/repo.git","default_branch":"main"}"#;

fn mr(iid: i64) -> String {
    format!(
        r#"{{"web_url":"https://gitlab.example/mr/{iid}","iid":{iid},"title":"MR {iid}","description":null,"author":null,"labels":[],"draft":false,"source_branch":"topic-{iid}","target_branch":"main","sha":"0123456789abcdef0123456789abcdef01234567","merge_commit_sha":null,"squash_commit_sha":null,"created_at":null,"updated_at":null,"merged_at":null,"closed_at":null,"project_id":7,"source_project_id":7,"target_project_id":7,"assignees":[],"reviewers":[],"merge_when_pipeline_succeeds":false}}"#
    )
}

fn job(id: i64) -> String {
    format!(
        r#"{{"id":{id},"name":"job-{id}","status":"success","allow_failure":false,"started_at":null,"finished_at":null,"web_url":null,"pipeline":{{"id":5,"web_url":null,"status":"success"}}}}"#
    )
}

/// A self-hosted account stored against the read mock, so every read below
/// resolves its client through the same stored-credential lookup the API uses.
struct StoredAccount {
    server: ReadMockServer,
    storage: but_forge_storage::Controller,
    account: GitlabAccountIdentifier,
    host: String,
    _dir: tempfile::TempDir,
}

async fn stored_account(replies: Vec<Reply>) -> StoredAccount {
    let mut all = vec![reply(200, ALICE)];
    all.extend(replies);
    let (host, server) = mock_gitlab_reads(all);
    let dir = tempfile::tempdir().expect("temporary storage should be created");
    let storage = but_forge_storage::Controller::from_path(dir.path());
    store_selfhosted_pat(&host, &Sensitive(TOKEN.into()), &storage)
        .await
        .expect("valid PAT should be stored");
    StoredAccount {
        server,
        storage,
        account: GitlabAccountIdentifier::selfhosted("alice", &host),
        host,
        _dir: dir,
    }
}

/// The public read wrappers the desktop polls through `list_reviews`,
/// `get_review` and `list_ci_checks`.
#[derive(Debug, Clone, Copy)]
enum Read {
    OpenList,
    TargetList,
    CommitList,
    RecentlyClosed,
    Get,
    MergeStatus,
    PipelineJobs,
}

/// Perform `read` through the stored account and report how many items it returned.
async fn perform(
    read: Read,
    storage: &but_forge_storage::Controller,
    account: &GitlabAccountIdentifier,
) -> anyhow::Result<usize> {
    let account = Some(account);
    let project = GitLabProjectId::new("group", "repo");
    Ok(match read {
        Read::OpenList => but_gitlab::mr::list(account, project, storage).await?.len(),
        Read::TargetList => but_gitlab::mr::list_all_for_target(account, project, "main", storage)
            .await?
            .len(),
        Read::CommitList => {
            but_gitlab::mr::list_for_commit(account, project, "0123456789abcdef", storage)
                .await?
                .len()
        }
        Read::RecentlyClosed => but_gitlab::mr::list_recently_closed(account, project, storage)
            .await?
            .len(),
        Read::Get => {
            but_gitlab::mr::get(account, project, 1, storage).await?;
            1
        }
        Read::MergeStatus => {
            but_gitlab::mr::get_merge_status(account, project, 1, storage).await?;
            1
        }
        Read::PipelineJobs => {
            but_gitlab::checks::list_pipeline_jobs_for_ref(account, project, "main", storage)
                .await?
                .len()
        }
    })
}

#[test]
fn rejected_token_on_read_paths_carries_the_unauthorized_code_on_every_seam() {
    memory_keyring::install();

    run(async {
        let seams: Vec<(Read, Vec<Reply>)> = vec![
            (Read::OpenList, vec![reply(401, REJECTED)]),
            (
                Read::OpenList,
                vec![page(format!("[{}]", mr(1)), "2"), reply(401, REJECTED)],
            ),
            (Read::TargetList, vec![reply(401, REJECTED)]),
            (Read::CommitList, vec![reply(401, REJECTED)]),
            (Read::RecentlyClosed, vec![reply(401, REJECTED)]),
            (
                Read::RecentlyClosed,
                vec![reply(200, "[]"), reply(401, REJECTED)],
            ),
            (Read::Get, vec![reply(401, REJECTED)]),
            (Read::MergeStatus, vec![reply(401, REJECTED)]),
            (Read::PipelineJobs, vec![reply(401, REJECTED)]),
            (
                Read::PipelineJobs,
                vec![reply(200, PIPELINE), reply(401, REJECTED)],
            ),
            (
                Read::PipelineJobs,
                vec![
                    reply(200, PIPELINE),
                    page(format!("[{}]", job(1)), "2"),
                    reply(401, REJECTED),
                ],
            ),
        ];
        for (read, replies) in seams {
            let requests = replies.len() + 1;
            let fixture = stored_account(replies).await;
            let error = perform(read, &fixture.storage, &fixture.account)
                .await
                .expect_err("a rejected token must fail the read, never return a partial page");
            let StoredAccount { server, host, .. } = fixture;
            assert_eq!(
                server.finish().len(),
                requests,
                "{read:?}: every page up to the rejection should have been requested"
            );

            let context = error.custom_context_or_error_chain();
            assert_eq!(
                context.code.to_string(),
                "GitLabUnauthorized",
                "{read:?}: a 401 on a read path is the same rejected token the account refresh reports: {error:#}"
            );
            assert_eq!(
                context.message.as_deref(),
                Some("GitLab did not accept the token."),
                "{read:?}: the API sends only the static guidance"
            );
            let chain = format!("{error:#}");
            assert!(
                chain.contains("HTTP 401"),
                "{read:?}: the status should stay in the chain for logs: {chain}"
            );
            let debug = format!("{error:?}");
            assert!(
                !debug.contains(TOKEN) && !debug.contains(&host),
                "{read:?}: neither the token nor the host may reach the error: {debug}"
            );
        }
    });
}

#[test]
fn read_failures_other_than_a_rejected_token_stay_unclassified() {
    memory_keyring::install();

    run(async {
        let cases: Vec<(Read, Vec<Reply>, &str)> = vec![
            (Read::OpenList, vec![reply(403, REJECTED)], "403"),
            (Read::OpenList, vec![reply(404, REJECTED)], "404"),
            (Read::OpenList, vec![reply(429, REJECTED)], "429"),
            // Auth wording in the body of another status is not a rejection.
            (
                Read::OpenList,
                vec![reply(500, r#"{"message":"401 Unauthorized"}"#)],
                "500",
            ),
            (
                Read::OpenList,
                vec![reply(200, "not json")],
                "error decoding response body",
            ),
            (Read::Get, vec![reply(500, REJECTED)], "500"),
            (Read::MergeStatus, vec![reply(403, REJECTED)], "403"),
            (Read::PipelineJobs, vec![reply(500, REJECTED)], "500"),
            (
                Read::PipelineJobs,
                vec![reply(200, PIPELINE), reply(403, REJECTED)],
                "403",
            ),
        ];
        for (read, replies, detail) in cases {
            let fixture = stored_account(replies).await;
            let error = perform(read, &fixture.storage, &fixture.account)
                .await
                .expect_err("a failed read should surface");
            fixture.server.finish();
            let context = error.custom_context_or_error_chain();
            assert_eq!(
                context.code.to_string(),
                "Unknown",
                "{read:?}: only a rejected token is terminal for the poller: {error:#}"
            );
            assert!(
                context
                    .message
                    .as_deref()
                    .is_some_and(|message| message.contains(detail)),
                "{read:?}: an unclassified failure keeps its detail for the user: {error:#}"
            );
        }

        // The stored account outlives its mock: the next read is refused.
        let fixture = stored_account(vec![]).await;
        fixture.server.finish();
        let error = perform(Read::OpenList, &fixture.storage, &fixture.account)
            .await
            .expect_err("an unreachable host should fail the read");
        assert_eq!(
            error.custom_context_or_error_chain().code.to_string(),
            "NetworkError",
            "transport failures keep their existing outage classification: {error:#}"
        );
    });
}

#[test]
fn ci_reads_keep_treating_a_forbidden_or_missing_pipeline_as_no_checks() {
    memory_keyring::install();

    run(async {
        for status in [403, 404] {
            let fixture = stored_account(vec![reply(status, REJECTED)]).await;
            let jobs = perform(Read::PipelineJobs, &fixture.storage, &fixture.account)
                .await
                .expect("a pipeline GitLab hides or lacks is an empty check list");
            fixture.server.finish();
            assert_eq!(jobs, 0, "HTTP {status} on the latest pipeline stays empty");
        }
    });
}

#[test]
fn read_paths_still_return_data_across_pages() {
    memory_keyring::install();

    run(async {
        let cases: Vec<(Read, Vec<Reply>, usize)> = vec![
            (
                Read::OpenList,
                vec![
                    page(format!("[{}]", mr(1)), "2"),
                    reply(200, format!("[{}]", mr(2))),
                ],
                2,
            ),
            (
                Read::RecentlyClosed,
                vec![
                    reply(200, format!("[{}]", mr(3))),
                    reply(200, format!("[{}]", mr(4))),
                ],
                2,
            ),
            (Read::Get, vec![reply(200, mr(1)), reply(200, PROJECT)], 1),
            (Read::MergeStatus, vec![reply(200, MERGE_STATUS)], 1),
            (
                Read::PipelineJobs,
                vec![
                    reply(200, PIPELINE),
                    page(format!("[{}]", job(1)), "2"),
                    reply(200, format!("[{}]", job(2))),
                ],
                2,
            ),
        ];
        for (read, replies, expected) in cases {
            let requests = replies.len() + 1;
            let fixture = stored_account(replies).await;
            let items = perform(read, &fixture.storage, &fixture.account)
                .await
                .expect("successful reads should still return data");
            assert_eq!(
                fixture.server.finish().len(),
                requests,
                "{read:?}: every advertised page should be requested"
            );
            assert_eq!(items, expected, "{read:?}: all pages should be returned");
        }
    });
}
