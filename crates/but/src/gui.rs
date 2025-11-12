use anyhow::{Context, Result};

/// Open the GitButler GUI application for `possibly_project_dir`.
///
/// This expects that the GUI application is present and has correctly registered URL
/// schemes for the different channels.
pub fn open(possibly_project_dir: &std::path::Path) -> Result<()> {
    let channel = get_app_channel();
    let absolute_path = std::fs::canonicalize(possibly_project_dir).with_context(|| {
        format!(
            "Failed to canonicalize path: {}",
            possibly_project_dir.display()
        )
    })?;
    channel.open(&absolute_path)?;
    Ok(())
}

fn get_app_channel() -> AppChannel {
    if let Ok(channel) = std::env::var("CHANNEL") {
        match channel.as_str() {
            "nightly" => AppChannel::Nightly,
            "release" => AppChannel::Release,
            _ => AppChannel::Dev,
        }
    } else {
        AppChannel::Dev
    }
}

enum AppChannel {
    Nightly,
    Release,
    Dev,
}

impl AppChannel {
    fn open(&self, possibly_project_dir: &std::path::Path) -> Result<()> {
        let scheme = match self {
            AppChannel::Nightly => "gb-nightly",
            AppChannel::Release => "gb",
            AppChannel::Dev => "gb-dev",
        };

        let url = format!("{}://open?path={}", scheme, possibly_project_dir.display());

        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg("-u")
                .arg(&url)
                .spawn()
                .context("Failed to open URL with 'open' command")?;
        }

        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("xdg-open")
                .arg(&url)
                .spawn()
                .context("Failed to open URL with 'xdg-open' command")?;
        }

        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("cmd")
                .args(["/C", "start", "", &url])
                .spawn()
                .context("Failed to open URL with 'start' command")?;
        }

        Ok(())
    }
}
