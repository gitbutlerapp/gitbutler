//! In place of commands.rs
use but_settings::AppSettings;
use but_settings::api::{FeatureFlagsUpdate, TelemetryUpdate};
use serde::Deserialize;

use crate::NoParams;
use crate::{IpcContext, error::Error};

pub fn get_app_settings(ipc_ctx: &IpcContext, _params: NoParams) -> Result<AppSettings, Error> {
    Ok(ipc_ctx.app_settings.get()?.clone())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOnboardingCompleteParams {
    pub update: bool,
}

pub fn update_onboarding_complete(
    ipc_ctx: &IpcContext,
    params: UpdateOnboardingCompleteParams,
) -> Result<(), Error> {
    ipc_ctx
        .app_settings
        .update_onboarding_complete(params.update)
        .map_err(|e| e.into())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTelemetryParams {
    pub update: TelemetryUpdate,
}

pub fn update_telemetry(ipc_ctx: &IpcContext, params: UpdateTelemetryParams) -> Result<(), Error> {
    ipc_ctx
        .app_settings
        .update_telemetry(params.update)
        .map_err(|e| e.into())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTelemetryDistinctIdParams {
    pub app_distinct_id: Option<String>,
}

pub fn update_telemetry_distinct_id(
    ipc_ctx: &IpcContext,
    params: UpdateTelemetryDistinctIdParams,
) -> Result<(), Error> {
    ipc_ctx
        .app_settings
        .update_telemetry_distinct_id(params.app_distinct_id)
        .map_err(|e| e.into())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFeatureFlagsParams {
    pub update: FeatureFlagsUpdate,
}

pub fn update_feature_flags(
    ipc_ctx: &IpcContext,
    params: UpdateFeatureFlagsParams,
) -> Result<(), Error> {
    ipc_ctx
        .app_settings
        .update_feature_flags(params.update)
        .map_err(|e| e.into())
}
