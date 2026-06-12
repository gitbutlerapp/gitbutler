use indexmap::IndexMap;
use strum::IntoEnumIterator as _;

use crate::args::{Args, SubcommandDiscriminant};
use crate::theme::{self, Paint};
use crate::tui::text::{terminal_width, truncate_text};
use crate::utils::{OutputChannel, envs};

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, strum::EnumIter)]
enum Group {
    Inspection,
    BranchingAndCommitting,
    EditingCommits,
    OperationHistory,
    ServerInteractions,
    Rules,
    OtherCommands,
}

impl std::fmt::Display for Group {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Group::Inspection => "Inspection",
            Group::BranchingAndCommitting => "Branching and Committing",
            Group::Rules => "Rules",
            Group::ServerInteractions => "Server Interactions",
            Group::EditingCommits => "Editing Commits",
            Group::OperationHistory => "Operation History",
            Group::OtherCommands => "Other Commands",
        })
    }
}

pub fn print_grouped(out: &mut OutputChannel) -> std::fmt::Result {
    let allow_truncation = out.format().allows_truncation();
    print_grouped_with_truncation(out, allow_truncation)
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
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Status => Group::Inspection,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Diff => Group::Inspection,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Show => Group::Inspection,

                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Commit => Group::BranchingAndCommitting,
                #[cfg(all(feature = "legacy", feature = "but-2"))]
                SubcommandDiscriminant::Commit2 => Group::BranchingAndCommitting,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Stage => Group::BranchingAndCommitting,
                SubcommandDiscriminant::Branch => Group::BranchingAndCommitting,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Unstage => Group::BranchingAndCommitting,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Merge => Group::BranchingAndCommitting,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Discard => Group::BranchingAndCommitting,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Unapply => Group::BranchingAndCommitting,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Apply => Group::BranchingAndCommitting,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Clean => Group::BranchingAndCommitting,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Pick => Group::BranchingAndCommitting,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Resolve => Group::BranchingAndCommitting,

                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Mark => Group::Rules,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Unmark => Group::Rules,

                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Push => Group::ServerInteractions,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Pull => Group::ServerInteractions,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Pr => Group::ServerInteractions,

                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Rub => Group::EditingCommits,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Absorb => Group::EditingCommits,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Reword => Group::EditingCommits,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Uncommit => Group::EditingCommits,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Amend => Group::EditingCommits,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Squash => Group::EditingCommits,
                SubcommandDiscriminant::Move => Group::EditingCommits,

                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Oplog => Group::OperationHistory,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Undo => Group::OperationHistory,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Redo => Group::OperationHistory,

                SubcommandDiscriminant::Gui => Group::OtherCommands,
                SubcommandDiscriminant::Update => Group::OtherCommands,
                SubcommandDiscriminant::Alias => Group::OtherCommands,
                SubcommandDiscriminant::Config => Group::OtherCommands,
                SubcommandDiscriminant::Skill => Group::OtherCommands,
                SubcommandDiscriminant::Help => Group::OtherCommands,

                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Setup => Group::OtherCommands,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Teardown => Group::OtherCommands,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Tui => Group::OtherCommands,

                SubcommandDiscriminant::Edit => continue,
                SubcommandDiscriminant::Metrics => continue,
                SubcommandDiscriminant::Completions => continue,
                SubcommandDiscriminant::Onboarding => continue,
                SubcommandDiscriminant::EvalHook => continue,
                SubcommandDiscriminant::External => continue,

                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Worktree => continue,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::RefreshRemoteData => continue,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Mcp => continue,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Actions => continue,
                #[cfg(feature = "legacy")]
                SubcommandDiscriminant::Fetch => continue,
                SubcommandDiscriminant::AgentLog => continue,
            };
            groups.entry(group).or_default().push(*clap_subcommand);
        } else {
            #[cfg(test)]
            panic!("no clap subcommand found for {subcommand_variant:?}");
        }
    }

    // Define command groupings and their order (excluding MISC)
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

    // Print grouped commands
    for (group, clap_subcommands) in &groups {
        if clap_subcommands.is_empty() {
            continue;
        }

        writeln!(out, "{}:", t.important.paint(group.to_string()))?;
        for clap_subcommand in clap_subcommands {
            let about = clap_subcommand.get_about().unwrap_or_default().to_string();
            // Calculate available width: terminal_width - indent (2) - command column (10) - buffer (1)
            let available_width = terminal_width.saturating_sub(LONGEST_COMMAND_LEN_AND_ELLIPSIS);
            let truncated_about = truncate_text(&about, available_width);
            writeln!(
                out,
                "  {:<LONGEST_COMMAND_LEN$}{}",
                t.success.paint(clap_subcommand.get_name()),
                truncated_about,
            )?;
            // printed_commands.insert(cmd_name.to_string());
        }
        writeln!(out)?;
    }

    // Add command completion instructions
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
    // Truncate long option descriptions if needed
    let option_descriptions = [
        (
            "  -C, --current-dir <PATH>",
            "Run as if but was started in PATH instead of the current working directory [default: .]",
        ),
        (
            "      --format <FORMAT>",
            "   Explicitly control how output should be formatted [possible values: human, agent, shell, json, none]",
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
        let truncated_desc = truncate_text(desc, available_width);
        writeln!(out, "{env}  {truncated_desc}")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::tui::text::strip_ansi_codes;

    #[test]
    #[cfg(feature = "legacy")]
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
  status       Overview of the project workspace state.
  diff         Displays the diff of changes in the repo.
  show         Shows detailed information about a commit or branch.

Branching and Committing:
  commit       Commit changes to a stack.
  stage        Stages a file or hunk to a specific branch.
  branch       Commands for managing branches.
  merge        Merge a branch into your local target branch.
  discard      Discard uncommitted changes from the worktree.
  resolve      Resolve conflicts in a commit.
  unapply      Unapply a branch from the workspace.
  apply        Apply a branch to the workspace.
  clean        Remove empty branches from the workspace.
  pick         Cherry-pick a commit from an unapplied branch into an applied v…

Editing Commits:
  rub          Combines two entities together to perform an operation like ame…
  absorb       Amends changes into the appropriate commits where they belong.
  reword       Edit the commit message of the specified commit.
  uncommit     Uncommit changes from a commit or file-in-commit to the unstage…
  amend        Amend a file change into a specific commit and rebases any depe…
  squash       Squash commits together.
  move         Move a commit or branch to a different location.

Operation History:
  oplog        Commands for viewing and managing operation history.
  undo         Undo the last operation.
  redo         Redo the last undo.

Server Interactions:
  push         Push changes in a branch to remote.
  pull         Updates all applied branches to be up to date with the target b…
  pr           Commands for creating and managing reviews on a forge, e.g. Git…

Rules:
  mark         Mark a commit or branch for auto-stage or auto-commit.
  unmark       Removes any marks from the workspace.

Other Commands:
  setup        Sets up a GitButler project from a git repository in the curren…
  teardown     Exit GitButler mode and return to normal Git workflow.
  gui          Open the GitButler GUI for the current project.
  tui          Show an interactive TUI.
  update       Manage GitButler CLI and app updates.
  alias        Manage command aliases.
  config       View and manage GitButler configuration.
  skill        Manage AI agent skills for GitButler.

To add command completion, add this to your shell rc: (for example ~/.zshrc)
  eval "$(but completions zsh)"

To use the GitButler CLI with coding agents (Claude Code hooks, Cursor hooks, MCP), see:
  https://docs.gitbutler.com/features/ai-integration/ai-overview

Options:
  -C, --current-dir <PATH>  Run as if but was started in PATH instead of the cu…
      --format <FORMAT>     Explicitly control how output should be formatted […
  -h, --help                Print help

Environment variables:
  BUT_PAGER  Sets the pager for large outputs. [default: less]
  BUT_THEME  Sets the theme for but. Options: dark, light. [default: dark]

"#]]
        );
    }

    #[test]
    #[cfg(feature = "legacy")]
    fn print_grouped_keeps_full_descriptions_when_truncation_is_disabled() {
        let mut buf = String::new();
        super::print_grouped_with_truncation(&mut buf, false).unwrap();
        let output = strip_ansi_codes(&buf);

        assert!(
            output.contains(
                "Cherry-pick a commit from an unapplied branch into an applied virtual branch."
            ),
            "agent help should keep the full command description"
        );
        assert!(
            output.contains("possible values: human, agent, shell, json, none"),
            "manual format help should include agent"
        );
    }

    #[test]
    #[cfg(not(feature = "legacy"))]
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

Branching and Committing:
  branch       Commands for managing branches.

Editing Commits:
  move         Move a commit or branch to a different location.

Other Commands:
  gui          Open the GitButler GUI for the current project.
  update       Manage GitButler CLI and app updates.
  alias        Manage command aliases.
  config       View and manage GitButler configuration.
  skill        Manage AI agent skills for GitButler.

To add command completion, add this to your shell rc: (for example ~/.zshrc)
  eval "$(but completions zsh)"

To use the GitButler CLI with coding agents (Claude Code hooks, Cursor hooks, MCP), see:
  https://docs.gitbutler.com/features/ai-integration/ai-overview

Options:
  -C, --current-dir <PATH>  Run as if but was started in PATH instead of the cu…
      --format <FORMAT>     Explicitly control how output should be formatted […
  -h, --help                Print help

Environment variables:
  BUT_PAGER  Sets the pager for large outputs. [default: less]
  BUT_THEME  Sets the theme for but. Options: dark, light. [default: dark]

"#]]
        );
    }
}
