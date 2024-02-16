pub(crate) mod private;

/// To use in a backend, create a function that initializes
/// an empty repository, whatever that looks like, and returns
/// something that implements the `Repository` trait.
///
/// Include this file via
/// `include!(concat!(env!("CARGO_MANIFEST_DIR"), "/integration-tests.rs"));`
///
/// Then, pass the function to `gitbutler_git_integration_tests!(fn)`, like so:
///
/// ```
/// #[cfg(test)]
/// mod tests {
///     async fn make_repo(test_name: String) -> impl crate::Repository {
///         // Use `test_name` to create a unique repository, if needed.
///         todo!();
///     }
///
///    crate::gitbutler_git_integration_tests!(make_repo);
/// }
/// ```
#[allow(unused_macros)]
macro_rules! gitbutler_git_integration_tests {
    ($create_repo:expr, $io_tests:tt) => {
        $crate::private::test_impl! {
            $create_repo, enable_io,

            async fn create_repo_selftest(repo) {
                // Do-nothing, just a selftest.
            }

            async fn non_existent_remote(repo) {
                use crate::*;
                match repo.remote("non-existent").await.unwrap_err() {
                    Error::NoSuchRemote(remote, _) => assert_eq!(remote, "non-existent"),
                    err => panic!("expected NoSuchRemote, got {:?}", err),
                }
            }

            async fn create_remote(repo) {
                use crate::*;

                match repo.remote("origin").await {
                    Err($crate::Error::NoSuchRemote(remote, _)) if remote == "origin" => {},
                    result => panic!("expected remote 'origin' query to fail with NoSuchRemote, but got {result:?}")
                }

                repo.create_remote("origin", "https://example.com/test.git").await.unwrap();

                assert_eq!(repo.remote("origin").await.unwrap(), "https://example.com/test.git".to_owned());
            }

            async fn get_head_no_commits(repo) {
                use crate::*;
                assert!(repo.head().await.is_err());
            }

            async fn get_symbolic_head_no_commits(repo) {
                use crate::*;
                assert!(repo.symbolic_head().await.is_err());
            }

            // DO NOT ADD IO TESTS HERE. THIS IS THE WRONG SPOT.
        }

        $crate::private::test_impl! {
            $create_repo, $io_tests,

            async fn fetch_with_ssh_basic_bad_password(repo, server, server_repo) {
                use crate::*;

                server.allow_authorization(Authorization::Basic {
                    username: Some("my_username".to_owned()),
                    password: Some("my_password".to_owned())
                });

                server.run_with_server(async move |port| {
                    repo.create_remote("origin", &format!("[my_username@localhost:{port}]:test.git")).await.unwrap();

                    let err = repo.fetch(
                        "origin",
                        RefSpec{
                            source: Some("refs/heads/master".to_owned()),
                            destination: Some("refs/heads/master".to_owned()),
                            ..Default::default()
                        },
                        &Authorization::Basic {
                            username: Some("my_username".to_owned()),
                            password: Some("wrong_password".to_owned()),
                        }
                    ).await.unwrap_err();

                    match err {
                        Error::AuthorizationFailed(_) => {},
                        _ => panic!("expected AuthorizationFailed, got {:?}", err),
                    }
                }).await
            }

            async fn fetch_with_ssh_basic_no_master(repo, server, server_repo) {
                use crate::*;

                let auth = Authorization::Basic {
                    username: Some("my_username".to_owned()),
                    password: Some("my_password".to_owned()),
                };
                server.allow_authorization(auth.clone());

                server.run_with_server(async move |port| {
                    repo.create_remote("origin", &format!("[my_username@localhost:{port}]:test.git")).await.unwrap();

                    let err = repo.fetch(
                        "origin",
                        RefSpec{
                            source: Some("refs/heads/master".to_owned()),
                            destination: Some("refs/heads/master".to_owned()),
                            ..Default::default()
                        },
                        &auth
                    ).await.unwrap_err();

                    if let Error::RefNotFound(refname) = err {
                        assert_eq!(refname, "refs/heads/master");
                    } else {
                        panic!("expected RefNotFound, got {:?}", err);
                    }
                }).await
            }

            // DO NOT ADD NON-IO TESTS HERE. THIS IS THE WRONG SPOT.
        }
    };
}

#[allow(unused_imports)]
pub(crate) use gitbutler_git_integration_tests;
