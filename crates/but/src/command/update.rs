use anyhow::Result;
#[cfg(unix)]
use but_installer::VersionRequest;
use but_settings::AppSettings;
use but_update::{AppName, CheckUpdateStatus, check_status};
use colored::Colorize;

use crate::{args::update, utils::OutputChannel};

pub fn handle(cmd: update::Subcommands, out: &mut OutputChannel, app_settings: &AppSettings) -> Result<()> {
    match cmd {
        update::Subcommands::Check => check_for_updates(out, app_settings),
        update::Subcommands::Suppress { days } => suppress_updates(out, days),
        #[cfg(unix)]
        update::Subcommands::Install { target } => install(out, target),
    }
}

fn check_for_updates(out: &mut OutputChannel, app_settings: &AppSettings) -> Result<()> {
    let mut cache = but_db::AppCacheHandle::new_in_directory(but_path::app_cache_dir().ok());
    let status = check_status(AppName::Cli, app_settings, &mut cache)?;

    if let Some(status) = status {
        if let Some(writer) = out.for_human() {
            print_human_output(writer, &status)?;
        } else if let Some(out) = out.for_json() {
            out.write_value(&status)?;
        } else if let Some(writer) = out.for_shell()
            && !status.up_to_date
        {
            writeln!(writer, "{}", status.latest_version)?;
        }
    } else if let Some(writer) = out.for_human() {
        writeln!(
            writer,
            "{} Another process is currently checking for updates",
            "→".yellow().bold()
        )?;
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
        let current_version = option_env!("VERSION").unwrap_or("0.0.0");
        writeln!(
            writer,
            "{} A new version is available: {} {} {}. Install it with 'but update install'",
            "→".yellow().bold(),
            current_version.dimmed(),
            "→".dimmed(),
            status.latest_version.green().bold()
        )?;

        if let Some(notes) = &status.release_notes {
            let trimmed = notes.trim();
            if !trimmed.is_empty() {
                writeln!(writer)?;
                writeln!(writer, "{trimmed}")?;
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
    let mut cache = but_db::AppCacheHandle::new_in_directory(but_path::app_cache_dir().ok());
    but_update::suppress_update(&mut cache, hours)?;

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

#[cfg(unix)]
fn install(out: &mut OutputChannel, target: Option<String>) -> Result<()> {
    // Installation requires interactive output and cannot be used with JSON mode
    // because the installer writes directly to stdout/stderr
    if out.for_json().is_some() {
        anyhow::bail!(
            "JSON output is not supported for 'but update install'.\n\n\
             The installation process requires interactive output.\n\
             For automated installations, use the standalone installer:\n\
             https://docs.gitbutler.com/installation"
        );
    }

    // Parse target to determine what to install
    let version_request = match target.as_deref() {
        Some("nightly") => VersionRequest::Nightly,
        Some("release") => VersionRequest::Release,
        Some(version_str) => {
            // Specific version - validate and create
            // Wrap validation errors with CLI-specific context
            VersionRequest::from_string(Some(version_str.to_string())).map_err(|e| {
                anyhow::anyhow!(
                    "Invalid version '{version_str}': {e}\n\nValid targets:\n  nightly          Install latest nightly build\n  release          Install latest stable release\n  <version>        Install specific version (e.g., 0.18.7)"
                )
            })?
        }
        None => {
            // Auto-detect from current channel
            let current_channel = but_path::AppChannel::new();
            match current_channel {
                but_path::AppChannel::Nightly => VersionRequest::Nightly,
                but_path::AppChannel::Release => VersionRequest::Release,
                but_path::AppChannel::Dev => VersionRequest::Release, // Dev installs release
            }
        }
    };

    // Call installer directly (handles all user-facing output)
    // Don't print usage info since user is already using the CLI
    but_installer::run_installation_with_version(version_request, false)?;

    // Show change log link
    if let Some(writer) = out.for_human() {
        writeln!(
            writer,
            "{} {}",
            "→".cyan().bold(),
            "View release notes: https://gitbutler.com/releases".bold()
        )?;
        writeln!(writer)?;
    }

    Ok(())
}
