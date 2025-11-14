use crate::args::Args;
use crate::metrics::Props;
use colored::Colorize;
use std::io::Write;

/// How we should format anything written to [`std::io::stdout()`].
#[derive(Debug, Copy, Clone)]
pub enum OutputFormat {
    /// The output to write is supposed to be for human consumption, and can be more verbose.
    Human,
    /// The output should be suitable for shells, and assigning the major result to variables so that it can be re-used
    /// in subsequent CLI invocations.
    Shell,
}

/// Where to write
pub enum OutputTarget {
    Stdout,
    Blackhole,
}

/// A utility `std::io::Write` implementation that can always be used to generate output for humans or for scripts.
pub struct Output {
    /// How to print the output, one should match on it.
    pub format: OutputFormat,
    /// The actual writer.
    inner: Box<dyn std::io::Write>,
}

impl Write for Output {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// Lifecycle
impl Output {
    /// Create a new instance to output to `target`.
    pub fn new(target: OutputTarget, format: Option<OutputFormat>) -> Self {
        Output {
            format: match target {
                OutputTarget::Stdout => format.unwrap_or_else(|| {
                    if atty::is(atty::Stream::Stdout) {
                        OutputFormat::Human
                    } else {
                        OutputFormat::Shell
                    }
                }),
                OutputTarget::Blackhole => {
                    // Most likely implemented, and it doesn't matter
                    OutputFormat::Human
                }
            },
            inner: match target {
                OutputTarget::Stdout => Box::new(std::io::stdout()),
                OutputTarget::Blackhole => Box::new(std::io::sink()),
            },
        }
    }
}

/// Utilities attached to `anyhow::Result<impl serde::Serialize>`.
pub trait ResultExt {
    /// Write this value as pretty `JSON` to stdout if `json` is `true`.
    fn output_json(self, json: bool) -> Self;
}

impl<T> ResultExt for anyhow::Result<T>
where
    T: serde::Serialize,
{
    fn output_json(self, json: bool) -> Self {
        if json && let Ok(value) = &self {
            let stdout = std::io::stdout();
            let mut stdout = stdout.lock();
            serde_json::to_writer_pretty(&mut stdout, value)
                .expect("This is to indicate that the write failed, leading to invalid JSON");
            stdout.write_all(b"\n").ok();
        }
        self
    }
}

/// A placeholder, which should be substituted for the actual return value.
pub fn we_need_proper_json_output_here() -> serde_json::Value {
    serde_json::Value::Null
}

/// Convert anything into a json value, **or panic**.
/// I think this should never fail at runtime, but I am not sure.
pub fn into_json_value(value: impl serde::Serialize) -> serde_json::Value {
    serde_json::to_value(&value)
        .expect("BUG: Failed to serialize JSON value, we should know that at compile time")
}

pub fn props<E, T, R>(start: std::time::Instant, result: R) -> Props
where
    R: std::ops::Deref<Target = anyhow::Result<T, E>>,
    E: std::fmt::Display,
{
    let error = result.as_ref().err().map(|e| e.to_string());
    let mut props = Props::new();
    props.insert("durationMs", start.elapsed().as_millis());
    props.insert("error", error);
    props
}

pub fn print_grouped_help() -> std::io::Result<()> {
    use std::collections::HashSet;

    use clap::CommandFactory;
    use terminal_size::{Width, terminal_size};

    let mut stdout = std::io::stdout();

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

    writeln!(
        stdout,
        "{}",
        "The GitButler CLI change control system".red()
    )?;
    writeln!(stdout)?;
    writeln!(stdout, "Usage: but [OPTIONS] <COMMAND>")?;
    writeln!(stdout, "       but [OPTIONS] [RUB-SOURCE] [RUB-TARGET]")?;
    writeln!(stdout)?;
    writeln!(
        stdout,
        "The GitButler CLI can be used to do nearly anything the desktop client can do (and more)."
    )?;
    writeln!(
        stdout,
        "It is a drop in replacement for most of the Git commands you would normally use, but Git"
    )?;
    writeln!(
        stdout,
        "commands (blame, log, etc) can also be used, as GitButler is fully Git compatible."
    )?;
    writeln!(stdout)?;
    writeln!(
        stdout,
        "Checkout the full docs here: https://docs.gitbutler.com/cli-overview"
    )?;
    writeln!(stdout)?;

    // Keep track of which commands we've already printed
    let mut printed_commands = HashSet::new();
    const LONGEST_COMMAND_LEN: usize = 13;
    const LONGEST_COMMAND_LEN_AND_ELLIPSIS: usize = LONGEST_COMMAND_LEN + 3;

    // Print grouped commands
    for (group_name, command_names) in &groups {
        writeln!(stdout, "{group_name}:")?;
        for cmd_name in command_names {
            if let Some(subcmd) = subcommands.iter().find(|c| c.get_name() == *cmd_name) {
                let about = subcmd.get_about().unwrap_or_default().to_string();
                // Calculate available width: terminal_width - indent (2) - command column (10) - buffer (1)
                let available_width =
                    terminal_width.saturating_sub(LONGEST_COMMAND_LEN_AND_ELLIPSIS);
                let truncated_about = truncate_text(&about, available_width);
                writeln!(
                    stdout,
                    "  {:<LONGEST_COMMAND_LEN$}{}",
                    cmd_name.green(),
                    truncated_about,
                )?;
                printed_commands.insert(cmd_name.to_string());
            }
        }
        writeln!(stdout)?;
    }

    // Collect any remaining commands not in the explicit groups
    let misc_commands: Vec<_> = subcommands
        .iter()
        .filter(|subcmd| !printed_commands.contains(subcmd.get_name()) && !subcmd.is_hide_set())
        .collect();

    // Print MISC section if there are any ungrouped commands
    if !misc_commands.is_empty() {
        writeln!(stdout, "{}:", "Other Commands".yellow())?;
        for subcmd in misc_commands {
            let about = subcmd.get_about().unwrap_or_default().to_string();
            // Calculate available width: terminal_width - indent (2) - command column (10) - buffer (1)
            let available_width = terminal_width.saturating_sub(LONGEST_COMMAND_LEN_AND_ELLIPSIS);
            let truncated_about = truncate_text(&about, available_width);
            writeln!(
                stdout,
                "  {:<LONGEST_COMMAND_LEN$}{}",
                subcmd.get_name().green(),
                truncated_about
            )?;
        }
        writeln!(stdout)?;
    }

    // Add command completion instructions
    writeln!(
        stdout,
        "To add command completion, add this to your shell rc: (for example ~/.zshrc)"
    )?;
    writeln!(stdout, "  eval \"$(but completions zsh)\"")?;
    writeln!(stdout)?;

    writeln!(
        stdout,
        "To use the GitButler CLI with coding agents (Claude Code hooks, Cursor hooks, MCP), see:"
    )?;
    writeln!(
        stdout,
        "  https://docs.gitbutler.com/features/ai-integration/ai-overview"
    )?;
    writeln!(stdout)?;

    writeln!(stdout, "{}:", "Options".yellow())?;
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
        writeln!(stdout, "{}  {}", flag, truncated_desc)?;
    }

    Ok(())
}
