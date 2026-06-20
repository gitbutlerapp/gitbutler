use clap::Parser as _;

use crate::args::{Args, Subcommands};

fn parse_amend(args: &[&str]) -> (String, Option<String>, Vec<String>) {
    let args = std::iter::once("but").chain(args.iter().copied());
    let args = Args::try_parse_from(args).unwrap();
    let cmd = args.cmd.unwrap();

    let Subcommands::Amend {
        target_or_source,
        legacy_commit,
        changes,
    } = cmd
    else {
        panic!("expected amend command. Got {cmd:?}");
    };

    (target_or_source, legacy_commit, changes)
}

#[test]
fn changes_flag_accepts_comma_separated_sources() {
    let (target_or_source, legacy_commit, changes) =
        parse_amend(&["amend", "c3", "--changes", "a1,b2"]);

    assert_eq!(target_or_source, "c3");
    assert_eq!(legacy_commit, None);
    assert_eq!(changes, ["a1", "b2"]);
}

#[test]
fn changes_flag_accepts_repeated_sources() {
    let (target_or_source, legacy_commit, changes) =
        parse_amend(&["amend", "c3", "--changes", "a1", "--changes", "b2"]);

    assert_eq!(target_or_source, "c3");
    assert_eq!(legacy_commit, None);
    assert_eq!(changes, ["a1", "b2"]);
}

#[test]
fn legacy_single_source_form_still_parses() {
    let (target_or_source, legacy_commit, changes) = parse_amend(&["amend", "a1", "c3"]);

    assert_eq!(target_or_source, "a1");
    assert_eq!(legacy_commit.as_deref(), Some("c3"));
    assert!(changes.is_empty());
}

#[test]
fn invalid_extra_positionals_are_rejected() {
    assert!(Args::try_parse_from(["but", "amend", "a1", "b2", "c3"]).is_err());
}
