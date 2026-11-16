use indexmap::IndexMap;
use strum::IntoEnumIterator as _;

use crate::args::{Args, HelpTopic, SubcommandDiscriminant};
use crate::theme::{self, Paint};
use crate::tui::text::{terminal_width, truncate_text};
use crate::utils::{OutputChannel, envs};

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, strum::EnumIter)]
pub(crate) enum Group {
    Inspection,
    BranchingAndCommitting,
    EditingCommits,
    OperationHistory,
    ServerInteractions,
    OtherCommands,
}

impl std::fmt::Display for Group {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Group::Inspection => "Inspection",
            Group::BranchingAndCommitting => "Branching and Committing",
            Group::ServerInteractions => "Server Interactions",
            Group::EditingCommits => "Editing Commits",
            Group::OperationHistory => "Operation History",
            Group::OtherCommands => "Other Commands",
        })
    }
}

pub fn print(out: &mut OutputChannel, topic: Option<HelpTopic>) -> std::fmt::Result {
    match topic {
        Some(topic) => print_topic(out, topic),
        None => print_grouped(out),
    }
}

pub fn print_grouped(out: &mut OutputChannel) -> std::fmt::Result {
    let allow_truncation = out.format().allows_truncation();
    print_grouped_with_truncation(out, allow_truncation)
}

fn print_topic(out: &mut OutputChannel, topic: HelpTopic) -> std::fmt::Result {
    use clap::CommandFactory;
    use std::fmt::Write;

    let mut cmd = Args::command();
    let topic_command = cmd
        .find_subcommand_mut("help")
        .and_then(|help| help.find_subcommand_mut(topic.name()))
        .expect("all help topics have clap command metadata");

    let t = theme::get();
    writeln!(out, "{}", t.important.paint(topic.title()))?;
    writeln!(out)?;

    // We can't easily hook into Clap's colorize choice. It's only implemented in
    // `Command::print_long_help()` and that forces use of `std::io::Stdout`, side-stepping our
    // OutputChannel implementation.
    //
    // A full implementation here would entail using `anstream::AutoStream` along with
    // `Command::get_color()` and map that to `anstream::ColorChoice`. But just checking if the
    // output is a terminal is generally sufficient as all modern terminals support ANSI escape
    // codes, so we'll stay with this simple solution for now.
    let long_help = topic_command.render_long_help();
    if out.is_terminal() {
        writeln!(out, "{}", long_help.ansi())
    } else {
        writeln!(out, "{long_help}")
    }
}

/// The visible top-level commands in the groups and order `but --help` shows.
pub(crate) fn grouped_subcommands(cmd: &clap::Command) -> IndexMap<Group, Vec<&clap::Command>> {
    let clap_subcommands: Vec<_> = cmd.get_subcommands().collect();

    let mut groups = Group::iter()
        .map(|group| (group, Vec::new()))
        .collect::<IndexMap<_, Vec<_>>>();

    for subcommand_variant in SubcommandDiscriminant::iter() {
        if matches!(subcommand_variant, SubcommandDiscriminant::External) {
            // There is no explicit subcommand that corresponds to External
            continue;
        }

        if let Some(clap_subcommand) = clap_subcommands.iter().find(|clap_subcommand| {
            clap_subcommand.get_name().to_lowercase().replace('-', "")
                == subcommand_variant.as_ref().to_lowercase()
        }) {
            if clap_subcommand.is_hide_set() {
                continue;
            }

            // This determines the groups the commands are shown in.
            //
            // The order of the commands within the groups is determined by order of the variants
            // in the code for `enum Subcommands`.
            //
            // The order of the groups themselves is likewise determined by order of the variants
            // in `enum Group`.
            let group = match subcommand_variant {
                SubcommandDiscriminant::Status => Group::Inspection,
                SubcommandDiscriminant::Diff => Group::Inspection,
                SubcommandDiscriminant::Show => Group::Inspection,
                SubcommandDiscriminant::_Comment => Group::Inspection,

                SubcommandDiscriminant::Commit => Group::BranchingAndCommitting,
                SubcommandDiscriminant::Branch => Group::BranchingAndCommitting,
                SubcommandDiscriminant::Worktree => Group::BranchingAndCommitting,
                SubcommandDiscriminant::Discard => Group::BranchingAndCommitting,
                SubcommandDiscriminant::Unapply => Group::BranchingAndCommitting,
                SubcommandDiscriminant::Apply => Group::BranchingAndCommitting,
                SubcommandDiscriminant::Clean => Group::BranchingAndCommitting,
                SubcommandDiscriminant::Pick => Group::BranchingAndCommitting,
                SubcommandDiscriminant::Switch => Group::BranchingAndCommitting,
                SubcommandDiscriminant::Resolve => Group::BranchingAndCommitting,

                SubcommandDiscriminant::Push => Group::ServerInteractions,
                SubcommandDiscriminant::Pull => Group::ServerInteractions,
                SubcommandDiscriminant::Merge => Group::ServerInteractions,
                SubcommandDiscriminant::Pr => Group::ServerInteractions,

                SubcommandDiscriminant::Absorb => Group::EditingCommits,
                SubcommandDiscriminant::Reword | SubcommandDiscriminant::_Reword2 => {
                    Group::EditingCommits
                }
                SubcommandDiscriminant::Uncommit => Group::EditingCommits,
                SubcommandDiscriminant::Amend => Group::EditingCommits,
                SubcommandDiscriminant::Squash => Group::EditingCommits,
                SubcommandDiscriminant::Move => Group::EditingCommits,
                SubcommandDiscriminant::Split => Group::EditingCommits,

                SubcommandDiscriminant::Oplog => Group::OperationHistory,
                SubcommandDiscriminant::Undo => Group::OperationHistory,
                SubcommandDiscriminant::Redo => Group::OperationHistory,

                SubcommandDiscriminant::Init => Group::OtherCommands,
                SubcommandDiscriminant::Gui => Group::OtherCommands,
                SubcommandDiscriminant::Update => Group::OtherCommands,
                SubcommandDiscriminant::Alias => Group::OtherCommands,
                SubcommandDiscriminant::Config => Group::OtherCommands,
                SubcommandDiscriminant::Skill => Group::OtherCommands,
                SubcommandDiscriminant::Agent => Group::OtherCommands,
                SubcommandDiscriminant::Mcp => Group::OtherCommands,
                SubcommandDiscriminant::Help => Group::OtherCommands,
                SubcommandDiscriminant::Completions => Group::OtherCommands,

                SubcommandDiscriminant::Teardown => Group::OtherCommands,
                SubcommandDiscriminant::Tui => Group::OtherCommands,

                SubcommandDiscriminant::Edit => continue,
                #[cfg(feature = "nightly")]
                SubcommandDiscriminant::_Open => continue,
                #[cfg(feature = "nightly")]
                SubcommandDiscriminant::_Expand => continue,
                #[cfg(feature = "nightly")]
                SubcommandDiscriminant::_Ui => continue,
                SubcommandDiscriminant::Metrics => continue,
                SubcommandDiscriminant::Onboarding => continue,
                SubcommandDiscriminant::External => continue,

                SubcommandDiscriminant::RefreshRemoteData => continue,
                SubcommandDiscriminant::Fetch => continue,
                SubcommandDiscriminant::AgentLog | SubcommandDiscriminant::Setup => continue,
            };
            groups.entry(group).or_default().push(*clap_subcommand);
        } else {
            #[cfg(test)]
            panic!("no clap subcommand found for {subcommand_variant:?}");
        }
    }

    groups
}

fn print_grouped_with_truncation(
    out: &mut dyn std::fmt::Write,
    allow_truncation: bool,
) -> std::fmt::Result {
    use clap::CommandFactory;

    // Without truncation, an effectively infinite width makes truncate_text a no-op.
    let terminal_width = if allow_truncation {
        terminal_width()
    } else {
        usize::MAX
    };

    let cmd = Args::command();
    let clap_subcommands: Vec<_> = cmd.get_subcommands().collect();
    let groups = grouped_subcommands(&cmd);

    let t = theme::get();

    writeln!(
        out,
        "{}",
        t.error.paint("The GitButler CLI change control system")
    )?;
    writeln!(out)?;
    writeln!(out, "Usage: but [OPTIONS] [COMMAND]")?;
    writeln!(out)?;
    writeln!(
        out,
        "The GitButler CLI can be used to do nearly anything the desktop client can do (and more)."
    )?;
    writeln!(
        out,
        "It is a drop in replacement for most of the Git workflows you would normally use, but Git"
    )?;
    writeln!(
        out,
        "commands (blame, log, etc) can also be used, as GitButler is fully Git compatible."
    )?;
    writeln!(out)?;
    writeln!(
        out,
        "Checkout the full docs here: https://docs.gitbutler.com/cli-overview"
    )?;
    writeln!(out)?;

    const LONGEST_COMMAND_LEN: usize = 13;
    const LONGEST_COMMAND_LEN_AND_ELLIPSIS: usize = LONGEST_COMMAND_LEN + 3;

    for (group, clap_subcommands) in &groups {
        if clap_subcommands.is_empty() {
            continue;
        }

        writeln!(out, "{}:", t.important.paint(group.to_string()))?;
        for clap_subcommand in clap_subcommands {
            let about = clap_subcommand.get_about().unwrap_or_default().to_string();
            let available_width = terminal_width.saturating_sub(LONGEST_COMMAND_LEN_AND_ELLIPSIS);
            let truncated_about = truncate_text(&about, available_width);
            writeln!(
                out,
                "  {:<LONGEST_COMMAND_LEN$}{}",
                t.success.paint(clap_subcommand.get_name()),
                truncated_about,
            )?;
        }
        writeln!(out)?;
    }

    if let Some(help_command) = clap_subcommands
        .iter()
        .find(|subcommand| subcommand.get_name() == "help")
    {
        writeln!(
            out,
            "{} (view with {}):",
            t.important.paint("Help Topics"),
            t.important.paint("but help <topic>")
        )?;
        for topic_command in help_command.get_subcommands() {
            if topic_command.is_hide_set() {
                continue;
            }
            let about = topic_command.get_about().unwrap_or_default().to_string();
            let available_width = terminal_width.saturating_sub(LONGEST_COMMAND_LEN_AND_ELLIPSIS);
            let truncated_about = truncate_text(&about, available_width);
            writeln!(
                out,
                "  {:<LONGEST_COMMAND_LEN$}{}",
                t.success.paint(topic_command.get_name()),
                truncated_about,
            )?;
        }
        writeln!(out)?;
    }

    writeln!(
        out,
        "To add command completion, add this to your shell rc: (for example ~/.zshrc)"
    )?;
    writeln!(out, "  eval \"$(but completions zsh)\"")?;
    writeln!(out)?;

    writeln!(
        out,
        "To use the GitButler CLI with coding agents (Claude Code hooks, Cursor hooks, MCP), see:"
    )?;
    writeln!(
        out,
        "  https://docs.gitbutler.com/features/ai-integration/ai-overview"
    )?;
    writeln!(out)?;

    writeln!(out, "{}:", t.important.paint("Options"))?;
    let option_descriptions = [
        (
            "  -C, --current-dir <PATH>",
            "Run as if but was started in PATH instead of the current working directory [default: .]",
        ),
        (
            "      --json",
            "              Output detailed information as JSON for tool consumption",
        ),
        ("  -h, --help", "              Print help"),
    ];

    for (flag, desc) in option_descriptions {
        let available_width = terminal_width.saturating_sub(flag.len() + 2);
        let truncated_desc = truncate_text(desc, available_width);
        writeln!(out, "{flag}  {truncated_desc}")?;
    }

    writeln!(out)?;
    writeln!(out, "{}:", t.important.paint("Environment variables"))?;
    for (env, desc) in envs::ALL_ENVS {
        let env = format!("  {env}");
        let available_width = terminal_width.saturating_sub(env.len() + 2);
        let truncated_desc = truncate_text(*desc, available_width);
        writeln!(out, "{env}  {truncated_desc}")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::tui::text::strip_ansi_codes;

    #[test]
    fn test_print_grouped() {
        let mut buf = String::new();
        super::print_grouped_with_truncation(&mut buf, true).unwrap();

        snapbox::assert_data_eq!(
            // test without color because it doesn't work consistently on ci
            &*strip_ansi_codes(&buf),
            snapbox::str![[r#"
The GitButler CLI change control system

Usage: but [OPTIONS] [COMMAND]

The GitButler CLI can be used to do nearly anything the desktop client can do (and more).
It is a drop in replacement for most of the Git workflows you would normally use, but Git
commands (blame, log, etc) can also be used, as GitButler is fully Git compatible.

Checkout the full docs here: https://docs.gitbutler.com/cli-overview

Inspection:
  status       Show an overview of the workspace state
  diff         Show the diff of changes in the repo
  show         Show details of a commit or branch

Branching and Committing:
  commit       Create a commit
  branch       Commands for managing branches
  worktree     Manage worktrees
  discard      Discard branches, commits, or changes
  resolve      Resolve conflicts in a commit or in uncommitted files
  unapply      Remove a branch and the rest of its stack from the workspace, k…
  apply        Apply a branch
  clean        Remove empty branches from the workspace
  pick         Cherry-pick commits into an applied branch

Editing Commits:
  squash       Squash commits, branches, or changes
  move         Move commits and changes around
  split        Split a commit in two
  absorb       Amend uncommitted changes into the commits they belong to
  reword       Edit the commit message of the specified commit
  uncommit     Move commits, branches, or committed changes back into the unco…
  amend        Amend uncommitted changes into a commit or branch

Operation History:
  oplog        Commands for viewing and managing operation history
  undo         Undo the last operation
  redo         Redo the last undo

Server Interactions:
  merge        Merge a branch directly onto the target branch, bypassing review
  push         Push changes in a branch to remote
  pull         Update applied branches and worktrees onto the latest target
  pr           Commands for creating and managing reviews on a forge, e.g. Git…

Other Commands:
  init         Create an empty Git repository in the current directory
  teardown     Exit GitButler mode and return to normal Git workflow
  gui          Open the GitButler GUI for the current project
  tui          Open a live terminal workspace for branches, commits, changes, …
  update       Manage GitButler CLI and app updates
  alias        Manage command aliases
  config       View and manage GitButler configuration
  skill        Manage AI agent skills for GitButler
  agent        Set up or remove GitButler for AI coding agents
  completions  Generate but shell completions

Help Topics (view with but help <topic>):
  cli-ids      Smart IDs to reference commits, branches and more in but

To add command completion, add this to your shell rc: (for example ~/.zshrc)
  eval "$(but completions zsh)"

To use the GitButler CLI with coding agents (Claude Code hooks, Cursor hooks, MCP), see:
  https://docs.gitbutler.com/features/ai-integration/ai-overview

Options:
  -C, --current-dir <PATH>  Run as if but was started in PATH instead of the cu…
      --json                Output detailed information as JSON for tool consum…
  -h, --help                Print help

Environment variables:
  BUT_OUTPUT_FORMAT  Sets the output format when --json is not passed. Options:…
  BUT_PAGER  Sets the pager for large outputs. [default: less]
...

"#]]
        );
    }

    #[test]
    fn print_grouped_keeps_full_descriptions_when_truncation_is_disabled() {
        let mut buf = String::new();
        super::print_grouped_with_truncation(&mut buf, false).unwrap();
        let output = strip_ansi_codes(&buf);

        assert!(
            output.contains(
                "Move commits, branches, or committed changes back into the uncommitted area"
            ),
            "agent help should keep the full command description"
        );
        assert!(
            output.contains(
                "BUT_OUTPUT_FORMAT  Sets the output format when --json is not passed. Options: human, json."
            ),
            "agent help should document the output-format environment variable"
        );
    }
}
