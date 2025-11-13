use std::{env, path::PathBuf};

use anyhow::bail;

pub fn app_data_dir() -> anyhow::Result<PathBuf> {
    if let Ok(test_dir) = std::env::var("E2E_TEST_APP_DATA_DIR") {
        return Ok(PathBuf::from(test_dir).join("com.gitbutler.app"));
    }
    dirs::data_dir()
        .ok_or(anyhow::anyhow!("Could not get app data dir"))
        .map(|dir| dir.join(identifier()))
}

pub fn app_config_dir() -> anyhow::Result<PathBuf> {
    if let Ok(test_dir) = std::env::var("E2E_TEST_APP_DATA_DIR") {
        return Ok(PathBuf::from(test_dir).join("gitbutler"));
    }
    dirs::config_dir()
        .ok_or(anyhow::anyhow!("Could not get app data dir"))
        .map(|dir| dir.join("gitbutler"))
}

pub fn identifier() -> &'static str {
    option_env!("IDENTIFIER").unwrap_or_else(|| {
        if let Some(channel) = option_env!("CHANNEL") {
            match channel {
                "nightly" => "com.gitbutler.app.nightly",
                "release" => "com.gitbutler.app",
                _ => "com.gitbutler.app.dev",
            }
        } else {
            "com.gitbutler.app.dev"
        }
    })
}

#[derive(Debug)]
pub enum AppChannel {
    Nightly,
    Release,
    Dev,
}

impl Default for AppChannel {
    fn default() -> Self {
        AppChannel::new()
    }
}

impl AppChannel {
    pub fn new() -> Self {
        match identifier() {
            "com.gitbutler.app.nightly" => AppChannel::Nightly,
            "com.gitbutler.app" => AppChannel::Release,
            _ => AppChannel::Dev,
        }
    }

    /// Open the GitButler GUI application for `possibly_project_dir`.
    ///
    /// This uses the deeplink URL scheme registered for the specific channel.
    pub fn open(&self, possibly_project_dir: &std::path::Path) -> anyhow::Result<()> {
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
