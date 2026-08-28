use std::io::{Read as _, Write as _};
use std::net::TcpListener;

use but_error::AnyhowContextExt as _;
use but_gitlab::{list_known_gitlab_accounts, store_selfhosted_pat};
use but_secret::Sensitive;

struct MockServer(std::thread::JoinHandle<()>);

impl MockServer {
    fn finish(self) {
        self.0.join().expect("mock server should finish");
    }
}

fn mock_gitlab(status: u16, body: &'static str) -> (String, MockServer) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
    let address = listener
        .local_addr()
        .expect("mock server should have an address");
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("mock server should accept");
        let mut request = Vec::new();
        let mut chunk = [0; 1024];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = stream.read(&mut chunk).expect("request should be readable");
            assert_ne!(read, 0, "request should include complete HTTP headers");
            request.extend_from_slice(&chunk[..read]);
        }
        let request = String::from_utf8(request).expect("request should be valid UTF-8");
        assert!(
            request.starts_with("GET /api/v4/user "),
            "PAT validation should request the authenticated user"
        );
        write!(
            stream,
            "HTTP/1.1 {status} Mock\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .expect("mock response should be writable");
    });
    (format!("http://{address}"), MockServer(handle))
}

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
    keyring::set_default_credential_builder(keyring::mock::default_credential_builder());

    run(async {
        for (status, expected_code) in [(401, "GitLabUnauthorized"), (403, "GitLabForbidden")] {
            let (host, server) = mock_gitlab(status, r#"{"message":"rejected"}"#);
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

        let body = r#"{"username":"alice","name":"Alice","email":null,"avatar_url":null}"#;
        let (host, server) = mock_gitlab(200, body);
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
