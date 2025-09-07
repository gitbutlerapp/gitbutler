use but_api_macros::api_cmd;
use gitbutler_secret::{Sensitive, secret};
use std::sync::Mutex;
use tracing::instrument;

use crate::error::Error;

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn secret_get_global(handle: String) -> Result<Option<String>, Error> {
    Ok(secret::retrieve(&handle, secret::Namespace::Global)?.map(|s| s.0))
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn secret_set_global(handle: String, secret: String) -> Result<(), Error> {
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    Ok(secret::persist(
        &handle,
        &Sensitive(secret),
        secret::Namespace::Global,
    )?)
}
