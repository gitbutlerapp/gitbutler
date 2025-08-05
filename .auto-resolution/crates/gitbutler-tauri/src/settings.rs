#![allow(deprecated)]
use but_api::NoParams;
use but_api::{commands::settings, IpcContext};
use but_settings::api::{FeatureFlagsUpdate, TelemetryUpdate};
use but_settings::AppSettings;
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn get_app_settings(ipc_ctx: State<'_, IpcContext>) -> Result<AppSettings, Error> {
    settings::get_app_settings(&ipc_ctx, NoParams {})
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn update_onboarding_complete(
    ipc_ctx: State<'_, IpcContext>,
    update: bool,
) -> Result<(), Error> {
    settings::update_onboarding_complete(
        &ipc_ctx,
        settings::UpdateOnboardingCompleteParams { update },
    )
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn update_telemetry(
    ipc_ctx: State<'_, IpcContext>,
    update: TelemetryUpdate,
) -> Result<(), Error> {
    settings::update_telemetry(&ipc_ctx, settings::UpdateTelemetryParams { update })
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn update_telemetry_distinct_id(
    ipc_ctx: State<'_, IpcContext>,
    app_distinct_id: Option<String>,
) -> Result<(), Error> {
    settings::update_telemetry_distinct_id(
        &ipc_ctx,
        settings::UpdateTelemetryDistinctIdParams { app_distinct_id },
    )
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn update_feature_flags(
    ipc_ctx: State<'_, IpcContext>,
    update: FeatureFlagsUpdate,
) -> Result<(), Error> {
    settings::update_feature_flags(&ipc_ctx, settings::UpdateFeatureFlagsParams { update })
}
