use crate::error::Error;
use anyhow::anyhow;
use regex::Regex;
use std::collections::HashMap;
use std::env;
use tracing::instrument;

pub fn open_that(path: &str) -> Result<(), Error> {
    let re = Regex::new(r"^((https://)|(http://)|(mailto:)|(vscode://)|(vscodium://)).+").unwrap();
    if !re.is_match(&path) {
        return Err(anyhow!("Invalid path format").into());
    }

    fn clean_env_vars(var_names: &[&str]) {
        for var_name in var_names {
            if let Ok(var_value) = env::var(var_name) {
                let cleaned_value = var_value
                    .split(':')
                    .filter(|path| !path.contains("appimage-run"))
                    .collect::<Vec<_>>()
                    .join(":");
                env::set_var(var_name, cleaned_value);
            }
        }
    }

    std::thread::spawn({
        let path = path.to_string();
        move || {
            clean_env_vars(&[
                "XDG_DATA_DIRS",
                "GTK_PATH",
                "GTK_EXE_PREFIX",
                "APPDIR",
                "LD_LIBRARY_PATH",
                "GIO_EXTRA_MODULES",
                "PATH",
                "PYTHONHOME",
                "PYTHONPATH",
                "PERLLIB",
                "QT_PLUGIN_PATH",
                "GSETTINGS_SCHEMA_DIR",
                "GST_PLUGIN_SYSTEM_PATH",
                "GST_PLUGIN_SYSTEM_PATH_1_0",
                "GTK_DATA_PREFIX",
                "GDK_PIXBUF_MODULE_FILE",
                "GTK_IM_MODULE_FILE",
            ]);

            for mut cmd in open::commands(&path) {
                // println!("{:#?}", env::vars());
                // cmd.env_clear();
                // println!("{:#?}", filtered_out_env);
                // cmd.envs(&filtered_out_env);

                println!("{:#?}", env::vars());
                cmd.envs(env::vars());

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
    open_that(url).unwrap();
    Ok(())
}
