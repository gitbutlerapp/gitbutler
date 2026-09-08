use std::io::{Read as _, Write as _};
use std::net::TcpListener;

use but_error::AnyhowContextExt as _;
use but_gitlab::{
    GitlabAccountIdentifier, get_gl_user, list_known_gitlab_accounts, store_selfhosted_pat,
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

/// Serve `responses` in order, one per connection, to `GET /api/v4/user`.
fn mock_gitlab(responses: Vec<(u16, &'static str)>) -> (String, MockServer) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
    let address = listener
        .local_addr()
        .expect("mock server should have an address");
    let handle = std::thread::spawn(move || {
        for (status, body) in responses {
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
