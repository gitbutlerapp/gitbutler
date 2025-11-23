use crate::args::Args;
use colored::Colorize;

pub fn print_grouped(out: &mut dyn std::fmt::Write) -> std::fmt::Result {
    use std::collections::HashSet;

    use clap::CommandFactory;
    use terminal_size::{Width, terminal_size};

    // Get terminal width, default to 80 if detection fails
    let terminal_width = if let Some((Width(w), _)) = terminal_size() {
        w as usize
    } else {
        80
    };

    // Helper function to truncate text to fit within available width
    let truncate_text = |text: &str, available_width: usize| -> String {
        const ELLIPSIS_LEN: usize = 1;
        if text.len() <= available_width {
            text.to_string()
        } else if available_width > ELLIPSIS_LEN {
            format!("{}…", &text[..available_width.saturating_sub(ELLIPSIS_LEN)])
        } else {
            text.chars().take(available_width).collect()
        }
    };

    let cmd = Args::command();
    let subcommands: Vec<_> = cmd.get_subcommands().collect();

    // Define command groupings and their order (excluding MISC)
    let groups = [
        ("Inspection".yellow(), vec!["status"]),
        (
            "Branching and Committing".yellow(),
            vec!["commit", "new", "branch", "base", "mark", "unmark"],
        ),
        (
            "Server Interactions".yellow(),
            vec!["push", "review", "forge"],
        ),
        (
            "Editing Commits".yellow(),
            vec!["rub", "describe", "absorb"],
        ),
        (
            "Operation History".yellow(),
            vec!["oplog", "undo", "restore", "snapshot"],
        ),
    ];

    writeln!(out, "{}", "The GitButler CLI change control system".red())?;
    writeln!(out)?;
    writeln!(out, "Usage: but [OPTIONS] <COMMAND>")?;
    writeln!(out, "       but [OPTIONS] [RUB-SOURCE] [RUB-TARGET]")?;
    writeln!(out)?;
    writeln!(
        out,
        "The GitButler CLI can be used to do nearly anything the desktop client can do (and more)."
    )?;
    writeln!(
        out,
        "It is a drop in replacement for most of the Git commands you would normally use, but Git"
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
                    cmd_name.green(),
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
        writeln!(out, "{}:", "Other Commands".yellow())?;
        for subcmd in misc_commands {
            let about = subcmd.get_about().unwrap_or_default().to_string();
            // Calculate available width: terminal_width - indent (2) - command column (10) - buffer (1)
            let available_width = terminal_width.saturating_sub(LONGEST_COMMAND_LEN_AND_ELLIPSIS);
            let truncated_about = truncate_text(&about, available_width);
            writeln!(
                out,
                "  {:<LONGEST_COMMAND_LEN$}{}",
                subcmd.get_name().green(),
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

    writeln!(out, "{}:", "Options".yellow())?;
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
        writeln!(out, "{}  {}", flag, truncated_desc)?;
    }

    Ok(())
}
