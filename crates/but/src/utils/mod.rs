use std::io::Write;

use crate::{args::Args, metrics::MetricsContext};
use colored::Colorize;
use minus::ExitStrategy;

pub mod table;
pub use table::types::Table;

/// How we should format anything written to [`std::io::stdout()`].
#[derive(Debug, Copy, Clone, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// The output to write is supposed to be for human consumption, and can be more verbose.
    #[default]
    Human,
    /// The output should be suitable for shells, and assigning the major result to variables so that it can be re-used
    /// in subsequent CLI invocations.
    Shell,
    /// Output detailed information as JSON for tool consumption.
    Json,
    /// Do not output anything, like redirecting to `/dev/null`.
    None,
}

/// A utility `std::io::Write` implementation that can always be used to generate output for humans or for scripts.
pub struct OutputChannel {
    /// How to print the output, one should match on it. Match on this if you prefer this style.
    format: OutputFormat,
    /// The actual writer.
    // TODO: remove once nobody writes to io::out anymore.
    inner: std::io::Stdout,
    /// Possibly a pager we are using. If `Some`, our `inner` is the pager itself which we interact with from here.
    pager: Option<minus::Pager>,
}

/// Conversions
impl OutputChannel {
    /// Provide a write implementation for humans, if the format setting permits.
    pub fn for_human(&mut self) -> Option<&mut (dyn std::fmt::Write + 'static)> {
        matches!(self.format, OutputFormat::Human).then(|| self as &mut dyn std::fmt::Write)
    }
    /// Provide a write implementation for Shwll output, if the format setting permits.
    pub fn for_shell(&mut self) -> Option<&mut (dyn std::fmt::Write + 'static)> {
        matches!(self.format, OutputFormat::Shell).then(|| self as &mut dyn std::fmt::Write)
    }
    /// Provide a handle to receive a serde-serializable value to write to stdout.
    pub fn for_json(&mut self) -> Option<&mut Self> {
        matches!(self.format, OutputFormat::Json).then_some(self)
    }
}

/// JSON utilities
impl OutputChannel {
    /// Write `value` as pretty JSON to the output.
    ///
    /// Note that it's owned to avoid double-printing with [ResultJsonExt::output_json]
    pub fn write_value(&mut self, value: impl serde::Serialize) -> std::io::Result<()> {
        json_pretty_to_stdout(&value)
    }
}

fn json_pretty_to_stdout(value: &impl serde::Serialize) -> std::io::Result<()> {
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    let value = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
    if value != "null" {
        stdout.write_all(value.as_bytes())?;
        stdout.write_all(b"\n").ok();
    }
    Ok(())
}

impl std::fmt::Write for OutputChannel {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        match self.format {
            OutputFormat::Human | OutputFormat::Shell => {
                if let Some(out) = self.pager.as_mut() {
                    out.write_str(s)
                } else {
                    self.inner.write_all(s.as_bytes()).or_else(|err| {
                        if err.kind() == std::io::ErrorKind::BrokenPipe {
                            // Ignore broken pipes and keep writing.
                            // This allows the caller to use `?` without having to think
                            // about ignoring errors selectively.
                            Ok(())
                        } else {
                            Err(std::fmt::Error)
                        }
                    })
                }
            }
            OutputFormat::Json | OutputFormat::None => {
                // It's not an error to try to write in JSON mode, it's a feature.
                // However, the only way to write JSON is to use [Self::write_value()].
                Ok(())
            }
        }
    }
}

/// Lifecycle
impl OutputChannel {
    /// Create a new instance to output with `format` (advisory), which affects where it prints to.
    ///
    /// It's configured to print to stdout unless [`OutputFormat::Json`] is used, then it prints everything
    /// to a `/dev/null` equivalent, so callers never have to worry if they interleave JSON with other output.
    ///
    /// WARNING: the current implementation is static and would cache everything in memory.
    ///          Use `dynamic_output` (cargo feature + see https://docs.rs/minus/5.6.1/minus/#threads) otherwise.
    ///          It also needs to avoid
    pub fn new_with_pager(format: OutputFormat) -> Self {
        OutputChannel {
            format,
            inner: std::io::stdout(),
            pager: if !matches!(format, OutputFormat::Human)
                || std::env::var_os("NOPAGER").is_some()
            {
                None
            } else {
                let pager = minus::Pager::new();
                let msg = "can talk to newly created pager";
                pager.set_exit_strategy(ExitStrategy::PagerQuit).expect(msg);
                pager.set_prompt("GitButler").expect(msg);
                Some(pager)
            },
        }
    }

    /// Like [`Self::new_with_pager`], but will never create a pager or write JSON.
    /// Use this if a second instance of a channel is needed, and the first one could have a pager.
    pub fn new_without_pager_non_json(format: OutputFormat) -> Self {
        OutputChannel {
            format: match format {
                OutputFormat::Human | OutputFormat::Shell | OutputFormat::None => format,
                OutputFormat::Json => OutputFormat::None,
            },
            inner: std::io::stdout(),
            pager: None,
        }
    }
}

impl Drop for OutputChannel {
    fn drop(&mut self) {
        if let Some(pager) = self.pager.take() {
            minus::page_all(pager).ok();
        }
    }
}

/// Utilities attached to `anyhow::Result<impl serde::Serialize>`.
pub trait ResultJsonExt {
    /// Write this value as pretty `JSON` to stdout if `json` is `true`.
    ///
    /// This style is great if you don't want to forget that JSON must be implemented.
    /// Note that "null" isn't printed and silently dropped.
    fn output_json(self, json: bool) -> anyhow::Result<()>;
}

pub trait ResultErrorExt {
    fn show_root_cause_error_then_exit(self) -> !;
}

impl ResultErrorExt for anyhow::Result<()> {
    fn show_root_cause_error_then_exit(self) -> ! {
        let code = if let Err(e) = &self {
            writeln!(std::io::stderr(), "{} {}", e, e.root_cause()).ok();
            1
        } else {
            0
        };
        std::process::exit(code);
    }
}

/// Utilities attached to `anyhow::Result<T>`.
pub trait ResultMetricsExt {
    fn emit_metrics(self, ctx: Option<MetricsContext>) -> anyhow::Result<()>;
}

impl<T> ResultJsonExt for anyhow::Result<T>
where
    T: serde::Serialize,
{
    fn output_json(self, json: bool) -> anyhow::Result<()> {
        if json && let Ok(value) = &self {
            json_pretty_to_stdout(value)?;
        }
        self.map(|_| ())
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

pub fn print_grouped_help(out: &mut dyn std::fmt::Write) -> std::fmt::Result {
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
