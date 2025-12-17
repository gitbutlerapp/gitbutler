use std::path::Path;

use anyhow::Result;
use serde_json::json;
use serde_json_lenient::to_string_pretty;

use crate::{
    AppSettings,
    json::{json_difference, merge_non_null_json_value},
    watch::SETTINGS_FILE,
};

pub(crate) static DEFAULTS: &str = include_str!("../assets/defaults.jsonc");

impl AppSettings {
    /// Load the settings from the configuration directory, or initialize the file with an empty JSON object at `config_path`.
    /// Finally, merge all customizations from `config_path` into the default settings.
    ///
    /// Use `customization` to alter any of the built-in defaults based on other requirements that are application or distribution defined.
    /// This is the only way to alter the defaults without writing these customizations back to disk.
    pub fn load(config_path: &Path, customization: Option<serde_json::Value>) -> Result<Self> {
        // If the file on config_path does not exist, create it empty
        if !config_path.exists() {
            but_fs::create_dirs_then_write(config_path, "{}\n")?;
        }

        // merge customizations from disk into the defaults to get a complete set of settings.
        let customizations = serde_json_lenient::from_str(&std::fs::read_to_string(config_path)?)?;
        let mut settings: serde_json::Value = serde_json_lenient::from_str(DEFAULTS)?;

        merge_non_null_json_value(customizations, &mut settings);
        if let Some(extra) = customization {
            merge_non_null_json_value(extra, &mut settings);
        }
        Ok(serde_json::from_value(settings)?)
    }

    pub fn load_from_default_path_creating_without_customization() -> Result<Self> {
        let config_dir = but_path::app_config_dir()?;
        std::fs::create_dir_all(&config_dir).expect("failed to create config dir");
        AppSettings::load(config_dir.join(SETTINGS_FILE).as_path(), None)
    }

    /// Save all value in this instance to the custom configuration file *if they differ* from the defaults.
    pub fn save(&self, config_path: &Path, customization: Option<serde_json::Value>) -> Result<()> {
        // Load the current settings
        let current = serde_json::to_value(AppSettings::load(config_path, customization)?)?;

        // Derive changed values only compared to the current settings
        let update = serde_json::to_value(self)?;
        let diff = json_difference(current, &update);

        // If there are no changes, do nothing
        if diff == json!({}) {
            return Ok(());
        }

        // Load the existing customizations only
        let mut customizations =
            serde_json_lenient::from_str(&std::fs::read_to_string(config_path)?)?;

        // Merge the new customizations into the existing ones
        // TODO: This will nuke any comments in the file
        merge_non_null_json_value(diff, &mut customizations);
        but_fs::write(config_path, to_string_pretty(&customizations)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ensure_default_settings_covers_all_fields() {
        let settings: serde_json::Value =
            serde_json_lenient::from_str(crate::persistence::DEFAULTS).unwrap();
        let app_settings: Result<super::AppSettings, serde_json::Error> =
            serde_json::from_value(settings.clone());
        if app_settings.is_err() {
            println!(
                "\n==========================================================================================="
            );
            println!("Not all AppSettings have default values.");
            println!(
                "Make sure to update the defaults file in 'crates/gitbutler-settings/assets/defaults.jsonc'."
            );
            println!(
                "===========================================================================================\n"
            );
        }
        assert!(app_settings.is_ok())
    }
}
