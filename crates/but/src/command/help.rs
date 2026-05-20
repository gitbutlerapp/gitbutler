use crate::args::Args;
use crate::theme::{self, Paint};
use crate::tui::text::{terminal_width, truncate_text};
use crate::utils::envs;

pub fn print_grouped(out: &mut dyn std::fmt::Write) -> std::fmt::Result {
    use std::collections::HashSet;

    use clap::CommandFactory;

    let terminal_width = terminal_width();

    let cmd = Args::command();
    let subcommands: Vec<_> = cmd.get_subcommands().collect();

    // Define command groupings and their order (excluding MISC)
    let t = theme::get();

    let groups = [
        (
            t.important.paint("Inspection"),
            vec!["status", "diff", "show"],
        ),
        (
            t.important.paint("Branching and Committing"),
            vec![
                "commit", "stage", "new", "branch", "merge", "discard", "resolve",
            ],
        ),
        (t.important.paint("Rules"), vec!["mark", "unmark"]),
        (
            t.important.paint("Server Interactions"),
            vec!["push", "pull", "base", "pr", "forge"],
        ),
        (
            t.important.paint("Editing Commits"),
            vec![
                "rub", "absorb", "reword", "uncommit", "amend", "squash", "move",
            ],
        ),
        (
            t.important.paint("Operation History"),
            vec!["oplog", "undo", "redo"],
        ),
    ];

    writeln!(
        out,
        "{}",
        t.error.paint("The GitButler CLI change control system")
    )?;
    writeln!(out)?;
    writeln!(out, "Usage: but [OPTIONS] [COMMAND]")?;
    writeln!(out, "       but [OPTIONS] [PATH]")?;
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

    // Keep track of which commands we've already printed
    let mut printed_commands = HashSet::new();
    const LONGEST_COMMAND_LEN: usize = 13;
    const LONGEST_COMMAND_LEN_AND_ELLIPSIS: usize = LONGEST_COMMAND_LEN + 3;

    // Print grouped commands
    for (group_name, command_names) in &groups {
        writeln!(out, "{group_name}:")?;
        for cmd_name in command_names {
            if let Some(subcmd) = subcommands.iter().find(|c| c.get_name() == *cmd_name) {
                let about = subcmd.get_about().unwrap_or_default().to_string();
                // Calculate available width: terminal_width - indent (2) - command column (10) - buffer (1)
                let available_width =
                    terminal_width.saturating_sub(LONGEST_COMMAND_LEN_AND_ELLIPSIS);
                let truncated_about = truncate_text(&about, available_width);
                writeln!(
                    out,
                    "  {:<LONGEST_COMMAND_LEN$}{}",
                    t.success.paint(cmd_name),
                    truncated_about,
                )?;
                printed_commands.insert(cmd_name.to_string());
            }
        }
        writeln!(out)?;
    }

    // Collect any remaining commands not in the explicit groups
    let misc_commands: Vec<_> = subcommands
        .iter()
        .filter(|subcmd| !printed_commands.contains(subcmd.get_name()) && !subcmd.is_hide_set())
        .collect();

    // Print MISC section if there are any ungrouped commands
    if !misc_commands.is_empty() {
        writeln!(out, "{}:", t.important.paint("Other Commands"))?;
        for subcmd in misc_commands {
            let about = subcmd.get_about().unwrap_or_default().to_string();
            // Calculate available width: terminal_width - indent (2) - command column (10) - buffer (1)
            let available_width = terminal_width.saturating_sub(LONGEST_COMMAND_LEN_AND_ELLIPSIS);
            let truncated_about = truncate_text(&about, available_width);
            writeln!(
                out,
                "  {:<LONGEST_COMMAND_LEN$}{}",
                t.success.paint(subcmd.get_name()),
                truncated_about
            )?;
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
        ("  -j, --json", "Whether to use JSON output format"),
        ("  -h, --help", "Print help"),
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
