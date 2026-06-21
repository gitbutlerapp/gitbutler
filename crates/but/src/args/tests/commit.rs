use clap::Parser as _;

use crate::args::{Args, Subcommands, commit::Subcommands as CommitSubcommands};

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

#[test]
fn batch_pairs_messages_with_change_groups() {
    let args = Args::try_parse_from([
        "but",
        "commit",
        "batch",
        "feature",
        "-m",
        "Refactor validation",
        "--changes",
        "a1,b2",
        "-m",
        "Add docs",
        "-p",
        "c3,d4",
    ])
    .unwrap();
    let cmd = args.cmd.unwrap();

    let Subcommands::Commit(commit_args) = cmd else {
        panic!("expected commit command. Got {cmd:?}");
    };
    let Some(CommitSubcommands::Batch {
        branch,
        messages,
        changes,
        ..
    }) = commit_args.cmd
    else {
        panic!("expected commit batch command. Got {:?}", commit_args.cmd);
    };

    assert_eq!(branch.map(|branch| branch.0).as_deref(), Some("feature"));
    assert_eq!(messages, ["Refactor validation", "Add docs"]);
    assert_eq!(changes, ["a1,b2", "c3,d4"]);
}

#[test]
fn batch_pairs_by_occurrence_index_not_flag_order() {
    let args = Args::try_parse_from([
        "but",
        "commit",
        "batch",
        "feature",
        "--changes",
        "a1,b2",
        "-m",
        "Refactor validation",
        "-p",
        "c3,d4",
        "-m",
        "Add docs",
    ])
    .unwrap();
    let cmd = args.cmd.unwrap();

    let Subcommands::Commit(commit_args) = cmd else {
        panic!("expected commit command. Got {cmd:?}");
    };
    let Some(CommitSubcommands::Batch {
        messages, changes, ..
    }) = commit_args.cmd
    else {
        panic!("expected commit batch command. Got {:?}", commit_args.cmd);
    };

    assert_eq!(messages, ["Refactor validation", "Add docs"]);
    assert_eq!(changes, ["a1,b2", "c3,d4"]);
}

#[test]
fn batch_requires_messages_and_change_groups() {
    assert!(Args::try_parse_from(["but", "commit", "batch", "feature", "-m", "Refactor"]).is_err());
    assert!(
        Args::try_parse_from(["but", "commit", "batch", "feature", "--changes", "a1"]).is_err()
    );
}
