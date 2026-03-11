use clap::Parser as _;

use crate::args::{Args, Subcommands};

#[test]
fn basic() {
    let args = Args::try_parse_from(["but", "reword", "a"]).unwrap();
    let cmd = args.cmd.unwrap();

    let Subcommands::Reword {
        target,
        message,
        format,
        diff,
        no_diff,
    } = cmd
    else {
        panic!("expected reword command. Got {cmd:?}");
    };

    assert_eq!(target, "a");
    assert_eq!(message, None);
    assert!(!format);
    assert!(!diff);
    assert!(!no_diff);
}

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
