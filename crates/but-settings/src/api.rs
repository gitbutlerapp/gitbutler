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
    pub unapply_v3_pgm: Option<bool>,
    pub single_branch: Option<bool>,
    pub worktree_manipulation: Option<bool>,
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
            unapply_v3_pgm,
            single_branch,
            worktree_manipulation,
        }: FeatureFlagsUpdate,
    ) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        if let Some(unapply_v3_pgm) = unapply_v3_pgm {
            settings.feature_flags.unapply_v3_pgm = unapply_v3_pgm;
        }
        if let Some(single_branch) = single_branch {
            settings.feature_flags.single_branch = single_branch;
        }
        if let Some(worktree_manipulation) = worktree_manipulation {
            settings.feature_flags.worktree_manipulation = worktree_manipulation;
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
    use super::*;

    fn create_test_settings() -> (tempfile::TempDir, AppSettingsWithDiskSync) {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let settings =
            AppSettingsWithDiskSync::new_with_customization(temp_dir.path(), None).unwrap();
        (temp_dir, settings)
    }

    #[test]
    fn update_feature_flags_updates_unapply_v3_pgm_and_persists() {
        let (dir, settings) = create_test_settings();
        let original_single_branch = settings.get().unwrap().feature_flags.single_branch;

        settings
            .update_feature_flags(FeatureFlagsUpdate {
                unapply_v3_pgm: Some(true),
                single_branch: None,
                worktree_manipulation: None,
            })
            .unwrap();

        let s = settings.get().unwrap();
        assert!(
            s.feature_flags.unapply_v3_pgm,
            "the API should be able to enable the Unapply v3 PGM flag"
        );
        assert_eq!(
            s.feature_flags.single_branch, original_single_branch,
            "partial updates should leave unrelated feature flags untouched"
        );
        drop(s);

        let reloaded = AppSettingsWithDiskSync::new_with_customization(dir.path(), None).unwrap();
        assert!(
            reloaded.get().unwrap().feature_flags.unapply_v3_pgm,
            "the Unapply v3 PGM flag should be readable after reload"
        );
    }

    #[test]
    fn feature_flags_update_deserializes_unapply_v3_pgm_from_api_payload() {
        let update: FeatureFlagsUpdate =
            serde_json::from_value(serde_json::json!({ "unapplyV3Pgm": true })).unwrap();

        assert_eq!(
            update.unapply_v3_pgm,
            Some(true),
            "the API payload should map unapplyV3Pgm to the settings update"
        );
    }
}
