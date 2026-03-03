use std::time::Duration;

use clap::{CommandFactory, Parser, error::ErrorKind};

#[allow(dead_code)]
#[path = "../src/cli.rs"]
mod cli_impl;

use cli_impl::{
    BlockMode, CheckFormat, Cmd, DiscoveryFormat, Platform, ReadView, normalize_claim_path,
    parse_duration,
};

#[test]
fn normalize_claim_path_strips_dot_slash() -> anyhow::Result<()> {
    assert_eq!(normalize_claim_path("./src/foo.rs")?, "src/foo.rs");
    Ok(())
}

#[test]
fn normalize_claim_path_strips_trailing_slash() -> anyhow::Result<()> {
    assert_eq!(normalize_claim_path("src/")?, "src");
    Ok(())
}

#[test]
fn normalize_claim_path_collapses_dotdot() -> anyhow::Result<()> {
    assert_eq!(normalize_claim_path("src/lib/../foo.rs")?, "src/foo.rs");
    Ok(())
}

#[test]
fn normalize_claim_path_identity() -> anyhow::Result<()> {
    assert_eq!(normalize_claim_path("src/foo.rs")?, "src/foo.rs");
    Ok(())
}

#[test]
fn normalize_claim_path_rejects_absolute_paths() {
    assert!(normalize_claim_path("/tmp/foo.rs").is_err());
}

#[test]
fn normalize_claim_path_rejects_paths_outside_repo() {
    assert!(normalize_claim_path("../foo.rs").is_err());
}

#[test]
fn parse_acquire_multiple_paths() {
    let plat = Platform::parse_from([
        "but-link",
        "acquire",
        "--path",
        "src/a.rs",
        "--path",
        "src/b.rs",
        "--ttl",
        "15m",
        "--strict",
        "--dry-run",
        "--format",
        "compact",
        "--agent-id",
        "a1",
    ]);
    let (_, cmd) = plat.into_runtime().unwrap();
    match cmd {
        Cmd::Acquire {
            paths,
            ttl,
            strict,
            dry_run,
            format,
        } => {
            assert_eq!(paths, vec!["src/a.rs", "src/b.rs"]);
            assert_eq!(ttl, Duration::from_secs(900));
            assert!(strict);
            assert!(dry_run);
            assert_eq!(format, CheckFormat::Compact);
        }
        _ => panic!("expected Acquire"),
    }
}

#[test]
fn removed_claims_subcommand_fails_to_parse() {
    let err = Platform::try_parse_from(["but-link", "claims"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidSubcommand);
}

#[test]
fn parse_read_defaults_to_inbox() {
    let plat = Platform::parse_from(["but-link", "read", "--agent-id", "a1"]);
    let (_, cmd) = plat.into_runtime().unwrap();
    match cmd {
        Cmd::Read {
            view,
            format,
            since,
        } => {
            assert_eq!(view, ReadView::Inbox);
            assert_eq!(format, DiscoveryFormat::Full);
            assert!(since.is_none());
        }
        _ => panic!("expected Read"),
    }
}

#[test]
fn removed_read_type_flag_fails_to_parse() {
    let err = Platform::try_parse_from(["but-link", "read", "--type", "all", "--agent-id", "a1"])
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);
}

#[test]
fn parse_read_discovery_digest_format() {
    let plat = Platform::parse_from([
        "but-link",
        "read",
        "--view",
        "discoveries",
        "--format",
        "digest",
        "--agent-id",
        "a1",
    ]);
    let (_, cmd) = plat.into_runtime().unwrap();
    match cmd {
        Cmd::Read { view, format, .. } => {
            assert_eq!(view, ReadView::Discoveries);
            assert_eq!(format, DiscoveryFormat::Digest);
        }
        _ => panic!("expected Read"),
    }
}

#[test]
fn parse_read_claims_defaults_to_observer_agent() {
    let plat = Platform::parse_from(["but-link", "read", "--view", "claims"]);
    let (agent_id, cmd) = plat.into_runtime().unwrap();
    assert_eq!(agent_id, cli_impl::OBSERVER_AGENT_ID);
    match cmd {
        Cmd::Read { view, .. } => assert_eq!(view, ReadView::Claims),
        _ => panic!("expected Read"),
    }
}

#[test]
fn parse_acquire_requires_path_flags() {
    let err = Platform::try_parse_from(["but-link", "acquire", "src/foo.rs", "--agent-id", "a1"])
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);
}

#[test]
fn parse_block_subcommand() {
    let plat = Platform::parse_from([
        "but-link",
        "block",
        "--path",
        "src/foo.rs",
        "--reason",
        "shared refactor",
        "--mode",
        "hard",
        "--ttl",
        "10m",
        "--agent-id",
        "a1",
    ]);
    let (_, cmd) = plat.into_runtime().unwrap();
    match cmd {
        Cmd::Block {
            paths,
            reason,
            mode,
            ttl,
        } => {
            assert_eq!(paths, vec!["src/foo.rs"]);
            assert_eq!(reason, "shared refactor");
            assert_eq!(mode, BlockMode::Hard);
            assert_eq!(ttl, Some(Duration::from_secs(600)));
        }
        _ => panic!("expected Block"),
    }
}

#[test]
fn parse_ack_subcommand() {
    let plat = Platform::parse_from([
        "but-link",
        "ack",
        "--agent",
        "peer-a",
        "--path",
        "src/foo.rs",
        "--note",
        "saw it",
        "--agent-id",
        "a1",
    ]);
    let (_, cmd) = plat.into_runtime().unwrap();
    match cmd {
        Cmd::Ack {
            target_agent_id,
            paths,
            note,
        } => {
            assert_eq!(target_agent_id, "peer-a");
            assert_eq!(paths, vec!["src/foo.rs"]);
            assert_eq!(note.as_deref(), Some("saw it"));
        }
        _ => panic!("expected Ack"),
    }
}

#[test]
fn parse_intent_subcommand_with_paths() {
    let plat = Platform::parse_from([
        "but-link",
        "intent",
        "crate::auth",
        "--tag",
        "api",
        "--surface",
        "AuthToken",
        "--path",
        "src/auth.rs",
        "--agent-id",
        "B",
    ]);
    let (_, cmd) = plat.into_runtime().unwrap();
    match cmd {
        Cmd::Intent {
            scope,
            tags,
            surface,
            paths,
        } => {
            assert_eq!(scope, "crate::auth");
            assert_eq!(tags, vec!["api"]);
            assert_eq!(surface, vec!["AuthToken"]);
            assert_eq!(paths, vec!["src/auth.rs"]);
        }
        _ => panic!("expected Intent"),
    }
}

#[test]
fn parse_tui_without_agent_id() {
    let plat = Platform::parse_from(["but-link", "tui"]);
    let (agent_id, cmd) = plat.into_runtime().unwrap();
    assert_eq!(agent_id, cli_impl::OBSERVER_AGENT_ID);
    assert!(matches!(cmd, Cmd::Tui));
}

#[test]
fn removed_agents_subcommand_fails_to_parse() {
    let err = Platform::try_parse_from(["but-link", "agents"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidSubcommand);
}

#[test]
fn removed_release_subcommand_fails_to_parse() {
    let err = Platform::try_parse_from([
        "but-link",
        "release",
        "--path",
        "src/foo.rs",
        "--agent-id",
        "a1",
    ])
    .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidSubcommand);
}

#[test]
fn help_hides_internal_eval_subcommand() {
    let mut cmd = Platform::command();
    let help = cmd.render_long_help().to_string();

    assert!(help.contains("acquire"));
    assert!(!help.contains("\n  release\n"));
    assert!(!help.contains("\n  claims\n"));
    assert!(!help.contains("\n  agents\n"));
    assert!(!help.contains("\n  claim\n"));
    assert!(!help.contains("\n  check\n"));
    assert!(!help.contains("eval"));
}

#[test]
fn parse_duration_variants() {
    assert_eq!(parse_duration("15m").unwrap(), Duration::from_secs(900));
    assert_eq!(parse_duration("1h").unwrap(), Duration::from_secs(3600));
    assert_eq!(parse_duration("30s").unwrap(), Duration::from_secs(30));
    assert_eq!(parse_duration("2d").unwrap(), Duration::from_secs(172_800));
    assert_eq!(parse_duration("60").unwrap(), Duration::from_secs(60));
}

#[test]
fn parse_help_long_flag() {
    let err = Platform::try_parse_from(["but-link", "--help"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::DisplayHelp);
}

#[test]
fn parse_help_short_h_flag() {
    let err = Platform::try_parse_from(["but-link", "-H"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::DisplayHelp);
}
