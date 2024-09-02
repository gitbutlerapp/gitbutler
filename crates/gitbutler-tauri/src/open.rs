use crate::error::Error;
use anyhow::anyhow;
use regex::Regex;
use std::env;
use tracing::instrument;

pub(crate) fn open_that(path: &str) -> Result<(), Error> {
    let re = Regex::new(r"^((https://)|(http://)|(mailto:)|(vscode://)|(vscodium://)).+").unwrap();
    if !re.is_match(path) {
        return Err(anyhow!("Invalid path format").into());
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
                        .filter(|path| {
                            !path.contains("appimage-run") && !path.contains("/tmp/.mount")
                        })
                        .collect::<Vec<_>>()
                        .join(":"),
                )
            })
    }

    std::thread::spawn({
        let path = path.to_string();

        move || {
            for mut cmd in open::commands(&path) {
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
                    break;
                };
            }
        }
    });
    Ok(())
}

#[tauri::command()]
#[instrument(err(Debug))]
pub fn open_url(url: &str) -> Result<(), Error> {
    open_that(url)
}
