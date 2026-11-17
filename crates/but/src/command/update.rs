use crate::{
    CliResult,
    args::update,
    theme::{Paint, Theme},
    utils::{CliOutput, CliOutputHuman, OutputChannel, WriteWithUtils},
};
use anyhow::Result;
#[cfg(all(unix, not(feature = "packaged-but-distribution")))]
use but_installer::VersionRequest;
use but_settings::AppSettings;
use but_update::{AppName, CheckUpdateStatus, check_status};

pub fn handle(
    cmd: update::Subcommands,
    out: &mut OutputChannel,
    app_settings: &AppSettings,
) -> CliResult<()> {
    match cmd {
        update::Subcommands::Check => out.print_cli_output(check_for_updates(app_settings)?),
        update::Subcommands::Suppress { days } => out.print_cli_output(suppress_updates(days)?),
        #[cfg(all(unix, not(feature = "packaged-but-distribution")))]
        update::Subcommands::Install { target } => Ok(install(out, target)?),
    }
}

#[must_use]
enum UpdateOutcome {
    /// `None` if another process is checking for updates.
    Checked(Option<CheckUpdateStatus>),
    Suppressed {
        days: u32,
        hours: u32,
    },
}

impl CliOutputHuman for UpdateOutcome {
    fn on_human(self, out: &mut dyn WriteWithUtils, _agent: bool, t: &'static Theme) -> Result<()> {
        match self {
            UpdateOutcome::Checked(Some(status)) => print_human_output(out, &status, t)?,
            UpdateOutcome::Checked(None) => {
                writeln!(
                    out,
                    "{} Another process is currently checking for updates",
                    t.attention.paint("→")
                )?;
            }
            UpdateOutcome::Suppressed { days, hours: _ } => {
                writeln!(
                    out,
                    "{} Update notifications suppressed for {} {}",
                    t.sym().success,
                    days,
                    if days == 1 { "day" } else { "days" }
                )?;
            }
        }
        Ok(())
    }
}

impl CliOutput for UpdateOutcome {
    fn on_json(self) -> impl serde::Serialize {
        match self {
            UpdateOutcome::Checked(status) => serde_json::json!(status),
            UpdateOutcome::Suppressed { days, hours } => serde_json::json!({
                "suppressed": true,
                "days": days,
                "hours": hours
            }),
        }
    }
}

fn check_for_updates(app_settings: &AppSettings) -> Result<UpdateOutcome> {
    let mut cache = but_ctx::Context::app_cache();
    let status = check_status(AppName::Cli, app_settings, &mut cache)?;
    Ok(UpdateOutcome::Checked(status))
}

fn print_human_output(
    writer: &mut dyn std::fmt::Write,
    status: &CheckUpdateStatus,
    t: &Theme,
) -> Result<()> {
    if status.up_to_date {
        writeln!(
            writer,
            "{} You're running the latest version ({})",
            t.sym().success,
            t.important.paint(&status.latest_version)
        )?;
    } else {
        let current_version = option_env!("VERSION").unwrap_or("0.0.0");

        let install_hint = {
            #[cfg(feature = "packaged-but-distribution")]
            {
                "Install it with your package manager"
            }
            // `but update install` only exists on Unix (see `args::update::Subcommands`), so only
            // suggest it there. On other platforms (e.g. Windows) point at the download instead.
            #[cfg(all(unix, not(feature = "packaged-but-distribution")))]
            {
                "Install it with 'but update install'"
            }
            #[cfg(all(not(unix), not(feature = "packaged-but-distribution")))]
            {
                "Download it from https://gitbutler.com/downloads"
            }
        };

        writeln!(
            writer,
            "{} A new version is available: {} {} {}. {}",
            t.attention.paint("→"),
            t.hint.paint(current_version),
            t.hint.paint("→"),
            t.success.paint(&status.latest_version),
            install_hint,
        )?;

        if let Some(notes) = &status.release_notes {
            let trimmed = notes.trim();
            if !trimmed.is_empty() {
                writeln!(writer)?;
                writeln!(writer, "{trimmed}")?;
            }
        }

        // currently always shows the AppImage URL on Linux, which is rarely what you want
        #[cfg(not(target_os = "linux"))]
        if let Some(url) = &status.url {
            writeln!(writer)?;
            writeln!(writer, "Download: {}", t.link.paint(url.as_str()))?;
        }
    }

    Ok(())
}

fn suppress_updates(days: u32) -> Result<UpdateOutcome> {
    // Convert days to hours (the API uses hours)
    // Note: days is already validated to be 1-30 by clap, so no overflow possible
    let hours = days * 24;

    let mut cache = but_ctx::Context::app_cache();
    but_update::suppress_update(&mut cache, hours)?;

    Ok(UpdateOutcome::Suppressed { days, hours })
}

#[cfg(all(unix, not(feature = "packaged-but-distribution")))]
fn install(out: &mut OutputChannel, target: Option<String>) -> Result<()> {
    // Installation requires interactive output and cannot be used with JSON mode
    // because the installer writes directly to stdout/stderr
    if out.is_json() {
        anyhow::bail!(
            "JSON output is not supported for 'but update install'.\n\n\
             The installation process requires interactive output.\n\
             For automated installations, use the standalone installer:\n\
             https://docs.gitbutler.com/installation"
        );
    }

    let version_request = match target.as_deref() {
        Some("nightly") => VersionRequest::Nightly,
        Some("release") => VersionRequest::Release,
        Some(version_str) => {
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

    if let Some(writer) = out.for_human() {
        let t = crate::theme::get();
        writeln!(
            writer,
            "{} {}",
            t.info.paint("→"),
            t.link
                .paint("View release notes: https://gitbutler.com/releases")
        )?;
        writeln!(writer)?;
    }

    let mut cache = but_ctx::Context::app_cache();
    if let Err(err) = cache.update_check_mut().and_then(|handle| handle.delete()) {
        tracing::warn!(?err, "Failed to invalidate update check cache");
        if let Some(writer) = out.for_human() {
            writeln!(
                writer,
                "Failed to invalidate update check cache - skipping invalidation: {err:?}",
            )?;
        }
    }

    Ok(())
}
