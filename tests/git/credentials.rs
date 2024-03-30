use gitbutler::git::credentials::{Credential, Helper, HttpsCredential, SshCredential};
use gitbutler::{keys, project_repository, projects, users};
use std::path::PathBuf;

use crate::shared::{temp_dir, test_repository};

#[derive(Default)]
struct TestCase<'a> {
    remote_url: &'a str,
    github_access_token: Option<&'a str>,
    preferred_key: projects::AuthKey,
    home_dir: Option<PathBuf>,
}

impl TestCase<'_> {
    fn run(&self) -> Vec<(String, Vec<Credential>)> {
        let local_app_data = temp_dir();

        let users = users::Controller::from_path(&local_app_data);
        let user = users::User {
            github_access_token: self.github_access_token.map(ToString::to_string),
            ..Default::default()
        };
        users.set_user(&user).unwrap();

        let keys = keys::Controller::from_path(&local_app_data);
        let helper = Helper::new(keys, users, self.home_dir.clone());

        let (repo, _tmp) = test_repository();
        repo.remote(
            "origin",
            &self.remote_url.parse().expect("failed to parse remote url"),
        )
        .unwrap();
        let project = projects::Project {
            path: repo.workdir().unwrap().to_path_buf(),
            preferred_key: self.preferred_key.clone(),
            ..Default::default()
        };
        let project_repository = project_repository::Repository::open(&project).unwrap();

        let flow = helper.help(&project_repository, "origin").unwrap();
        flow.into_iter()
            .map(|(remote, credentials)| {
                (
                    remote.url().unwrap().as_ref().unwrap().to_string(),
                    credentials,
                )
            })
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
                github_access_token: Some("token"),
                preferred_key: projects::AuthKey::Local {
                    private_key_path: PathBuf::from("/tmp/id_rsa"),
                },
                ..Default::default()
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
                github_access_token: Some("token"),
                preferred_key: projects::AuthKey::Local {
                    private_key_path: PathBuf::from("/tmp/id_rsa"),
                },
                ..Default::default()
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

    mod with_github_token {
        use super::*;

        #[test]
        fn https() {
            let test_case = TestCase {
                remote_url: "https://gitlab.com/test-gitbutler/test.git",
                github_access_token: Some("token"),
                ..Default::default()
            };
            let flow = test_case.run();

            assert_eq!(flow.len(), 1);

            assert_eq!(
                flow[0].0,
                "git@gitlab.com:test-gitbutler/test.git".to_string(),
            );
            assert_eq!(flow[0].1.len(), 1);
            assert!(matches!(
                flow[0].1[0],
                Credential::Ssh(SshCredential::GitButlerKey(_))
            ));
        }

        #[test]
        fn ssh() {
            let test_case = TestCase {
                remote_url: "git@gitlab.com:test-gitbutler/test.git",
                github_access_token: Some("token"),
                ..Default::default()
            };
            let flow = test_case.run();

            assert_eq!(flow.len(), 1);

            assert_eq!(
                flow[0].0,
                "git@gitlab.com:test-gitbutler/test.git".to_string(),
            );
            assert_eq!(flow[0].1.len(), 1);
            assert!(matches!(
                flow[0].1[0],
                Credential::Ssh(SshCredential::GitButlerKey(_))
            ));
        }
    }
}

mod github {
    use super::*;

    mod with_github_token {
        use super::*;

        #[test]
        fn https() {
            let test_case = TestCase {
                remote_url: "https://github.com/gitbutlerapp/gitbutler.git",
                github_access_token: Some("token"),
                ..Default::default()
            };
            let flow = test_case.run();
            assert_eq!(flow.len(), 1);
            assert_eq!(
                flow[0].0,
                "https://github.com/gitbutlerapp/gitbutler.git".to_string(),
            );
            assert_eq!(
                flow[0].1,
                vec![Credential::Https(HttpsCredential::GitHubToken(
                    "token".to_string()
                ))]
            );
        }

        #[test]
        fn ssh() {
            let test_case = TestCase {
                remote_url: "git@github.com:gitbutlerapp/gitbutler.git",
                github_access_token: Some("token"),
                ..Default::default()
            };
            let flow = test_case.run();
            assert_eq!(flow.len(), 1);
            assert_eq!(
                flow[0].0,
                "https://github.com/gitbutlerapp/gitbutler.git".to_string(),
            );
            assert_eq!(
                flow[0].1,
                vec![Credential::Https(HttpsCredential::GitHubToken(
                    "token".to_string()
                ))]
            );
        }
    }

    mod without_github_token {
        use super::*;

        mod without_preferred_key {
            use super::*;

            #[test]
            fn https() {
                let test_case = TestCase {
                    remote_url: "https://github.com/gitbutlerapp/gitbutler.git",
                    ..Default::default()
                };
                let flow = test_case.run();

                assert_eq!(flow.len(), 1);

                assert_eq!(
                    flow[0].0,
                    "git@github.com:gitbutlerapp/gitbutler.git".to_string(),
                );
                assert_eq!(flow[0].1.len(), 1);
                assert!(matches!(
                    flow[0].1[0],
                    Credential::Ssh(SshCredential::GitButlerKey(_))
                ));
            }

            #[test]
            fn ssh() {
                let test_case = TestCase {
                    remote_url: "git@github.com:gitbutlerapp/gitbutler.git",
                    ..Default::default()
                };
                let flow = test_case.run();

                assert_eq!(flow.len(), 1);

                assert_eq!(
                    flow[0].0,
                    "git@github.com:gitbutlerapp/gitbutler.git".to_string(),
                );
                assert_eq!(flow[0].1.len(), 1);
                assert!(matches!(
                    flow[0].1[0],
                    Credential::Ssh(SshCredential::GitButlerKey(_))
                ));
            }
        }

        mod with_preferred_key {
            use super::*;

            #[test]
            fn https() {
                let test_case = TestCase {
                    remote_url: "https://github.com/gitbutlerapp/gitbutler.git",
                    github_access_token: Some("token"),
                    preferred_key: projects::AuthKey::Local {
                        private_key_path: PathBuf::from("/tmp/id_rsa"),
                    },
                    ..Default::default()
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
                    github_access_token: Some("token"),
                    preferred_key: projects::AuthKey::Local {
                        private_key_path: PathBuf::from("/tmp/id_rsa"),
                    },
                    ..Default::default()
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
