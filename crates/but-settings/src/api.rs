use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::AppSettingsWithDiskSync;

#[derive(
    Copy, Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase", default)]
#[schemars(extend("x-input" = true))]
/// Update request for [`crate::app_settings::TelemetrySettings`].
pub struct TelemetryUpdate {
    pub app_metrics_enabled: Option<bool>,
    pub app_error_reporting_enabled: Option<bool>,
}
but_schemars::register_sdk_type!(TelemetryUpdate);

#[derive(
    Copy, Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase", default)]
#[schemars(extend("x-input" = true))]
/// Update request for [`crate::app_settings::FeatureFlags`].
pub struct FeatureFlagsUpdate {
    pub worktree_manipulation: Option<bool>,
    pub gpui: Option<bool>,
}
but_schemars::register_sdk_type!(FeatureFlagsUpdate);

#[derive(
    Copy, Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase", default)]
#[schemars(extend("x-input" = true))]
/// Update request for [`crate::app_settings::Reviews`].
pub struct ReviewsUpdate {
    pub auto_fill_pr_description_from_commit: Option<bool>,
}
but_schemars::register_sdk_type!(ReviewsUpdate);

#[derive(
    Copy, Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase", default)]
#[schemars(extend("x-input" = true))]
/// Update request for [`crate::app_settings::Fetch`].
pub struct FetchUpdate {
    pub auto_fetch_interval_minutes: Option<isize>,
}
but_schemars::register_sdk_type!(FetchUpdate);

#[derive(
    Copy, Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase", default)]
#[schemars(extend("x-input" = true))]
/// Update request for [`crate::app_settings::UiSettings`].
pub struct UiUpdate {
    pub use_native_title_bar: Option<bool>,
    pub no_shadow: Option<bool>,
    // Note that the CLI related information cannot be set - it's set at compile time.
}
but_schemars::register_sdk_type!(UiUpdate);

/// Mutation, immediately followed by writing everything to disk.
impl AppSettingsWithDiskSync {
    pub fn update_onboarding_complete(&self, update: bool) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        settings.onboarding_complete = update;
        settings.save()
    }

    pub fn update_telemetry(&self, update: TelemetryUpdate) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        if let Some(app_metrics_enabled) = update.app_metrics_enabled {
            settings.telemetry.app_metrics_enabled = app_metrics_enabled;
        }
        if let Some(app_error_reporting_enabled) = update.app_error_reporting_enabled {
            settings.telemetry.app_error_reporting_enabled = app_error_reporting_enabled;
        }
        settings.save()
    }

    pub fn update_telemetry_distinct_id(&self, app_distinct_id: Option<String>) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        settings.telemetry.app_distinct_id = app_distinct_id;
        settings.save()
    }

    pub fn update_feature_flags(
        &self,
        FeatureFlagsUpdate {
            worktree_manipulation,
            gpui,
        }: FeatureFlagsUpdate,
    ) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        if let Some(worktree_manipulation) = worktree_manipulation {
            settings.feature_flags.worktree_manipulation = worktree_manipulation;
        }
        if let Some(gpui) = gpui {
            settings.feature_flags.gpui = gpui;
        }
        settings.save()
    }

    pub fn update_reviews(&self, update: ReviewsUpdate) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        if let Some(auto_fill_pr_description_from_commit) =
            update.auto_fill_pr_description_from_commit
        {
            settings.reviews.auto_fill_pr_description_from_commit =
                auto_fill_pr_description_from_commit;
        }
        settings.save()
    }

    pub fn update_fetch(&self, update: FetchUpdate) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        if let Some(auto_fetch_interval_minutes) = update.auto_fetch_interval_minutes {
            settings.fetch.auto_fetch_interval_minutes = auto_fetch_interval_minutes;
        }
        settings.save()
    }

    pub fn update_ui(&self, update: UiUpdate) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        if let Some(use_native_title_bar) = update.use_native_title_bar {
            settings.ui.use_native_title_bar = use_native_title_bar;
        }
        if let Some(no_shadow) = update.no_shadow {
            settings.ui.no_shadow = no_shadow;
        }
        settings.save()
    }
}

#[cfg(test)]
mod tests {
    use super::{AppSettingsWithDiskSync, FeatureFlagsUpdate};

    fn create_test_settings() -> (tempfile::TempDir, AppSettingsWithDiskSync) {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let settings =
            AppSettingsWithDiskSync::new_with_customization(temp_dir.path(), None).unwrap();
        (temp_dir, settings)
    }

    #[test]
    fn update_feature_flags_updates_worktree_manipulation_and_persists() {
        let (dir, settings) = create_test_settings();
        let original_watch_mode = settings.get().unwrap().feature_flags.watch_mode.clone();
        let enabled = !settings.get().unwrap().feature_flags.worktree_manipulation;

        settings
            .update_feature_flags(FeatureFlagsUpdate {
                worktree_manipulation: Some(enabled),
                ..Default::default()
            })
            .unwrap();

        let s = settings.get().unwrap();
        assert_eq!(
            s.feature_flags.watch_mode, original_watch_mode,
            "partial updates should leave unrelated feature flags untouched"
        );
        drop(s);

        let reloaded = AppSettingsWithDiskSync::new_with_customization(dir.path(), None).unwrap();
        assert_eq!(
            reloaded.get().unwrap().feature_flags.worktree_manipulation,
            enabled,
            "feature flag updates should persist across reloads"
        );
    }

    #[test]
    fn empty_feature_flags_update_preserves_existing_value() {
        let (_dir, settings) = create_test_settings();
        settings
            .update_feature_flags(FeatureFlagsUpdate {
                worktree_manipulation: Some(true),
                ..Default::default()
            })
            .unwrap();
        settings
            .update_feature_flags(FeatureFlagsUpdate::default())
            .unwrap();
        assert!(
            settings.get().unwrap().feature_flags.worktree_manipulation,
            "an omitted flag should retain its value"
        );
    }

    #[test]
    fn feature_flags_update_deserializes_worktree_manipulation() {
        let update: FeatureFlagsUpdate =
            serde_json::from_value(serde_json::json!({ "worktreeManipulation": true })).unwrap();
        assert_eq!(
            update.worktree_manipulation,
            Some(true),
            "the API accepts camelCase feature flags"
        );
    }
}
