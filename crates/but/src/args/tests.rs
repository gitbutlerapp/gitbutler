use super::*;

mod commit;
mod reword;

mod config_ai {
    use clap::Parser;

    use crate::args::{
        Args, Subcommands,
        config::{AiKeyOption, AiSubcommand, Platform as ConfigPlatform, Subcommands as ConfigCmd},
    };

    #[test]
    fn interactive_defaults_to_global_scope() {
        let args = Args::try_parse_from(["but", "config", "ai"]).expect("parse args");
        let cmd = args.cmd.expect("subcommand");

        match cmd {
            Subcommands::Config(ConfigPlatform {
                cmd: Some(ConfigCmd::Ai { local, global, cmd }),
            }) => {
                assert!(!local);
                assert!(!global);
                assert!(cmd.is_none());
            }
            _ => panic!("unexpected command shape"),
        }
    }

    #[test]
    fn openai_non_interactive_parses_provider_flags() {
        let args = Args::try_parse_from([
            "but",
            "config",
            "ai",
            "openai",
            "--key-option",
            "bring-your-own",
            "--model",
            "gpt-5.4-nano",
            "--endpoint",
            "https://api.openai.com/v1",
            "--api-key-env",
            "OPENAI_API_KEY",
        ])
        .expect("parse args");

        let cmd = args.cmd.expect("subcommand");
        match cmd {
            Subcommands::Config(ConfigPlatform {
                cmd:
                    Some(ConfigCmd::Ai {
                        cmd:
                            Some(AiSubcommand::Openai {
                                key_option,
                                model,
                                endpoint,
                                api_key,
                                api_key_env,
                            }),
                        ..
                    }),
            }) => {
                assert!(matches!(key_option, Some(AiKeyOption::BringYourOwn)));
                assert_eq!(model.as_deref(), Some("gpt-5.4-nano"));
                assert_eq!(endpoint.as_deref(), Some("https://api.openai.com/v1"));
                assert!(api_key.is_none());
                assert_eq!(api_key_env.as_deref(), Some("OPENAI_API_KEY"));
            }
            _ => panic!("unexpected command shape"),
        }
    }

    #[test]
    fn local_scope_flag_parses_before_provider() {
        let args = Args::try_parse_from([
            "but",
            "config",
            "ai",
            "--local",
            "ollama",
            "--endpoint",
            "localhost:11434",
            "--model",
            "llama3.1",
        ])
        .expect("parse args");

        let cmd = args.cmd.expect("subcommand");
        match cmd {
            Subcommands::Config(ConfigPlatform {
                cmd:
                    Some(ConfigCmd::Ai {
                        local,
                        global,
                        cmd: Some(AiSubcommand::Ollama { endpoint, model }),
                    }),
            }) => {
                assert!(local);
                assert!(!global);
                assert_eq!(endpoint.as_deref(), Some("localhost:11434"));
                assert_eq!(model.as_deref(), Some("llama3.1"));
            }
            _ => panic!("unexpected command shape"),
        }
    }

    #[test]
    fn show_subcommand_parses() {
        let args = Args::try_parse_from(["but", "config", "ai", "show"]).expect("parse args");

        let cmd = args.cmd.expect("subcommand");
        match cmd {
            Subcommands::Config(ConfigPlatform {
                cmd:
                    Some(ConfigCmd::Ai {
                        local,
                        global,
                        cmd: Some(AiSubcommand::Show),
                    }),
            }) => {
                assert!(!local);
                assert!(!global);
            }
            _ => panic!("unexpected command shape"),
        }
    }
}

#[test]
fn clap() {
    use clap::CommandFactory;
    Args::command().debug_assert();
}

mod push {
    #[cfg(feature = "legacy")]
    mod get_gerrit_flags {
        use crate::{args::push::Command as Args, command::legacy::push::get_gerrit_flags};

        #[test]
        fn non_gerrit_mode() {
            let args = Args {
                branch_id: Some("test".to_string()),
                with_force: true,
                skip_force_push_protection: false,
                run_hooks: true,
                wip: false,
                ready: false,
                hashtag: vec![],
                topic: None,
                topic_from_branch: false,
                private: false,
                dry_run: false,
            };

            let result = get_gerrit_flags(&args, "test-branch", false);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), vec![]);
        }

        #[test]
        fn error_when_flags_without_gerrit_mode() {
            let args = Args {
                branch_id: Some("test".to_string()),
                with_force: true,
                skip_force_push_protection: false,
                run_hooks: true,
                wip: true,
                ready: false,
                hashtag: vec![],
                topic: None,
                topic_from_branch: false,
                private: false,
                dry_run: false,
            };

            let result = get_gerrit_flags(&args, "test-branch", false);
            assert!(result.is_err());
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("can only be used when gerrit_mode is enabled")
            );
        }

        #[test]
        fn default_ready() {
            let args = Args {
                branch_id: Some("test".to_string()),
                with_force: true,
                skip_force_push_protection: false,
                run_hooks: true,
                wip: false,
                ready: false,
                hashtag: vec![],
                topic: None,
                topic_from_branch: false,
                private: false,
                dry_run: false,
            };

            let result = get_gerrit_flags(&args, "test-branch", true);
            assert!(result.is_ok());
            let flags = result.unwrap();
            assert_eq!(flags.len(), 1);
            assert!(matches!(flags[0], but_gerrit::PushFlag::Ready));
        }

        #[test]
        fn wip() {
            let args = Args {
                branch_id: Some("test".to_string()),
                with_force: true,
                skip_force_push_protection: false,
                run_hooks: true,
                wip: true,
                ready: false,
                hashtag: vec![],
                topic: None,
                topic_from_branch: false,
                private: false,
                dry_run: false,
            };

            let result = get_gerrit_flags(&args, "test-branch", true);
            assert!(result.is_ok());
            let flags = result.unwrap();
            assert_eq!(flags.len(), 1);
            assert!(matches!(flags[0], but_gerrit::PushFlag::Wip));
        }

        #[test]
        fn multiple_hashtags() {
            let args = Args {
                branch_id: Some("test".to_string()),
                with_force: true,
                skip_force_push_protection: false,
                run_hooks: true,
                wip: false,
                ready: false,
                hashtag: vec!["tag1".to_string(), "tag2".to_string(), "tag3".to_string()],
                topic: None,
                topic_from_branch: false,
                private: false,
                dry_run: false,
            };

            let result = get_gerrit_flags(&args, "test-branch", true);
            assert!(result.is_ok());
            let flags = result.unwrap();
            assert_eq!(flags.len(), 4); // Ready + 3 hashtags

            let ready_count = flags
                .iter()
                .filter(|f| matches!(f, but_gerrit::PushFlag::Ready))
                .count();
            assert_eq!(ready_count, 1);

            let hashtag_count = flags
                .iter()
                .filter(|f| matches!(f, but_gerrit::PushFlag::Hashtag(_)))
                .count();
            assert_eq!(hashtag_count, 3);
        }

        #[test]
        fn topic_from_custom() {
            let args = Args {
                branch_id: Some("test".to_string()),
                with_force: true,
                skip_force_push_protection: false,
                run_hooks: true,
                wip: false,
                ready: false,
                hashtag: vec![],
                topic: Some("custom-topic".to_string()),
                topic_from_branch: false,
                private: false,
                dry_run: false,
            };

            let result = get_gerrit_flags(&args, "test-branch", true);
            assert!(result.is_ok());
            let flags = result.unwrap();
            assert_eq!(flags.len(), 2); // Ready + Topic

            let topic_flags: Vec<_> = flags
                .iter()
                .filter_map(|f| match f {
                    but_gerrit::PushFlag::Topic(t) => Some(t.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(topic_flags, vec!["custom-topic"]);
        }

        #[test]
        fn topic_from_branch() {
            let args = Args {
                branch_id: Some("test".to_string()),
                with_force: true,
                skip_force_push_protection: false,
                run_hooks: true,
                wip: false,
                ready: false,
                hashtag: vec![],
                topic: None,
                topic_from_branch: true,
                private: false,
                dry_run: false,
            };

            let result = get_gerrit_flags(&args, "my-branch-name", true);
            assert!(result.is_ok());
            let flags = result.unwrap();
            assert_eq!(flags.len(), 2); // Ready + Topic

            let topic_flags: Vec<_> = flags
                .iter()
                .filter_map(|f| match f {
                    but_gerrit::PushFlag::Topic(t) => Some(t.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(topic_flags, vec!["my-branch-name"]);
        }

        #[test]
        fn private() {
            let args = Args {
                branch_id: Some("test".to_string()),
                with_force: true,
                skip_force_push_protection: false,
                run_hooks: true,
                wip: false,
                ready: false,
                hashtag: vec![],
                topic: None,
                topic_from_branch: false,
                private: true,
                dry_run: false,
            };

            let result = get_gerrit_flags(&args, "test-branch", true);
            assert!(result.is_ok());
            let flags = result.unwrap();
            assert_eq!(flags.len(), 2); // Ready + Private

            let private_count = flags
                .iter()
                .filter(|f| matches!(f, but_gerrit::PushFlag::Private))
                .count();
            assert_eq!(private_count, 1);
        }

        #[test]
        fn all_combined() {
            let args = Args {
                branch_id: Some("test".to_string()),
                with_force: true,
                skip_force_push_protection: false,
                run_hooks: true,
                wip: true,
                ready: false,
                hashtag: vec!["tag1".to_string(), "tag2".to_string()],
                topic: Some("custom-topic".to_string()),
                topic_from_branch: false,
                private: true,
                dry_run: false,
            };

            let result = get_gerrit_flags(&args, "test-branch", true);
            assert!(result.is_ok());
            let flags = result.unwrap();
            assert_eq!(flags.len(), 5); // Wip + 2 hashtags + Topic + Private

            let wip_count = flags
                .iter()
                .filter(|f| matches!(f, but_gerrit::PushFlag::Wip))
                .count();
            assert_eq!(wip_count, 1);

            let hashtag_count = flags
                .iter()
                .filter(|f| matches!(f, but_gerrit::PushFlag::Hashtag(_)))
                .count();
            assert_eq!(hashtag_count, 2);

            let topic_count = flags
                .iter()
                .filter(|f| matches!(f, but_gerrit::PushFlag::Topic(_)))
                .count();
            assert_eq!(topic_count, 1);

            let private_count = flags
                .iter()
                .filter(|f| matches!(f, but_gerrit::PushFlag::Private))
                .count();
            assert_eq!(private_count, 1);
        }

        #[test]
        fn empty_hashtag_error() {
            let args = Args {
                branch_id: Some("test".to_string()),
                with_force: true,
                skip_force_push_protection: false,
                run_hooks: true,
                wip: false,
                ready: false,
                hashtag: vec!["  ".to_string()],
                topic: None,
                topic_from_branch: false,
                private: false,
                dry_run: false,
            };

            let result = get_gerrit_flags(&args, "test-branch", true);
            assert!(result.is_err());
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("Hashtag cannot be empty")
            );
        }

        #[test]
        fn empty_topic_error() {
            let args = Args {
                branch_id: Some("test".to_string()),
                with_force: true,
                skip_force_push_protection: false,
                run_hooks: true,
                wip: false,
                ready: false,
                hashtag: vec![],
                topic: Some("  ".to_string()),
                topic_from_branch: false,
                private: false,
                dry_run: false,
            };

            let result = get_gerrit_flags(&args, "test-branch", true);
            assert!(result.is_err());
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("Topic cannot be empty")
            );
        }
    }
}
