use anyhow::Result;
use but_settings::AppSettings;
use but_update::{AppName, CheckUpdateStatus, check_status};
use colored::Colorize;

use crate::{args::update, utils::OutputChannel};

pub fn handle(
    cmd: update::Subcommands,
    out: &mut OutputChannel,
    app_settings: &AppSettings,
) -> Result<()> {
    match cmd {
        update::Subcommands::Check => check_for_updates(out, app_settings),
        update::Subcommands::Suppress { days } => suppress_updates(out, days),
    }
}

fn check_for_updates(out: &mut OutputChannel, app_settings: &AppSettings) -> Result<()> {
    let status = check_status(AppName::Cli, app_settings)?;

    if let Some(writer) = out.for_human() {
        print_human_output(writer, &status)?;
    } else if let Some(out) = out.for_json() {
        out.write_value(&status)?;
    } else if let Some(writer) = out.for_shell()
        && !status.up_to_date
    {
        writeln!(writer, "{}", status.latest_version)?;
    }

    Ok(())
}

fn print_human_output(writer: &mut dyn std::fmt::Write, status: &CheckUpdateStatus) -> Result<()> {
    if status.up_to_date {
        writeln!(
            writer,
            "{} You're running the latest version ({})",
            "✓".green().bold(),
            status.latest_version.bold()
        )?;
    } else {
        writeln!(
            writer,
            "{} A new version is available: {} {} {}",
            "→".yellow().bold(),
            env!("CARGO_PKG_VERSION").dimmed(),
            "→".dimmed(),
            status.latest_version.green().bold()
        )?;

        if let Some(notes) = &status.release_notes {
            let trimmed = notes.trim();
            if !trimmed.is_empty() {
                writeln!(writer)?;
                writeln!(writer, "{}", trimmed)?;
            }
        }

        if let Some(url) = &status.url {
            writeln!(writer)?;
            writeln!(writer, "Download: {}", url.cyan())?;
        }
    }

    Ok(())
}

fn suppress_updates(out: &mut OutputChannel, days: u32) -> Result<()> {
    // Convert days to hours (the API uses hours)
    // Note: days is already validated to be 1-30 by clap, so no overflow possible
    let hours = days * 24;

    // Call the suppress_update function
    but_update::suppress_update(hours)?;

    if let Some(writer) = out.for_human() {
        writeln!(
            writer,
            "{} Update notifications suppressed for {} {}",
            "✓".green().bold(),
            days,
            if days == 1 { "day" } else { "days" }
        )?;
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({
            "suppressed": true,
            "days": days,
            "hours": hours
        }))?;
    }

    Ok(())
}
