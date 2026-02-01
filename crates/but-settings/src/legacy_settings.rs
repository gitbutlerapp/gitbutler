//! Legacy settings migration.
//!
//! This module handles one-time migration of settings from the old legacy
//! storage format to the new settings system.
//!
//! This module is temporary migration code and can be removed once we no longer
//! need to support reading settings from the legacy Tauri store format and all
//! supported application versions have shipped with the new settings system.

use std::path::Path;

use anyhow::Result;
use serde_json::json;

use crate::json::merge_json_value;

/// Parse legacy settings JSON and map to new settings format.
/// This function is pure and testable - it only does the transformation.
///
/// Always returns overrides with at least the `migratedFromLegacy` flag set to true,
/// ensuring the migration is marked as complete even if no other fields were migrated.
fn parse_legacy_settings_to_overrides(legacy_json: &serde_json::Value) -> serde_json::Value {
    let set_bool_if_exists = |from_key: &str, to_json: &mut serde_json::Value, to_key: &str| {
        if let Some(v) = legacy_json.get(from_key).and_then(|v| v.as_bool()) {
            to_json[to_key] = json!(v);
        }
    };

    let mut overrides = json!({});
    let mut telemetry = json!({});

    set_bool_if_exists(
        "appAnalyticsConfirmed",
        &mut overrides,
        "onboardingComplete",
    );
    set_bool_if_exists("appMetricsEnabled", &mut telemetry, "appMetricsEnabled");
    set_bool_if_exists(
        "appNonAnonMetricsEnabled",
        &mut telemetry,
        "appNonAnonMetricsEnabled",
    );
    set_bool_if_exists(
        "appErrorReportingEnabled",
        &mut telemetry,
        "appErrorReportingEnabled",
    );

    if let Some(telemetry_obj) = telemetry.as_object()
        && !telemetry_obj.is_empty()
    {
        overrides["telemetry"] = telemetry;
    }

    // Always mark that migration has been completed, even if no fields were migrated
    // This ensures the migration only runs once per legacy file
    merge_json_value(
        json!({"telemetry": {"migratedFromLegacy": true}}),
        &mut overrides,
    );

    // Always return overrides with at least the migration flag
    // Even if the legacy file had no mappable fields, we need to persist the flag
    overrides
}

/// Read and parse legacy settings from a specific path.
/// Returns parsed overrides (with migratedFromLegacy flag) if the file exists and is valid, None otherwise.
fn read_legacy_overrides(legacy_store_path: &Path) -> Option<serde_json::Value> {
    if !legacy_store_path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(legacy_store_path).ok()?;
    let legacy_json: serde_json::Value = serde_json_lenient::from_str(&content).ok()?;
    Some(parse_legacy_settings_to_overrides(&legacy_json))
}

/// Persist legacy overrides to disk by merging them with existing customizations.
/// Reads the current config file, merges the legacy overrides into it, and writes back only if changed.
fn maybe_persist_overrides(config_path: &Path, legacy_overrides: serde_json::Value) -> Result<()> {
    // Read current customizations from disk
    let current_customizations: serde_json::Value =
        serde_json_lenient::from_str(&std::fs::read_to_string(config_path)?)?;

    // Merge legacy overrides into customizations
    let mut customizations_with_overrides = current_customizations.clone();
    merge_json_value(legacy_overrides, &mut customizations_with_overrides);

    // Only write if the merged result differs from current
    let diff = crate::json::json_difference(current_customizations, &customizations_with_overrides);

    if let Some(diff_obj) = diff.as_object()
        && !diff_obj.is_empty()
    {
        but_fs::write(
            config_path,
            serde_json_lenient::to_string_pretty(&customizations_with_overrides)?,
        )?;
    }

    Ok(())
}

/// Perform one-time migration of legacy Tauri store settings.
///
/// This function encapsulates the entire migration process:
/// - Checks if migration has already been completed
/// - Reads legacy settings from the app data directory
/// - Returns overrides to be merged into settings
/// - Persists the migration flag to prevent re-running
///
/// Returns `Some(overrides)` if migration should be applied, `None` if already completed or no legacy file exists.
pub(crate) fn maybe_migrate_legacy_settings(
    config_path: &Path,
    current_settings: &serde_json::Value,
) -> Option<serde_json::Value> {
    // Check if migration has already been completed
    let already_migrated = current_settings
        .get("telemetry")
        .and_then(|t| t.get("migratedFromLegacy"))
        .and_then(|m| m.as_bool())
        .unwrap_or(false);

    if already_migrated {
        return None;
    }

    // Try to read legacy settings from the standard app data directory
    let legacy_store_path = but_path::app_data_dir().ok()?.join("settings.json");
    let legacy_overrides = read_legacy_overrides(&legacy_store_path)?;

    // Persist the legacy overrides to disk
    if let Err(err) = maybe_persist_overrides(config_path, legacy_overrides.clone()) {
        tracing::error!("Failed to persist legacy settings overrides: {err:#}");
    }

    Some(legacy_overrides)
}

#[cfg(test)]
mod parse_legacy_settings_to_overrides {
    use serde_json::json;

    use crate::legacy_settings::parse_legacy_settings_to_overrides;

    #[test]
    fn parse_all_legacy_fields() -> anyhow::Result<()> {
        let legacy_json = json!({
            "appAnalyticsConfirmed": true,
            "appMetricsEnabled": false,
            "appNonAnonMetricsEnabled": true,
            "appErrorReportingEnabled": false
        });

        let result = parse_legacy_settings_to_overrides(&legacy_json);

        assert_eq!(result["onboardingComplete"], json!(true));
        assert_eq!(result["telemetry"]["appMetricsEnabled"], json!(false));
        assert_eq!(result["telemetry"]["appNonAnonMetricsEnabled"], json!(true));
        assert_eq!(
            result["telemetry"]["appErrorReportingEnabled"],
            json!(false)
        );
        Ok(())
    }

    #[test]
    fn parse_only_onboarding_field() -> anyhow::Result<()> {
        let legacy_json = json!({
            "appAnalyticsConfirmed": true
        });

        let result = parse_legacy_settings_to_overrides(&legacy_json);

        assert_eq!(result["onboardingComplete"], json!(true));
        // Migration flag is always present to mark that migration occurred
        assert_eq!(result["telemetry"]["migratedFromLegacy"], json!(true));
        Ok(())
    }

    #[test]
    fn parse_only_telemetry_fields() -> anyhow::Result<()> {
        let legacy_json = json!({
            "appMetricsEnabled": false,
            "appErrorReportingEnabled": false
        });

        let result = parse_legacy_settings_to_overrides(&legacy_json);

        assert!(result.get("onboardingComplete").is_none());
        assert_eq!(result["telemetry"]["appMetricsEnabled"], json!(false));
        assert_eq!(
            result["telemetry"]["appErrorReportingEnabled"],
            json!(false)
        );
        Ok(())
    }

    #[test]
    fn parse_ignores_extra_fields() -> anyhow::Result<()> {
        let legacy_json = json!({
            "appAnalyticsConfirmed": true,
            "someRandomField": "ignored",
            "anotherField": 123
        });

        let result = parse_legacy_settings_to_overrides(&legacy_json);

        assert_eq!(result["onboardingComplete"], json!(true));
        assert!(result.get("someRandomField").is_none());
        assert!(result.get("anotherField").is_none());
        Ok(())
    }

    #[test]
    fn parse_ignores_wrong_types() -> anyhow::Result<()> {
        let legacy_json = json!({
            "appAnalyticsConfirmed": "not a boolean",
            "appMetricsEnabled": 123,
            "appNonAnonMetricsEnabled": null
        });

        let result = parse_legacy_settings_to_overrides(&legacy_json);

        // Even with invalid data, we return the migration flag to prevent re-running migration
        assert_eq!(result["telemetry"]["migratedFromLegacy"], json!(true));
        // No other fields should be present since all values were invalid
        assert!(result.get("onboardingComplete").is_none());
        assert_eq!(result["telemetry"].as_object().unwrap().len(), 1); // Only migratedFromLegacy
        Ok(())
    }

    #[test]
    fn parse_empty_json() -> anyhow::Result<()> {
        let legacy_json = json!({});

        let result = parse_legacy_settings_to_overrides(&legacy_json);

        // Even with empty legacy file, we return the migration flag to prevent re-running migration
        assert_eq!(result["telemetry"]["migratedFromLegacy"], json!(true));
        // No other fields should be present
        assert!(result.get("onboardingComplete").is_none());
        assert_eq!(result["telemetry"].as_object().unwrap().len(), 1); // Only migratedFromLegacy
        Ok(())
    }

    #[test]
    fn parse_onboarding_false() -> anyhow::Result<()> {
        let legacy_json = json!({
            "appAnalyticsConfirmed": false
        });

        let result = parse_legacy_settings_to_overrides(&legacy_json);

        assert_eq!(result["onboardingComplete"], json!(false));
        // Migration flag is always present to mark that migration occurred
        assert_eq!(result["telemetry"]["migratedFromLegacy"], json!(true));
        Ok(())
    }

    #[test]
    fn parse_user_example_one() -> anyhow::Result<()> {
        // User-provided example with all fields set to true
        let legacy_json = json!({
            "appErrorReportingEnabled": true,
            "appMetricsEnabled": true,
            "appNonAnonMetricsEnabled": true,
            "appAnalyticsConfirmed": true
        });

        let result = parse_legacy_settings_to_overrides(&legacy_json);

        assert_eq!(result["onboardingComplete"], json!(true));
        assert_eq!(result["telemetry"]["appMetricsEnabled"], json!(true));
        assert_eq!(result["telemetry"]["appNonAnonMetricsEnabled"], json!(true));
        assert_eq!(result["telemetry"]["appErrorReportingEnabled"], json!(true));
        Ok(())
    }

    #[test]
    fn parse_user_example_two() -> anyhow::Result<()> {
        // User-provided example with subset of fields
        let legacy_json = json!({
            "appAnalyticsConfirmed": true,
            "appErrorReportingEnabled": true,
            "appMetricsEnabled": true
        });

        let result = parse_legacy_settings_to_overrides(&legacy_json);

        assert_eq!(result["onboardingComplete"], json!(true));
        assert_eq!(result["telemetry"]["appMetricsEnabled"], json!(true));
        assert_eq!(result["telemetry"]["appErrorReportingEnabled"], json!(true));
        // appNonAnonMetricsEnabled should not be in the result
        assert!(
            result["telemetry"]
                .get("appNonAnonMetricsEnabled")
                .is_none()
        );
        Ok(())
    }
}

#[cfg(test)]
mod read_legacy_overrides {
    use serde_json::json;
    use tempfile::TempDir;

    use crate::legacy_settings::read_legacy_overrides;

    #[test]
    fn read_from_nonexistent_path() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().join("nonexistent-settings.json");
        let result = read_legacy_overrides(&path);
        assert!(result.is_none());
        Ok(())
    }

    #[test]
    fn read_from_valid_legacy_file() -> anyhow::Result<()> {
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new()?;
        let path = temp_file.path();

        // Write valid legacy settings
        let legacy_settings = json!({
            "appAnalyticsConfirmed": true,
            "appMetricsEnabled": false,
            "appErrorReportingEnabled": true
        });
        std::fs::write(path, serde_json::to_string_pretty(&legacy_settings)?)?;

        let result = read_legacy_overrides(path).unwrap();

        assert_eq!(result["onboardingComplete"], json!(true));
        assert_eq!(result["telemetry"]["appMetricsEnabled"], json!(false));
        assert_eq!(result["telemetry"]["appErrorReportingEnabled"], json!(true));
        Ok(())
    }
}

#[cfg(test)]
mod maybe_persist_overrides {
    use serde_json::json;

    use crate::legacy_settings::{maybe_persist_overrides, read_legacy_overrides};

    #[test]
    fn maybe_persist_writes_when_different() -> anyhow::Result<()> {
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new()?;
        let path = temp_file.path();

        // Write initial customizations
        let original = json!({"telemetry": {"appMetricsEnabled": true}});
        std::fs::write(path, serde_json::to_string_pretty(&original)?)?;

        // Persist overrides that differ
        let overrides = json!({"onboardingComplete": true});
        maybe_persist_overrides(path, overrides)?;

        // Verify file was written
        let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        assert_eq!(content["onboardingComplete"], json!(true));
        assert_eq!(content["telemetry"]["appMetricsEnabled"], json!(true));
        Ok(())
    }

    #[test]
    fn maybe_persist_does_not_write_when_identical() -> anyhow::Result<()> {
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new()?;
        let path = temp_file.path();

        // Write initial customizations that already contain the overrides
        let original = json!({
            "onboardingComplete": true,
            "telemetry": {"appMetricsEnabled": false}
        });
        std::fs::write(path, serde_json::to_string_pretty(&original)?)?;

        // Get modification time before
        let metadata_before = std::fs::metadata(path)?;
        let modified_before = metadata_before.modified()?;

        // Wait a bit to ensure timestamp would change if file was written
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Persist overrides that are already there
        let overrides = json!({"onboardingComplete": true});
        maybe_persist_overrides(path, overrides)?;

        // Verify file was NOT written since values are the same (timestamp unchanged)
        let metadata_after = std::fs::metadata(path)?;
        let modified_after = metadata_after.modified()?;
        assert_eq!(modified_before, modified_after);

        // Verify content is still correct
        let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        assert_eq!(content["onboardingComplete"], json!(true));
        assert_eq!(content["telemetry"]["appMetricsEnabled"], json!(false));
        Ok(())
    }

    #[test]
    fn maybe_persist_deep_merges_nested_telemetry() -> anyhow::Result<()> {
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new()?;
        let path = temp_file.path();

        // Original has some telemetry settings
        let original = json!({
            "telemetry": {"appMetricsEnabled": true}
        });
        std::fs::write(path, serde_json::to_string_pretty(&original)?)?;

        // Overrides have different telemetry settings
        let overrides = json!({
            "telemetry": {"appErrorReportingEnabled": false}
        });
        maybe_persist_overrides(path, overrides)?;

        // Verify both telemetry fields are present (deep merge)
        let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        assert_eq!(content["telemetry"]["appMetricsEnabled"], json!(true));
        assert_eq!(
            content["telemetry"]["appErrorReportingEnabled"],
            json!(false)
        );
        Ok(())
    }

    #[test]
    fn integration_legacy_migration_through_app_settings() -> anyhow::Result<()> {
        use tempfile::TempDir;

        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("settings.json");
        let legacy_path = temp_dir.path().join("legacy-settings.json");

        // Create initial config with some customizations
        let initial_config = json!({
            "telemetry": {"appMetricsEnabled": true}
        });
        std::fs::write(&config_path, serde_json::to_string_pretty(&initial_config)?)?;

        // Create legacy settings to migrate
        let legacy_settings = json!({
            "appAnalyticsConfirmed": true,
            "appErrorReportingEnabled": false
        });
        std::fs::write(
            &legacy_path,
            serde_json::to_string_pretty(&legacy_settings)?,
        )?;

        // Read legacy overrides and persist them
        let legacy_overrides = read_legacy_overrides(&legacy_path).unwrap();
        maybe_persist_overrides(&config_path, legacy_overrides)?;

        // Verify config file now contains both original and migrated settings
        let final_config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config_path)?)?;

        // Original setting preserved
        assert_eq!(final_config["telemetry"]["appMetricsEnabled"], json!(true));
        // Migrated settings added
        assert_eq!(final_config["onboardingComplete"], json!(true));
        assert_eq!(
            final_config["telemetry"]["appErrorReportingEnabled"],
            json!(false)
        );
        Ok(())
    }

    #[test]
    fn legacy_true_values_override_false_in_config() -> anyhow::Result<()> {
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new()?;
        let path = temp_file.path();

        // Config has telemetry settings disabled
        let original = json!({
            "telemetry": {
                "appMetricsEnabled": false,
                "appErrorReportingEnabled": false
            }
        });
        std::fs::write(path, serde_json::to_string_pretty(&original)?)?;

        // Legacy settings have them enabled (true)
        let overrides = json!({
            "telemetry": {
                "appMetricsEnabled": true,
                "appErrorReportingEnabled": true
            }
        });
        maybe_persist_overrides(path, overrides)?;

        // Verify true values overwrote false values
        let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        assert_eq!(content["telemetry"]["appMetricsEnabled"], json!(true));
        assert_eq!(
            content["telemetry"]["appErrorReportingEnabled"],
            json!(true)
        );
        Ok(())
    }

    #[test]
    fn legacy_true_values_are_written_to_empty_config() -> anyhow::Result<()> {
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new()?;
        let path = temp_file.path();

        // Config file is empty (just initialized)
        let original = json!({});
        std::fs::write(path, serde_json::to_string_pretty(&original)?)?;

        // Legacy settings have various true values
        let overrides = json!({
            "onboardingComplete": true,
            "telemetry": {
                "appMetricsEnabled": true,
                "appErrorReportingEnabled": true,
                "appNonAnonMetricsEnabled": true
            }
        });
        maybe_persist_overrides(path, overrides)?;

        // Verify all true values were written
        let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        assert_eq!(content["onboardingComplete"], json!(true));
        assert_eq!(content["telemetry"]["appMetricsEnabled"], json!(true));
        assert_eq!(
            content["telemetry"]["appErrorReportingEnabled"],
            json!(true)
        );
        assert_eq!(
            content["telemetry"]["appNonAnonMetricsEnabled"],
            json!(true)
        );
        Ok(())
    }
    #[test]
    fn maybe_persist_partial_overlap_only_writes_differences() -> anyhow::Result<()> {
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new()?;
        let path = temp_file.path();

        // Config already has some matching values and some different values
        let original = json!({
            "onboardingComplete": true,
            "telemetry": {
                "appMetricsEnabled": false,
                "appErrorReportingEnabled": true
            }
        });
        std::fs::write(path, serde_json::to_string_pretty(&original)?)?;

        // Overrides have: one matching field (onboardingComplete), one different field (appMetricsEnabled),
        // and one new field (appNonAnonMetricsEnabled)
        let overrides = json!({
            "onboardingComplete": true,
            "telemetry": {
                "appMetricsEnabled": true,
                "appNonAnonMetricsEnabled": true
            }
        });
        maybe_persist_overrides(path, overrides)?;

        // Verify file was written with merged result
        let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        assert_eq!(content["onboardingComplete"], json!(true));
        assert_eq!(
            content["telemetry"]["appMetricsEnabled"],
            json!(true),
            "should override false with true"
        );
        assert_eq!(
            content["telemetry"]["appErrorReportingEnabled"],
            json!(true),
            "should preserve existing field not in overrides"
        );
        assert_eq!(
            content["telemetry"]["appNonAnonMetricsEnabled"],
            json!(true),
            "should add new field from overrides"
        );
        Ok(())
    }

    #[test]
    fn maybe_persist_handles_corrupted_config_file() -> anyhow::Result<()> {
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new()?;
        let path = temp_file.path();

        // Write invalid JSON to config file
        std::fs::write(path, "{ this is not valid json }")?;

        let overrides = json!({
            "onboardingComplete": true
        });

        // Should return an error when trying to parse corrupted config
        let result = maybe_persist_overrides(path, overrides);
        assert!(result.is_err(), "should fail to parse corrupted JSON");
        Ok(())
    }

    #[test]
    fn maybe_persist_with_nested_partial_match() -> anyhow::Result<()> {
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new()?;
        let path = temp_file.path();

        // Config has nested structure with some matching and some different values
        let original = json!({
            "onboardingComplete": false,
            "telemetry": {
                "appMetricsEnabled": true,
                "appErrorReportingEnabled": true,
                "appNonAnonMetricsEnabled": false
            }
        });
        std::fs::write(path, serde_json::to_string_pretty(&original)?)?;

        // Overrides change some telemetry fields but not all
        let overrides = json!({
            "onboardingComplete": true,
            "telemetry": {
                "appMetricsEnabled": false,
                "appNonAnonMetricsEnabled": true
            }
        });
        maybe_persist_overrides(path, overrides)?;

        // Verify all fields are correctly merged
        let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        assert_eq!(content["onboardingComplete"], json!(true));
        assert_eq!(content["telemetry"]["appMetricsEnabled"], json!(false));
        assert_eq!(
            content["telemetry"]["appErrorReportingEnabled"],
            json!(true),
            "should preserve field not in overrides"
        );
        assert_eq!(
            content["telemetry"]["appNonAnonMetricsEnabled"],
            json!(true)
        );
        Ok(())
    }

    #[test]
    fn maybe_persist_empty_config_file() -> anyhow::Result<()> {
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new()?;
        let path = temp_file.path();

        // Start with truly empty config (just empty object)
        std::fs::write(path, "{}")?;

        let overrides = json!({
            "onboardingComplete": false,
            "telemetry": {
                "appMetricsEnabled": false
            }
        });
        maybe_persist_overrides(path, overrides)?;

        // Verify all override values were written
        let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        assert_eq!(content["onboardingComplete"], json!(false));
        assert_eq!(content["telemetry"]["appMetricsEnabled"], json!(false));
        Ok(())
    }
}
