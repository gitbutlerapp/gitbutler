use clap::Parser as _;

use crate::args::{Args, Subcommands};

#[test]
fn uses_the_promoted_diff_arguments() {
    let args = Args::try_parse_from(["but", "diff", "some-target"]).unwrap();
    let cmd = args.cmd.unwrap();

    let Subcommands::Diff(args) = cmd else {
        panic!("expected diff command. Got {cmd:?}");
    };

    assert!(args.target.is_some(), "the diff target should be parsed");
}

#[test]
fn old_tui_flags_are_removed() {
    assert!(
        Args::try_parse_from(["but", "diff", "--tui"]).is_err(),
        "the old diff TUI should only be available through `but tui --diff`"
    );
    assert!(
        Args::try_parse_from(["but", "diff", "--no-tui"]).is_err(),
        "the old diff TUI override should be removed"
    );
}

#[test]
fn diff2_is_no_longer_a_builtin() {
    let args = Args::try_parse_from(["but", "_diff2"]).unwrap();

    assert!(
        matches!(args.cmd, Some(Subcommands::External(_))),
        "unknown commands should fall through to external command lookup"
    );
}
