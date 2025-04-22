use std::{path::PathBuf, str};

use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_project as projects;
use gitbutler_repo::credentials::{help, Credential, SshCredential};
use gitbutler_testsupport::{temp_dir, test_repository};
use gitbutler_user as users;

#[derive(Default)]
struct TestCase<'a> {
    remote_url: &'a str,
    with_github_login: bool,
    preferred_key: projects::AuthKey,
}

impl TestCase<'_> {
    fn run(&self) -> Vec<(String, Vec<Credential>)> {
        let local_app_data = temp_dir();

        gitbutler_testsupport::secrets::setup_blackhole_store();
        let users = users::Controller::from_path(local_app_data.path());
        let user: users::User = serde_json::from_str(if self.with_github_login {
            include_str!("../tests/fixtures/users/with-github.v1")
        } else {
            include_str!("../tests/fixtures/users/login-only.v1")
        })
        .expect("valid v1 sample user");
        users.set_user(&user).unwrap();

        let (repo, _tmp) = test_repository();
        repo.remote("origin", self.remote_url).unwrap();
        let project = projects::Project {
            path: repo.workdir().unwrap().to_path_buf(),
            preferred_key: self.preferred_key.clone(),
            ..Default::default()
        };
        let ctx = CommandContext::open(&project, AppSettings::default()).unwrap();

        let flow = help(&ctx, "origin").unwrap();
        flow.into_iter()
            .map(|(remote, credentials)| (remote.url().as_ref().unwrap().to_string(), credentials))
            .collect::<Vec<_>>()
    }
}

mod not_github {
    use super::*;

    mod with_preferred_key {
        use super::*;

        #[test]
        fn https() {
            let test_case = TestCase {
                remote_url: "https://gitlab.com/test-gitbutler/test.git",
                with_github_login: true,
                preferred_key: projects::AuthKey::Local {
                    private_key_path: PathBuf::from("/tmp/id_rsa"),
                },
            };
            let flow = test_case.run();
            assert_eq!(flow.len(), 1);
            assert_eq!(
                flow[0].0,
                "git@gitlab.com:test-gitbutler/test.git".to_string(),
            );
            assert_eq!(
                flow[0].1,
                vec![Credential::Ssh(SshCredential::Keyfile {
                    key_path: PathBuf::from("/tmp/id_rsa"),
                    passphrase: None,
                })]
            );
        }

        #[test]
        fn ssh() {
            let test_case = TestCase {
                remote_url: "git@gitlab.com:test-gitbutler/test.git",
                with_github_login: true,
                preferred_key: projects::AuthKey::Local {
                    private_key_path: PathBuf::from("/tmp/id_rsa"),
                },
            };
            let flow = test_case.run();
            assert_eq!(flow.len(), 1);
            assert_eq!(
                flow[0].0,
                "git@gitlab.com:test-gitbutler/test.git".to_string(),
            );
            assert_eq!(
                flow[0].1,
                vec![Credential::Ssh(SshCredential::Keyfile {
                    key_path: PathBuf::from("/tmp/id_rsa"),
                    passphrase: None,
                })]
            );
        }
    }
}

mod github {
    use super::*;

    mod without_github_token {
        use super::*;

        mod with_preferred_key {
            use super::*;

            #[test]
            fn https() {
                let test_case = TestCase {
                    remote_url: "https://github.com/gitbutlerapp/gitbutler.git",
                    with_github_login: true,
                    preferred_key: projects::AuthKey::Local {
                        private_key_path: PathBuf::from("/tmp/id_rsa"),
                    },
                };
                let flow = test_case.run();
                assert_eq!(flow.len(), 1);
                assert_eq!(
                    flow[0].0,
                    "git@github.com:gitbutlerapp/gitbutler.git".to_string(),
                );
                assert_eq!(
                    flow[0].1,
                    vec![Credential::Ssh(SshCredential::Keyfile {
                        key_path: PathBuf::from("/tmp/id_rsa"),
                        passphrase: None,
                    })]
                );
            }

            #[test]
            fn ssh() {
                let test_case = TestCase {
                    remote_url: "git@github.com:gitbutlerapp/gitbutler.git",
                    with_github_login: true,
                    preferred_key: projects::AuthKey::Local {
                        private_key_path: PathBuf::from("/tmp/id_rsa"),
                    },
                };
                let flow = test_case.run();
                assert_eq!(flow.len(), 1);
                assert_eq!(
                    flow[0].0,
                    "git@github.com:gitbutlerapp/gitbutler.git".to_string(),
                );
                assert_eq!(
                    flow[0].1,
                    vec![Credential::Ssh(SshCredential::Keyfile {
                        key_path: PathBuf::from("/tmp/id_rsa"),
                        passphrase: None,
                    })]
                );
            }
        }
    }
}
