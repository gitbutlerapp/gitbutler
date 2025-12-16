//! In place of commands.rs
use anyhow::Result;
use but_api_macros::but_api;
use but_settings::{
    AppSettings, AppSettingsWithDiskSync,
    api::{
        CliUpdate, ClaudeUpdate, FeatureFlagsUpdate, FetchUpdate, ReviewsUpdate, TelemetryUpdate,
        UiUpdate,
    },
};
use serde::Deserialize;
use tracing::instrument;

#[but_api]
#[instrument(err(Debug))]
pub fn get_app_settings() -> Result<AppSettings> {
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
) -> Result<()> {
    app_settings_sync.update_onboarding_complete(params.update)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTelemetryParams {
    pub update: TelemetryUpdate,
}

pub fn update_telemetry(
    app_settings_sync: &AppSettingsWithDiskSync,
    params: UpdateTelemetryParams,
) -> Result<()> {
    app_settings_sync.update_telemetry(params.update)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTelemetryDistinctIdParams {
    pub app_distinct_id: Option<String>,
}

pub fn update_telemetry_distinct_id(
    app_settings_sync: &AppSettingsWithDiskSync,
    params: UpdateTelemetryDistinctIdParams,
) -> Result<()> {
    app_settings_sync.update_telemetry_distinct_id(params.app_distinct_id)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFeatureFlagsParams {
    pub update: FeatureFlagsUpdate,
}

pub fn update_feature_flags(
    app_settings_sync: &AppSettingsWithDiskSync,
    params: UpdateFeatureFlagsParams,
) -> Result<()> {
    app_settings_sync.update_feature_flags(params.update)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateClaudeParams {
    pub update: ClaudeUpdate,
}

pub fn update_claude(
    app_settings_sync: &AppSettingsWithDiskSync,
    params: UpdateClaudeParams,
) -> Result<()> {
    app_settings_sync.update_claude(params.update)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReviewsParams {
    pub update: ReviewsUpdate,
}

pub fn update_reviews(
    app_settings_sync: &AppSettingsWithDiskSync,
    params: UpdateReviewsParams,
) -> Result<()> {
    app_settings_sync.update_reviews(params.update)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFetchParams {
    pub update: FetchUpdate,
}

pub fn update_fetch(
    app_settings_sync: &AppSettingsWithDiskSync,
    params: UpdateFetchParams,
) -> Result<()> {
    app_settings_sync.update_fetch(params.update)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUiParams {
    pub update: UiUpdate,
}

pub fn update_ui(
    app_settings_sync: &AppSettingsWithDiskSync,
    params: UpdateUiParams,
) -> Result<()> {
    app_settings_sync.update_ui(params.update)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCliParams {
    pub update: CliUpdate,
}

pub fn update_cli(
    app_settings_sync: &AppSettingsWithDiskSync,
    params: UpdateCliParams,
) -> Result<()> {
    app_settings_sync.update_cli(params.update)
}
