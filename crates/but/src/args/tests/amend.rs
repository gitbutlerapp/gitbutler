use clap::Parser as _;

use crate::args::{Args, Subcommands, amend::Platform};

fn parse_amend(args: &[&str]) -> Platform {
    let args = std::iter::once("but").chain(args.iter().copied());
    let args = Args::try_parse_from(args).expect("valid amend arguments");
    let cmd = args.cmd.expect("an amend command");

    let Subcommands::Amend(platform) = cmd else {
        panic!("expected amend command, got {cmd:?}");
    };

    platform
}

#[test]
fn parses_sources_followed_by_long_target() {
    let Platform {
        target,
        sources,
        allow_merged: _,
    } = parse_amend(&["amend", "a1", "b2", "c3", "--target", "d4"]);

    assert_eq!(target.0, "d4");
    assert_eq!(
        sources
            .into_iter()
            .map(|source| source.0)
            .collect::<Vec<_>>(),
        ["a1", "b2", "c3"]
    );
}

#[test]
fn parses_sources_followed_by_short_target() {
    let Platform {
        target,
        sources,
        allow_merged: _,
    } = parse_amend(&["amend", "a1", "b2", "-t", "d4"]);

    assert_eq!(target.0, "d4");
    assert_eq!(
        sources
            .into_iter()
            .map(|source| source.0)
            .collect::<Vec<_>>(),
        ["a1", "b2"]
    );
}

#[test]
fn allows_omitted_sources() {
    let Platform {
        target,
        sources,
        allow_merged: _,
    } = parse_amend(&["amend", "--target", "d4"]);

    assert_eq!(target.0, "d4");
    assert!(sources.is_empty(), "omitted sources should parse as empty");
}

#[test]
fn requires_target() {
    let error = Args::try_parse_from(["but", "amend", "a1"]).expect_err("amend requires a target");

    assert_eq!(
        error.kind(),
        clap::error::ErrorKind::MissingRequiredArgument
    );
}
