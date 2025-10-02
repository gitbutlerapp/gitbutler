//! In place of commands.rs
use but_api_macros::api_cmd;
use but_settings::api::{ClaudeUpdate, FeatureFlagsUpdate, ReviewsUpdate, TelemetryUpdate};
use but_settings::{AppSettings, AppSettingsWithDiskSync};
use serde::Deserialize;
use tracing::instrument;

use crate::error::Error;

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn get_app_settings() -> Result<AppSettings, Error> {
    let app_settings = AppSettings::load_from_default_path_creating()?;
    Ok(app_settings)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOnboardingCompleteParams {
    pub update: bool,
}

pub fn update_onboarding_complete(
    app_settings_sync: &AppSettingsWithDiskSync,
    params: UpdateOnboardingCompleteParams,
) -> Result<(), Error> {
    app_settings_sync
        .update_onboarding_complete(params.update)
        .map_err(|e| e.into())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTelemetryParams {
    pub update: TelemetryUpdate,
}

pub fn update_telemetry(
    app_settings_sync: &AppSettingsWithDiskSync,
    params: UpdateTelemetryParams,
) -> Result<(), Error> {
    app_settings_sync
        .update_telemetry(params.update)
        .map_err(|e| e.into())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTelemetryDistinctIdParams {
    pub app_distinct_id: Option<String>,
}

pub fn update_telemetry_distinct_id(
    app_settings_sync: &AppSettingsWithDiskSync,
    params: UpdateTelemetryDistinctIdParams,
) -> Result<(), Error> {
    app_settings_sync
        .update_telemetry_distinct_id(params.app_distinct_id)
        .map_err(|e| e.into())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFeatureFlagsParams {
    pub update: FeatureFlagsUpdate,
}

pub fn update_feature_flags(
    app_settings_sync: &AppSettingsWithDiskSync,
    params: UpdateFeatureFlagsParams,
) -> Result<(), Error> {
    app_settings_sync
        .update_feature_flags(params.update)
        .map_err(|e| e.into())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateClaudeParams {
    pub update: ClaudeUpdate,
}

pub fn update_claude(
    app_settings_sync: &AppSettingsWithDiskSync,
    params: UpdateClaudeParams,
) -> Result<(), Error> {
    app_settings_sync
        .update_claude(params.update)
        .map_err(|e| e.into())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReviewsParams {
    pub update: ReviewsUpdate,
}

pub fn update_reviews(
    app_settings_sync: &AppSettingsWithDiskSync,
    params: UpdateReviewsParams,
) -> Result<(), Error> {
    app_settings_sync
        .update_reviews(params.update)
        .map_err(|e| e.into())
}
