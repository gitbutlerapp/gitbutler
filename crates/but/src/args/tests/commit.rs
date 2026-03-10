use clap::Parser as _;

use crate::args::{Args, Subcommands};

#[test]
fn basic() {
    let args = Args::try_parse_from(["but", "commit"]).unwrap();
    let cmd = args.cmd.unwrap();

    let Subcommands::Commit(commit_args) = cmd else {
        panic!("expected commit command. Got {cmd:?}");
    };

    assert!(!commit_args.diff);
    assert!(!commit_args.no_diff);
}

#[test]
fn always_show_diff() {
    let args = Args::try_parse_from(["but", "commit", "--diff"]).unwrap();
    let cmd = args.cmd.unwrap();

    let Subcommands::Commit(commit_args) = cmd else {
        panic!("expected commit command. Got {cmd:?}");
    };

    assert!(commit_args.diff);
    assert!(!commit_args.no_diff);
}

#[test]
fn never_show_diff() {
    let args = Args::try_parse_from(["but", "commit", "--no-diff"]).unwrap();
    let cmd = args.cmd.unwrap();

    let Subcommands::Commit(commit_args) = cmd else {
        panic!("expected commit command. Got {cmd:?}");
    };

    assert!(!commit_args.diff);
    assert!(commit_args.no_diff);
}

#[test]
fn conflicting_diff_flags() {
    assert!(Args::try_parse_from(["but", "commit", "--diff", "--no-diff"]).is_err());
}
