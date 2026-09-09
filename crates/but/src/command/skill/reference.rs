//! The command reference `but skill reference` prints, rendered from the clap
//! command tree so it cannot drift from the binary. Every visible command gets
//! its intent line and its flags with their contracts; examples and long
//! descriptions stay in `but <cmd> --help`.

use std::fmt::Write as _;

use clap::{Arg, Command, CommandFactory as _};

use crate::{args::Args, command::help::grouped_subcommands};

/// Commands that serve humans, set agents up, or hold secrets: not agent work.
const SKIPPED: &[&str] = &[
    "but gui",
    "but tui",
    "but open",
    "but alias",
    "but completions",
    "but skill",
    "but agent",
    "but help",
    "but worktree",
    "but teardown",
    "but update suppress",
    "but update install",
    "but config ai",
    "but config metrics",
    "but config user",
    "but config forge list-users",
    "but config forge forget",
    "but config forge github-stacks",
    "but config target",
    "but config push-remote",
    "but config feature",
];

/// Flags stated once in the preamble, plus the ones clap adds to every command
/// when the tree is built (rendering needs a built tree: `Arg`'s `Display`
/// panics otherwise).
const SHARED_FLAGS: &[&str] = &["json", "status-after", "allow-merged", "help", "version"];

/// Flags that tune display or forge polling for a person at a terminal.
const SKIPPED_FLAGS: &[&str] = &[
    "but status --refresh-prs",
    "but status --no-hint",
    "but branch list --review",
    "but branch list --no-check",
    "but branch list --no-ahead",
    "but branch list --empty",
    "but branch show --review",
    "but branch show --ai",
    "but branch new --switch",
];

/// Flags under this help heading need a terminal, so agents never see them.
const INTERACTIVE: &str = "Interactive";

pub(super) fn render() -> String {
    let mut out = String::from(
        "# but command reference\n\n\
         - IDs for commits, branches, files, and hunks: `but help cli-ids`.\n\
         - Terms (workspace, applied, stack, target, `@`): `but skill concepts`.\n\
         - Every command accepts `--json`. Mutations accept `--status-after` to append the \
         resulting workspace status.\n\
         - History edits refuse branches and commits already merged upstream; \
         `--allow-merged` overrides that.\n\
         - Mutations are recorded in the operation log; `but undo` reverts the last one.\n\
         - To run against another directory, put `-C <path>` before the command: \
         `but -C <path> <cmd>`.\n\
         - Examples and details: `but <cmd> --help`.\n",
    );
    let mut root = Args::command();
    root.build();
    for (group, cmds) in grouped_subcommands(&root) {
        if cmds.is_empty() {
            continue;
        }
        let _ = write!(out, "\n## {group}\n");
        for cmd in cmds {
            write_command(&mut out, "but", cmd);
        }
    }
    out
}

fn write_command(out: &mut String, parent: &str, cmd: &Command) {
    let path = format!("{parent} {}", cmd.get_name());
    if cmd.is_hide_set() || SKIPPED.contains(&path.as_str()) {
        return;
    }
    let mut heading = path.clone();
    for arg in cmd.get_positionals().filter(|arg| !arg.is_hide_set()) {
        let _ = write!(heading, " {arg}");
    }
    // Positionals first: they are what the agent must supply.
    let listed = |arg: &&Arg| {
        !arg.is_hide_set()
            && arg.get_help_heading() != Some(INTERACTIVE)
            && !arg.get_long().is_some_and(|long| {
                SHARED_FLAGS.contains(&long)
                    || SKIPPED_FLAGS.contains(&format!("{path} --{long}").as_str())
            })
    };
    let positionals = cmd.get_positionals().filter(listed);
    let options = cmd
        .get_arguments()
        .filter(|arg| !arg.is_positional())
        .filter(listed);
    let args: Vec<&Arg> = positionals.chain(options).collect();
    // A command that only routes to subcommands says nothing its group
    // heading and subcommands do not.
    if !args.is_empty() || !cmd.has_subcommands() {
        let _ = write!(out, "\n### {heading}\n");
        if let Some(about) = cmd.get_about() {
            let _ = writeln!(out, "{about}");
        }
        for arg in args {
            let _ = writeln!(out, "- `{}` {}", signature(arg), description(arg));
        }
    }
    for sub in cmd.get_subcommands() {
        write_command(out, &path, sub);
    }
}

/// `[CHANGES]...` for a positional; `-m, --message <MESSAGE>` for an option.
fn signature(arg: &Arg) -> String {
    match (arg.get_short(), arg.get_long()) {
        (Some(short), Some(_)) => format!("-{short}, {arg}"),
        _ => arg.to_string(),
    }
}

/// The long help on one line, with the possible values and default clap
/// would append in `--help`.
fn description(arg: &Arg) -> String {
    let mut text = arg
        .get_long_help()
        .or_else(|| arg.get_help())
        .map(|help| {
            help.to_string()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    if !arg.get_action().takes_values() {
        return text;
    }
    let values: Vec<_> = arg
        .get_possible_values()
        .iter()
        .filter(|value| !value.is_hide_set())
        .map(|value| value.get_name().to_owned())
        .collect();
    if !values.is_empty() {
        let _ = write!(text, " [possible values: {}]", values.join(", "));
    }
    if !arg.is_hide_default_value_set() {
        let defaults: Vec<_> = arg
            .get_default_values()
            .iter()
            .map(|value| value.to_string_lossy())
            .collect();
        if !defaults.is_empty() {
            let _ = write!(text, " [default: {}]", defaults.join(" "));
        }
    }
    text
}
