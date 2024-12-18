use std::path::PathBuf;

use crate::app_settings::AppSettings;
use crate::json::{json_difference, merge_non_null_json_value};
use anyhow::Result;
use serde_json::json;

static DEFAULTS: &str = include_str!("../assets/defaults.jsonc");

impl AppSettings {
    /// Load the settings from the configuration directory. If a config file name is not provided, the default `gitbutler_settings.json` one is used.
    pub fn load(config_path: PathBuf) -> Result<Self> {
        // Load the defaults
        let mut settings: serde_json::Value = serde_json_lenient::from_str(DEFAULTS)?;

        // If the file on config_path does not exist, create it empty
        if !config_path.exists() {
            gitbutler_fs::write(config_path.clone(), "{}\n")?;
        }
        // Load customizations
        let customizations = serde_json_lenient::from_str(&std::fs::read_to_string(config_path)?)?;

        // Merge the customizations into the settings
        merge_non_null_json_value(customizations, &mut settings);
        Ok(serde_json::from_value(settings)?)
    }

    /// Save the updated fields of the AppSettings in the custom configuration file.
    pub fn save(&self, config_path: PathBuf) -> Result<()> {
        // Load the current settings
        let current = serde_json::to_value(AppSettings::load(config_path.clone())?)?;

        // Derive changed values only compared to the current settings
        let update = serde_json::to_value(self)?;
        let diff = json_difference(current, &update);

        // If there are no changes, do nothing
        if diff == json!({}) {
            return Ok(());
        }

        // Load the existing customizations only
        let mut customizations =
            serde_json_lenient::from_str(&std::fs::read_to_string(config_path.clone())?)?;

        // Merge the new customizations into the existing ones
        // TODO: This will nuke any comments in the file
        merge_non_null_json_value(diff, &mut customizations);
        gitbutler_fs::write(config_path, customizations.to_string())?;
        Ok(())
    }
}

mod tests {
    #[test]
    fn ensure_default_settings_covers_all_fields() {
        let settings: serde_json::Value =
            serde_json_lenient::from_str(crate::persistence::DEFAULTS).unwrap();
        let app_settings: Result<super::AppSettings, serde_json::Error> =
            serde_json::from_value(settings.clone());
        if app_settings.is_err() {
            println!("\n===========================================================================================");
            println!("Not all AppSettings have default values.");
            println!("Make sure to update the defaults file in 'crates/gitbutler-settings/assets/defaults.jsonc'.");
            println!("===========================================================================================\n");
        }
        assert!(app_settings.is_ok())
    }
}
