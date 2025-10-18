use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::AppSettingsWithDiskSync;

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Update request for [`crate::app_settings::TelemetrySettings`].
pub struct TelemetryUpdate {
    pub app_metrics_enabled: Option<bool>,
    pub app_error_reporting_enabled: Option<bool>,
    pub app_non_anon_metrics_enabled: Option<bool>,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Update request for [`crate::app_settings::FeatureFlags`].
pub struct FeatureFlagsUpdate {
    pub cv3: Option<bool>,
    pub ws3: Option<bool>,
    pub actions: Option<bool>,
    pub butbot: Option<bool>,
    pub rules: Option<bool>,
    pub single_branch: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Update request for [`crate::app_settings::Claude`].
pub struct ClaudeUpdate {
    pub executable: Option<String>,
    pub notify_on_completion: Option<bool>,
    pub notify_on_permission_request: Option<bool>,
    pub dangerously_allow_all_permissions: Option<bool>,
    pub auto_commit_after_completion: Option<bool>,
    pub use_configured_model: Option<bool>,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Update request for [`crate::app_settings::Reviews`].
pub struct ReviewsUpdate {
    pub auto_fill_pr_description_from_commit: Option<bool>,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Update request for [`crate::app_settings::Fetch`].
pub struct FetchUpdate {
    pub auto_fetch_interval_minutes: Option<isize>,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Update request for [`crate::app_settings::UiSettings`].
pub struct UiUpdate {
    pub use_native_title_bar: Option<bool>,
}

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
        if let Some(app_non_anon_metrics_enabled) = update.app_non_anon_metrics_enabled {
            settings.telemetry.app_non_anon_metrics_enabled = app_non_anon_metrics_enabled;
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
            cv3,
            ws3,
            actions,
            butbot,
            rules,
            single_branch,
        }: FeatureFlagsUpdate,
    ) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        if let Some(cv3) = cv3 {
            settings.feature_flags.cv3 = cv3;
        }
        if let Some(ws3) = ws3 {
            settings.feature_flags.ws3 = ws3;
        }
        if let Some(actions) = actions {
            settings.feature_flags.actions = actions;
        }
        if let Some(butbot) = butbot {
            settings.feature_flags.butbot = butbot;
        }
        if let Some(rules) = rules {
            settings.feature_flags.rules = rules;
        }
        if let Some(single_branch) = single_branch {
            settings.feature_flags.single_branch = single_branch;
        }
        settings.save()
    }

    pub fn update_claude(&self, update: ClaudeUpdate) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        if let Some(executable) = update.executable {
            settings.claude.executable = executable;
        }
        if let Some(notify_on_completion) = update.notify_on_completion {
            settings.claude.notify_on_completion = notify_on_completion;
        }
        if let Some(notify_on_permission_request) = update.notify_on_permission_request {
            settings.claude.notify_on_permission_request = notify_on_permission_request;
        }
        if let Some(dangerously_allow_all_permissions) = update.dangerously_allow_all_permissions {
            settings.claude.dangerously_allow_all_permissions = dangerously_allow_all_permissions;
        }
        if let Some(auto_commit_after_completion) = update.auto_commit_after_completion {
            settings.claude.auto_commit_after_completion = auto_commit_after_completion;
        }
        if let Some(use_configured_model) = update.use_configured_model {
            settings.claude.use_configured_model = use_configured_model;
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
        settings.save()
    }
}
