//! In place of commands.rs
use but_settings::AppSettings;
use but_settings::api::{FeatureFlagsUpdate, TelemetryUpdate};
use serde::Deserialize;

use crate::NoParams;
use crate::{App, error::Error};

pub fn get_app_settings(app: &App, _params: NoParams) -> Result<AppSettings, Error> {
    Ok(app.app_settings.get()?.clone())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOnboardingCompleteParams {
    pub update: bool,
}

pub fn update_onboarding_complete(
    app: &App,
    params: UpdateOnboardingCompleteParams,
) -> Result<(), Error> {
    app.app_settings
        .update_onboarding_complete(params.update)
        .map_err(|e| e.into())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTelemetryParams {
    pub update: TelemetryUpdate,
}

pub fn update_telemetry(app: &App, params: UpdateTelemetryParams) -> Result<(), Error> {
    app.app_settings
        .update_telemetry(params.update)
        .map_err(|e| e.into())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTelemetryDistinctIdParams {
    pub app_distinct_id: Option<String>,
}

pub fn update_telemetry_distinct_id(
    app: &App,
    params: UpdateTelemetryDistinctIdParams,
) -> Result<(), Error> {
    app.app_settings
        .update_telemetry_distinct_id(params.app_distinct_id)
        .map_err(|e| e.into())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFeatureFlagsParams {
    pub update: FeatureFlagsUpdate,
}

pub fn update_feature_flags(app: &App, params: UpdateFeatureFlagsParams) -> Result<(), Error> {
    app.app_settings
        .update_feature_flags(params.update)
        .map_err(|e| e.into())
}
