use clap::Parser as _;

use crate::args::{Args, Subcommands};

#[test]
fn always_show_diff() {
    let args = Args::try_parse_from(["but", "reword", "a", "--diff"]).unwrap();
    let cmd = args.cmd.unwrap();

    let Subcommands::Reword { diff, no_diff, .. } = cmd else {
        panic!("expected reword command. Got {cmd:?}");
    };

    assert!(diff);
    assert!(!no_diff);
}

#[test]
fn never_show_diff() {
    let args = Args::try_parse_from(["but", "reword", "a", "--no-diff"]).unwrap();
    let cmd = args.cmd.unwrap();

    let Subcommands::Reword { diff, no_diff, .. } = cmd else {
        panic!("expected reword command. Got {cmd:?}");
    };

    assert!(!diff);
    assert!(no_diff);
}

#[test]
fn conflicting_diff_flags() {
    assert!(Args::try_parse_from(["but", "reword", "a", "--diff", "--no-diff"]).is_err());
}
