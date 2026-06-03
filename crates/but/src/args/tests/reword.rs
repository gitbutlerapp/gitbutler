use clap::Parser as _;

use crate::args::{Args, Subcommands};

#[test]
fn fix_formatting() {
    let args = Args::try_parse_from(["but", "reword", "a", "--fix-formatting"]).unwrap();
    let cmd = args.cmd.unwrap();

    let Subcommands::Reword { format, .. } = cmd else {
        panic!("expected reword command. Got {cmd:?}");
    };

    assert!(format);
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
