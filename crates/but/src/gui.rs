use std::env;

use anyhow::{Context, Result, bail};

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
            AppChannel::Nightly => "but-nightly",
            AppChannel::Release => "but",
            AppChannel::Dev => "but-dev",
        };

        let url = format!("{}://open?path={}", scheme, possibly_project_dir.display());

        let mut cmd_errors = Vec::new();

        for mut cmd in open::commands(url) {
            let cleaned_vars = clean_env_vars(&[
                "APPDIR",
                "GDK_PIXBUF_MODULE_FILE",
                "GIO_EXTRA_MODULES",
                "GIO_EXTRA_MODULES",
                "GSETTINGS_SCHEMA_DIR",
                "GST_PLUGIN_SYSTEM_PATH",
                "GST_PLUGIN_SYSTEM_PATH_1_0",
                "GTK_DATA_PREFIX",
                "GTK_EXE_PREFIX",
                "GTK_IM_MODULE_FILE",
                "GTK_PATH",
                "LD_LIBRARY_PATH",
                "PATH",
                "PERLLIB",
                "PYTHONHOME",
                "PYTHONPATH",
                "QT_PLUGIN_PATH",
                "XDG_DATA_DIRS",
            ]);

            cmd.envs(cleaned_vars);
            cmd.current_dir(env::temp_dir());
            if cmd.status().is_ok() {
                return Ok(());
            } else {
                cmd_errors.push(anyhow::anyhow!("Failed to execute command {:?}", cmd));
            }
        }
        if !cmd_errors.is_empty() {
            bail!("Errors occurred: {:?}", cmd_errors);
        }
        Ok(())
    }
}

fn clean_env_vars<'a, 'b>(
    var_names: &'a [&'b str],
) -> impl Iterator<Item = (&'b str, String)> + 'a {
    var_names
        .iter()
        .filter_map(|name| env::var(name).map(|value| (*name, value)).ok())
        .map(|(name, value)| {
            (
                name,
                value
                    .split(':')
                    .filter(|path| !path.contains("appimage-run") && !path.contains("/tmp/.mount"))
                    .collect::<Vec<_>>()
                    .join(":"),
            )
        })
}
