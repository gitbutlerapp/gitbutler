use super::*;
use crate::{
    args::{Subcommands, agent, branch, config, update},
    bad_input,
};

use crate::args::atoms::CliIdArg;

fn prop<'a>(props: &'a [(String, serde_json::Value)], key: &str) -> Option<&'a serde_json::Value> {
    props
        .iter()
        .find_map(|(prop_key, value)| (prop_key.as_str() == key).then_some(value))
}

fn assert_command(subcommand: Subcommands, expected: &str) {
    assert_eq!(
        Event::new(EventKind::Cli(subcommand.to_metrics_command())).props["command"],
        serde_json::json!(expected)
    );
}

#[test]
fn status_events_include_one_percent_sampling_rate() {
    let props = sample_props(Props::new(), CommandName::Status, false, 0.005)
        .expect("draw below the rate should capture");

    assert_eq!(
        props.values["samplingRate"],
        serde_json::json!(0.01),
        "status events should carry their effective sampling rate"
    );
}

#[test]
fn cli_command_sampling_policy_matches_expected_rates() {
    let sampled_commands = CommandName::value_variants()
        .iter()
        .filter_map(|command| {
            let rate = command.sample_rate();
            (rate < 1.0).then(|| {
                (
                    command
                        .to_possible_value()
                        .expect("command names have clap values")
                        .get_name()
                        .to_owned(),
                    rate,
                )
            })
        })
        .collect::<Vec<_>>();

    assert_eq!(
        sampled_commands,
        vec![
            ("status".into(), 0.01),
            ("diff".into(), 0.05),
            ("branch-list".into(), 0.10),
            ("refresh-remote-data".into(), 0.05),
        ],
        "only the selected read-only commands should be sampled"
    );
}

#[test]
fn full_rate_cli_events_include_sampling_rate() {
    let props = sample_props(Props::new(), CommandName::Commit, false, 1.0)
        .expect("full-rate events should always capture");

    assert_eq!(
        props.values["samplingRate"],
        serde_json::json!(1.0),
        "full-rate events should record their rate"
    );
}

#[test]
fn command_rejections_bypass_sampling() {
    let result = Err::<(), _>(CliError::CommandRejection);
    let props =
        Props::from_cli_error_result(std::time::Instant::now(), &result, CommandName::Status);
    let props = sample_props(props, CommandName::Status, result.is_err(), 1.0)
        .expect("failed events should bypass sampling");

    assert_eq!(
        (&props.values["errorKind"], &props.values["samplingRate"]),
        (
            &serde_json::json!("commandRejection"),
            &serde_json::json!(1.0)
        ),
        "typed failures should use the full sampling rate"
    );
}

#[test]
fn successful_sampled_events_can_be_dropped() {
    assert!(
        sample_props(Props::new(), CommandName::Status, false, 0.5).is_none(),
        "successful status events above the rate should be dropped"
    );
}

#[test]
fn transport_rejects_malformed_or_invalid_rates() {
    for json in [
        "not-json",
        r#"{"samplingRate":"invalid"}"#,
        r#"{"samplingRate":0}"#,
        r#"{"samplingRate":1.1}"#,
    ] {
        assert!(
            prepare_transport_props(json).is_err(),
            "invalid transport properties must not produce an event: {json}"
        );
    }
}

#[test]
fn transport_keeps_a_valid_parent_rate_without_resampling() {
    let props = prepare_transport_props(r#"{"samplingRate":0.01}"#)
        .expect("a valid transport payload should parse");

    assert_eq!(
        props.values["samplingRate"],
        serde_json::json!(0.01),
        "the parent's effective rate must survive transport"
    );
}

#[test]
fn legacy_transport_events_without_a_rate_use_full_rate() {
    let props = prepare_transport_props("{}").expect("empty properties are valid JSON");

    assert_eq!(
        props.values["samplingRate"],
        serde_json::json!(1.0),
        "legacy events were emitted at the full rate"
    );
}

#[test]
fn transport_failures_without_a_rate_are_kept() {
    let props = prepare_transport_props(r#"{"error":"Internal error","errorKind":"internal"}"#)
        .expect("failure properties are valid JSON");

    assert_eq!(
        props.values["samplingRate"],
        serde_json::json!(1.0),
        "transport failures should use the full sampling rate"
    );
}

#[test]
fn commands_with_their_own_event_lifecycle_do_not_create_cli_context() {
    let mut settings = AppSettings::default();
    settings.telemetry.app_metrics_enabled = true;

    let commands = [
        Subcommands::Completions { shell: None },
        Subcommands::Mcp(crate::args::mcp::Platform {
            cmd: crate::args::mcp::Subcommands::Serve,
        }),
        Subcommands::Metrics {
            command_name: CommandName::Status,
            props: "{}".into(),
        },
    ];

    assert!(
        commands.iter().all(|command| command
            .to_metrics_context(&settings, Path::new("."))
            .is_none()),
        "commands that own or intentionally omit reporting should not get a CLI context"
    );
}

#[test]
fn workspace_shape_never_initializes_a_project() {
    but_testsupport::isolated_app_data_dir(|| {
        let tmp = tempfile::tempdir().expect("tempdir");
        let repo_dir = tmp.path().join("repo");
        gix::init(&repo_dir).expect("init plain repo");
        let mut event = Event::new(EventKind::Cli(CommandName::Commit));

        // A plain repository without GitButler state must stay untouched.
        add_workspace_shape(&mut event, &repo_dir);
        let data_dir = repo_dir.join(".git/gitbutler");
        assert!(!data_dir.exists());
        assert!(!event.props.contains_key("totalLanesInWorkspace"));

        // Even with a project data dir present, the project database must not be created.
        std::fs::create_dir(&data_dir).expect("create project data dir");
        add_workspace_shape(&mut event, &repo_dir);
        assert_eq!(
            std::fs::read_dir(&data_dir).expect("read data dir").count(),
            0
        );
        assert!(!event.props.contains_key("totalLanesInWorkspace"));
    });
}

#[test]
fn workspace_shape_counts_lanes_and_stacked_branches() {
    but_testsupport::isolated_app_data_dir(|| {
        let sandbox =
            but_testsupport::Sandbox::open_or_init_scenario_with_target_and_default_settings(
                "one-stack-three-dependent-branches",
            );
        let repo = sandbox.open_repo();
        let workdir = repo.workdir().expect("scenario is not bare").to_owned();
        // The shape is only read from repositories that already carry a project database.
        but_db::DbHandle::new_in_directory(repo.git_dir().join("gitbutler"))
            .expect("create project db");

        let mut event = Event::new(EventKind::Cli(CommandName::Commit));
        add_workspace_shape(&mut event, &workdir);

        assert_eq!(event.props["totalLanesInWorkspace"], serde_json::json!(1));
        assert_eq!(
            event.props["totalBranchesInWorkspace"],
            serde_json::json!(3)
        );
        assert_eq!(event.props["maxBranchesPerLane"], serde_json::json!(3));
    });
}

#[test]
fn metrics_use_invoked_command_names() {
    assert_command(
        Subcommands::Update(update::Platform {
            cmd: update::Subcommands::Check,
        }),
        "updateCheck",
    );
    assert_command(
        Subcommands::Update(update::Platform {
            cmd: update::Subcommands::Suppress { days: 7 },
        }),
        "updateSuppress",
    );
    assert_command(
        Subcommands::Agent(agent::Platform {
            cmd: Some(agent::Subcommands::Setup { print: false }),
        }),
        "agentSetup",
    );
    // Bare `but agent` (no subcommand) maps to the same metric.
    assert_command(
        Subcommands::Agent(agent::Platform { cmd: None }),
        "agentSetup",
    );
    assert_command(
        Subcommands::Branch(branch::Platform { cmd: None }),
        "branchList",
    );
    #[cfg(feature = "legacy")]
    assert_command(
        Subcommands::Move(crate::args::r#move::Platform {
            branch: Some(Some(CliIdArg("main".to_owned()))),
            above: None,
            below: None,
            unstack: false,
            sources: Vec::from([CliIdArg("ci".to_owned())]),
            allow_merged: Default::default(),
        }),
        "move",
    );

    #[cfg(all(unix, not(feature = "packaged-but-distribution")))]
    assert_command(
        Subcommands::Update(update::Platform {
            cmd: update::Subcommands::Install {
                target: Some("0.20.0".into()),
            },
        }),
        "updateInstall",
    );

    #[cfg(feature = "legacy")]
    {
        assert_command(
            Subcommands::Amend(crate::args::amend::Platform {
                target: CliIdArg("c1".into()),
                sources: vec![CliIdArg("a1".into())],
                allow_merged: Default::default(),
            }),
            "amend",
        );
    }
}

#[test]
fn formerly_unknown_commands_use_explicit_names() {
    assert_command(
        Subcommands::_Expand {
            cli_id: CliIdArg("c1".into()),
        },
        "expand",
    );
    assert_command(
        Subcommands::Config(config::Platform { cmd: None }),
        "config",
    );
    assert_command(
        Subcommands::Config(config::Platform {
            cmd: Some(config::Subcommands::User { cmd: None }),
        }),
        "config",
    );
    assert_command(
        Subcommands::Config(config::Platform {
            cmd: Some(config::Subcommands::Metrics { status: None }),
        }),
        "config",
    );

    #[cfg(feature = "legacy")]
    {
        assert_command(Subcommands::Setup { init: false }, "setup");
        assert_command(Subcommands::Teardown { checkout_to: None }, "teardown");
    }
}

#[test]
fn formerly_unclassified_commands_have_explicit_names() {
    let commands = [
        (
            Subcommands::_Comment(crate::args::comment::Platform {
                cmd: crate::args::comment::Subcommands::List {
                    wait: false,
                    timeout: 60,
                },
            }),
            "comment",
        ),
        (Subcommands::Completions { shell: None }, "completions"),
        (Subcommands::Help { topic: None }, "help"),
        (Subcommands::Onboarding, "onboarding"),
        (
            Subcommands::Mcp(crate::args::mcp::Platform {
                cmd: crate::args::mcp::Subcommands::Serve,
            }),
            "mcp",
        ),
        (
            Subcommands::Metrics {
                command_name: CommandName::Status,
                props: "{}".into(),
            },
            "metrics",
        ),
        (
            Subcommands::AgentLog {
                cmd: but_agentlog::Command::Sync,
            },
            "agentLog",
        ),
        #[cfg(feature = "legacy")]
        (
            Subcommands::Actions(crate::args::actions::Platform { cmd: None }),
            "actions",
        ),
    ];

    for (command, expected) in commands {
        assert_command(command, expected);
    }
}

#[test]
fn extra_props_keep_useful_source_and_target_kinds() {
    #[cfg(feature = "legacy")]
    {
        let moved = Subcommands::Move(crate::args::r#move::Platform {
            branch: Some(Some(CliIdArg("main".to_owned()))),
            above: None,
            below: None,
            unstack: false,
            sources: Vec::from([CliIdArg("ci".to_owned())]),
            allow_merged: Default::default(),
        });
        let props = moved.to_metrics_extra_props();
        assert_eq!(
            prop(&props, "sourceKind"),
            Some(&serde_json::json!("commitOrBranch"))
        );
        assert_eq!(
            prop(&props, "targetKind"),
            Some(&serde_json::json!("commitOrBranchOrUnassigned"))
        );
    }
}

#[test]
fn external_extra_props_include_sanitized_subcommand() {
    let props = Subcommands::External(vec![" typo-OK ".into()]).to_metrics_extra_props();
    assert_eq!(
        prop(&props, "externalSubcommand"),
        Some(&serde_json::json!("typo-OK"))
    );

    let props = Subcommands::External(vec!["/tmp/private".into()]).to_metrics_extra_props();
    assert_eq!(
        prop(&props, "externalSubcommand"),
        Some(&serde_json::json!(INVALID_UNRECOGNIZED_SUBCOMMAND))
    );

    let props = Subcommands::External(vec!["customer123".into()]).to_metrics_extra_props();
    assert_eq!(
        prop(&props, "externalSubcommand"),
        Some(&serde_json::json!(INVALID_UNRECOGNIZED_SUBCOMMAND))
    );
}

#[test]
fn internal_error_details_are_allowlisted() {
    let anyhow_result = Err::<(), _>(
        anyhow::anyhow!("stale id. If you just performed a Git operation, refresh")
            .context("Failed to uncommit."),
    );

    let props = Props::from_anyhow_result(
        std::time::Instant::now(),
        &anyhow_result,
        CommandName::Uncommit,
    );

    assert_eq!(props.values["error"], "Internal error");
    assert_eq!(props.values["errorKind"], "internal");
    assert_eq!(
        props.values["errorMessage"],
        "Failed to uncommit.: stale id."
    );
    assert_eq!(props.values["errorRoot"], "stale id.");

    let result =
        Err::<(), _>(anyhow::anyhow!("private-branch-name failed").context("private-path failed"));

    let props = Props::from_anyhow_result(std::time::Instant::now(), &result, CommandName::Status);

    assert_eq!(props.values["error"], "Internal error");
    assert_eq!(props.values["errorKind"], "internal");
    assert!(!props.values.contains_key("errorMessage"));
    assert!(!props.values.contains_key("errorRoot"));
    assert!(!props.as_json_string().contains("private-branch-name"));
    assert!(!props.as_json_string().contains("private-path"));
}

#[test]
fn commit_internal_error_details_are_captured() {
    let result = Err::<(), _>(CliError::Internal(
        anyhow::anyhow!("stale id. If you just performed a Git operation, refresh")
            .context("Failed to commit."),
    ));

    let props =
        Props::from_cli_error_result(std::time::Instant::now(), &result, CommandName::Commit);

    assert_eq!(props.values["errorMessage"], "Failed to commit.: stale id.");
    assert_eq!(props.values["errorRoot"], "stale id.");
}

#[test]
fn initialization_errors_use_distinct_kind_without_private_details() {
    let result = Err::<(), _>(CliError::Initialization(anyhow::anyhow!(
        "No git repository found at /Users/alice/secret-client"
    )));

    let props =
        Props::from_cli_error_result(std::time::Instant::now(), &result, CommandName::Commit);

    assert_eq!(props.values["errorKind"], "initialization");
    assert!(
        !props.as_json_string().contains("secret-client"),
        "initialization failures must stay low-cardinality"
    );
}

#[cfg(feature = "legacy")]
#[test]
fn explained_commit_rejections_omit_private_details() {
    let result = Err::<(), _>(CliError::Internal(anyhow::Error::new(
        crate::utils::rejection::ExplainedRejection(
            "Cannot commit private/path on private-branch".to_string(),
        ),
    )));

    let props =
        Props::from_cli_error_result(std::time::Instant::now(), &result, CommandName::Commit);

    assert_eq!(props.values["error"], "Command rejection");
    assert_eq!(props.values["errorKind"], "commandRejection");
    assert!(!props.values.contains_key("errorMessage"));
    assert!(!props.values.contains_key("errorRoot"));
    assert!(!props.as_json_string().contains("private/path"));
    assert!(!props.as_json_string().contains("private-branch"));
}

#[test]
fn cli_error_metrics_use_low_cardinality_failure_details() {
    let bad_input_result = Err::<(), _>(
        bad_input("Branch 'branch-with-private-name' not found")
            .arg_name("<BRANCH>")
            .arg_value("another-private-branch-name")
            .hint("Use a branch name")
            .into(),
    );

    let props = Props::from_cli_error_result(
        std::time::Instant::now(),
        &bad_input_result,
        CommandName::Commit,
    );

    assert_eq!(props.values["errorKind"], "badInput");
    assert_eq!(props.values["error"], "Bad input");
    assert!(!props.values.contains_key("errorMessage"));
    assert_eq!(props.values["badInputArgName"], "<BRANCH>");
    assert_eq!(props.values["badInputHasHint"], true);
    assert!(!props.as_json_string().contains("branch-with-private-name"));
    assert!(
        !props
            .as_json_string()
            .contains("another-private-branch-name")
    );

    let external_result = Err::<(), _>(CliError::ExternalCommandNotFound("typo".into()));
    let props = Props::from_cli_error_result(
        std::time::Instant::now(),
        &external_result,
        CommandName::External,
    );

    assert_eq!(props.values["error"], "Unrecognized subcommand");
    assert_eq!(props.values["errorKind"], "externalCommandNotFound");
    assert_eq!(props.values["unrecognizedSubcommand"], "typo");
    assert!(!props.values.contains_key("errorMessage"));

    let external_result = Err::<(), _>(CliError::ExternalCommandFailed(42));
    let props = Props::from_cli_error_result(
        std::time::Instant::now(),
        &external_result,
        CommandName::External,
    );
    assert_eq!(props.values["error"], "External command failed");
    assert_eq!(props.values["errorKind"], "externalCommandFailed");

    let external_result = Err::<(), _>(CliError::ExternalCommandNotFound(" typo-123_OK ".into()));
    let props = Props::from_cli_error_result(
        std::time::Instant::now(),
        &external_result,
        CommandName::External,
    );
    assert_eq!(props.values["unrecognizedSubcommand"], "typo-123_OK");

    let external_result = Err::<(), _>(CliError::ExternalCommandNotFound("/tmp/private".into()));
    let props = Props::from_cli_error_result(
        std::time::Instant::now(),
        &external_result,
        CommandName::External,
    );
    assert_eq!(
        props.values["unrecognizedSubcommand"],
        INVALID_UNRECOGNIZED_SUBCOMMAND
    );
    assert!(!props.as_json_string().contains("/tmp/private"));

    let long_command = "a".repeat(UNRECOGNIZED_SUBCOMMAND_MAX_CHARS + 1);
    let external_result = Err::<(), _>(CliError::ExternalCommandNotFound(long_command.into()));
    let props = Props::from_cli_error_result(
        std::time::Instant::now(),
        &external_result,
        CommandName::External,
    );
    assert_eq!(
        props.values["unrecognizedSubcommand"]
            .as_str()
            .expect("metric value is a string")
            .len(),
        UNRECOGNIZED_SUBCOMMAND_MAX_CHARS
    );
}

#[test]
fn detailed_error_messages_are_normalized_and_capped() {
    let multiline_result =
        Err::<(), _>(anyhow::anyhow!("first line\nsecond line").context("Failed to uncommit."));

    let props = Props::from_anyhow_result(
        std::time::Instant::now(),
        &multiline_result,
        CommandName::Uncommit,
    );

    assert_eq!(
        props.values["errorMessage"],
        "Failed to uncommit.: first line second line"
    );
    assert_eq!(props.values["errorRoot"], "first line second line");

    let long_result = Err::<(), _>(anyhow::anyhow!("{}", "a".repeat(1100)));

    let props = Props::from_anyhow_result(
        std::time::Instant::now(),
        &long_result,
        CommandName::Uncommit,
    );

    assert_eq!(
        props.values["errorMessage"].as_str().unwrap().len(),
        ERROR_MESSAGE_MAX_CHARS
    );
    assert_eq!(
        props.values["errorRoot"].as_str().unwrap().len(),
        ERROR_MESSAGE_MAX_CHARS
    );
}
