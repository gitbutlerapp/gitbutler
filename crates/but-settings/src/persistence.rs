use std::path::Path;

use crate::AppSettings;
use crate::json::{json_difference, merge_non_null_json_value};
use crate::watch::SETTINGS_FILE;
use anyhow::Result;
use serde_json::json;
use serde_json_lenient::to_string_pretty;

pub(crate) static DEFAULTS: &str = include_str!("../assets/defaults.jsonc");

impl AppSettings {
    /// Load the settings from the configuration directory, or initialize the file with an empty JSON object at `config_path`.
    /// Finally, merge all customizations from `config_path` into the default settings.
    pub fn load(config_path: &Path) -> Result<Self> {
        // If the file on config_path does not exist, create it empty
        if !config_path.exists() {
            gitbutler_fs::create_dirs_then_write(config_path, "{}\n")?;
        }

        // merge customizations from disk into the defaults to get a complete set of settings.
        let customizations = serde_json_lenient::from_str(&std::fs::read_to_string(config_path)?)?;
        let mut settings: serde_json::Value = serde_json_lenient::from_str(DEFAULTS)?;

        merge_non_null_json_value(customizations, &mut settings);
        Ok(serde_json::from_value(settings)?)
    }

    pub fn load_from_default_path_creating() -> Result<Self> {
        let config_dir = but_path::app_config_dir()?;
        std::fs::create_dir_all(&config_dir).expect("failed to create config dir");
        AppSettings::load(config_dir.join(SETTINGS_FILE).as_path())
    }

    /// Save all value in this instance to the custom configuration file *if they differ* from the defaults.
    pub fn save(&self, config_path: &Path) -> Result<()> {
        // Load the current settings
        let current = serde_json::to_value(AppSettings::load(config_path)?)?;

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
        gitbutler_fs::write(config_path, to_string_pretty(&customizations)?)?;
        Ok(())
    }
}

mod tests {
    use crate::app_settings::FeatureFlags;

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

    #[test]
    fn ensure_feature_flags_json_defaults_match_struct_defaults() {
        // Get the feature flags from the JSON defaults file
        let json_defaults: serde_json::Value =
            serde_json_lenient::from_str(crate::persistence::DEFAULTS).unwrap();
        let json_feature_flags = json_defaults
            .get("featureFlags")
            .expect("featureFlags should exist in defaults.jsonc");

        // Create what the struct would look like with proper serde defaults
        // Based on the struct definition, most fields are false except ws3 which has #[serde(default = "default_true")] 
        let expected_default_feature_flags = FeatureFlags {
            ws3: true,  // This has #[serde(default = "default_true")] so it should be true
            cv3: false,
            undo: false,
            actions: false,
            butbot: false,
            rules: false,
            single_branch: false,
        };

        // Serialize the expected default struct to JSON for comparison
        let expected_default_as_json: serde_json::Value = serde_json::to_value(&expected_default_feature_flags)
            .expect("Expected default FeatureFlags should be serializable");

        // Compare the JSON values directly - this will catch extra fields like 'v3' that don't exist in the struct
        if *json_feature_flags != expected_default_as_json {
            println!("\n===========================================================================================");
            println!("FeatureFlags JSON defaults don't match the default struct serialization!");
            println!();
            println!("JSON from defaults.jsonc:");
            println!("{}", serde_json::to_string_pretty(json_feature_flags).unwrap());
            println!();
            println!("Expected default struct serialized:");
            println!("{}", serde_json::to_string_pretty(&expected_default_as_json).unwrap());
            println!();
            println!("This means the JSON defaults contain fields or values that don't match");
            println!("the FeatureFlags struct definition. Please update either:");
            println!("1. The defaults in 'crates/but-settings/assets/defaults.jsonc' (remove extra fields, fix values)");
            println!("2. The FeatureFlags struct in 'crates/but-settings/src/app_settings.rs' (add missing fields)");
            println!("===========================================================================================\n");
        }

        assert_eq!(
            *json_feature_flags, expected_default_as_json,
            "FeatureFlags JSON defaults should match the expected default struct when serialized"
        );
    }
}
