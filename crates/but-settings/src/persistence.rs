use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::json;
use serde_json_lenient::to_string_pretty;

use crate::{
    AppSettings,
    json::{json_difference, merge_json_value},
    legacy_settings::maybe_migrate_legacy_settings,
    watch::SETTINGS_FILE,
};

pub(crate) static DEFAULTS: &str = include_str!("../assets/defaults.jsonc");

fn remove_deprecated_settings(customizations: &mut serde_json::Value) -> bool {
    let Some(root) = customizations.as_object_mut() else {
        return false;
    };
    let mut removed = false;

    if let Some(feature_flags) = root
        .get_mut("featureFlags")
        .and_then(serde_json::Value::as_object_mut)
    {
        for deprecated in ["apply3", "unapplyV3", "undo", "rules"] {
            if feature_flags.remove(deprecated).is_some() {
                removed = true;
            }
        }
        if feature_flags.is_empty() {
            root.remove("featureFlags");
        }
    }

    if let Some(telemetry) = root
        .get_mut("telemetry")
        .and_then(serde_json::Value::as_object_mut)
    {
        if telemetry.remove("appNonAnonMetricsEnabled").is_some() {
            removed = true;
        }
        if telemetry.is_empty() {
            root.remove("telemetry");
        }
    }

    removed
}

impl AppSettings {
    /// Load the settings from the configuration directory, or initialize the file with an empty JSON object at `config_path`.
    /// Finally, merge all customizations from `config_path` into the default settings.
    ///
    /// Use `customization` to alter any of the built-in defaults based on other requirements that are application or distribution defined.
    /// This is the only way to alter the defaults without writing these customizations back to disk.
    pub fn load(config_path: &Path, customization: Option<serde_json::Value>) -> Result<Self> {
        // If the file on config_path does not exist, create it empty
        if !config_path.exists() {
            but_utils::create_dirs_then_write(config_path, "{}\n")?;
        }

        // merge customizations from disk into the defaults to get a complete set of settings.
        let config_file_contents = std::fs::read_to_string(config_path).with_context(|| {
            format!(
                "failed to read settings file at '{}'",
                config_path.display()
            )
        })?;
        let customizations = serde_json_lenient::from_str::<serde_json::Value>(
            &config_file_contents,
        )
        .with_context(|| {
            format!(
                "failed to parse settings file at '{}' as JSON",
                config_path.display()
            )
        })?;
        let mut settings = serde_json_lenient::from_str::<serde_json::Value>(DEFAULTS)?;

        merge_json_value(customizations, &mut settings);
        if let Some(extra) = customization {
            merge_json_value(extra, &mut settings);
        }

        // Migrate legacy settings from Tauri store (only if not already migrated)
        if let Some(legacy_overrides) = maybe_migrate_legacy_settings(config_path, &settings) {
            // At this point, `maybe_migrate_legacy_settings` has attempted to write the `customizations`
            // as freshly read from `config_path` back to `config_path`, after merging them with the legacy settings.
            // We now repeat this merging step with the in-memory settings to bring it up-to date with those overrides.
            merge_json_value(legacy_overrides, &mut settings);
        }

        Ok(serde_json::from_value(settings)?)
    }

    /// Load the default settings, and creating the directory if needed.
    pub fn load_from_default_path_creating_without_customization() -> Result<Self> {
        let config_dir = but_path::app_config_dir()?;
        AppSettings::load(&Self::default_settings_path(&config_dir), None)
    }

    /// Return where the settings file would be placed in `config_dir`.
    pub fn default_settings_path(config_dir: &Path) -> PathBuf {
        config_dir.join(SETTINGS_FILE)
    }

    /// Save all value in this instance to the custom configuration file *if they differ* from the defaults.
    pub fn save(&self, config_path: &Path, customization: Option<serde_json::Value>) -> Result<()> {
        // Load the current settings
        let current = serde_json::to_value(AppSettings::load(config_path, customization)?)?;

        // Derive changed values only compared to the current settings
        let update = serde_json::to_value(self)?;
        let diff = json_difference(current, &update);

        // Load the existing customizations only
        let mut customizations =
            serde_json_lenient::from_str(&std::fs::read_to_string(config_path)?)?;
        let removed_deprecated_settings = remove_deprecated_settings(&mut customizations);

        // If there are no changes and no deprecated settings to clean up, do nothing
        if diff == json!({}) && !removed_deprecated_settings {
            return Ok(());
        }

        // Merge the new customizations into the existing ones
        // TODO: This will nuke any comments in the file
        merge_json_value(diff, &mut customizations);
        but_utils::write(config_path, to_string_pretty(&customizations)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::TempDir;

    use super::*;

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

    fn create_test_env() -> (TempDir, PathBuf, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("settings.json");
        let legacy_path = temp_dir.path().join("legacy-settings.json");
        (temp_dir, config_path, legacy_path)
    }

    #[test]
    fn migration_flag_prevents_repeated_migration() {
        let (_temp_dir, config_path, _legacy_path) = create_test_env();

        // Set up config with migration already completed
        std::fs::write(
            &config_path,
            r#"{
                "telemetry": {
                    "migratedFromLegacy": true,
                    "appMetricsEnabled": false
                }
            }"#,
        )
        .unwrap();

        // Load settings - migration should be skipped because flag is true
        let settings = AppSettings::load(&config_path, None).unwrap();

        // Verify the flag is set to true
        assert!(
            settings.telemetry.migrated_from_legacy,
            "Migration flag should be true after migration"
        );

        // Verify the custom setting is preserved (not overwritten by migration)
        assert!(
            !settings.telemetry.app_metrics_enabled,
            "Custom settings should be preserved when migration is skipped"
        );
    }

    #[test]
    fn save_prunes_deprecated_feature_flags() {
        let (_temp_dir, config_path, _legacy_path) = create_test_env();

        std::fs::write(
            &config_path,
            r#"{
                "telemetry": {
                    "migratedFromLegacy": true
                },
                "featureFlags": {
                    "apply3": true,
                    "unapplyV3": false,
                    "undo": true,
                    "rules": true,
                    "cv3": true
                }
            }"#,
        )
        .unwrap();

        let settings = AppSettings::load(&config_path, None).unwrap();
        settings.save(&config_path, None).unwrap();

        let saved: serde_json::Value =
            serde_json_lenient::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();

        assert_eq!(saved["featureFlags"]["cv3"], json!(true));
        assert_eq!(saved["featureFlags"].get("apply3"), None);
        assert_eq!(saved["featureFlags"].get("unapplyV3"), None);
        assert_eq!(saved["featureFlags"].get("undo"), None);
        assert_eq!(saved["featureFlags"].get("rules"), None);
    }

    #[test]
    fn save_prunes_deprecated_non_anon_metrics_flag() {
        let (_temp_dir, config_path, _legacy_path) = create_test_env();

        std::fs::write(
            &config_path,
            r#"{
                "telemetry": {
                    "migratedFromLegacy": true,
                    "appNonAnonMetricsEnabled": true,
                    "appMetricsEnabled": false
                }
            }"#,
        )
        .unwrap();

        let settings = AppSettings::load(&config_path, None).unwrap();
        settings.save(&config_path, None).unwrap();

        let saved: serde_json::Value =
            serde_json_lenient::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();

        assert_eq!(saved["telemetry"]["appMetricsEnabled"], json!(false));
        assert_eq!(saved["telemetry"].get("appNonAnonMetricsEnabled"), None);
    }
}
