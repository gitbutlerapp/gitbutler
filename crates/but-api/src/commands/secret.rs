use std::sync::Mutex;

use but_api_macros::api_cmd;
use but_secret::{Sensitive, secret};
use tracing::instrument;

use crate::error::Error;

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn secret_get(handle: String) -> Result<Option<String>, Error> {
    let sensitive = secret::retrieve(&handle, secret::Namespace::BuildKind)?.map(|s| s.0);
    Ok(sensitive)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug), skip(secret), fields(secret = "<redacted>"))]
pub fn secret_set(handle: String, secret: String) -> Result<(), Error> {
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    Ok(secret::persist(
        &handle,
        &Sensitive(secret),
        secret::Namespace::BuildKind,
    )?)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn secret_delete(handle: String) -> Result<(), Error> {
    Ok(secret::delete(&handle, secret::Namespace::BuildKind)?)
}

// Deprecated functions kept for backwards compatibility during migration
#[deprecated(note = "Use secret_get instead")]
#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn secret_get_global(handle: String) -> Result<Option<String>, Error> {
    let sensitive = secret::retrieve(&handle, secret::Namespace::Global)?.map(|s| s.0);
    Ok(sensitive)
}

#[deprecated(note = "Use secret_set instead")]
#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug), skip(secret), fields(secret = "<redacted>"))]
pub fn secret_set_global(handle: String, secret: String) -> Result<(), Error> {
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    Ok(secret::persist(
        &handle,
        &Sensitive(secret),
        secret::Namespace::Global,
    )?)
}

#[deprecated(note = "Use secret_delete instead")]
#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn secret_delete_global(handle: String) -> Result<(), Error> {
    Ok(secret::delete(&handle, secret::Namespace::Global)?)
}
