use std::sync::Mutex;

use anyhow::Result;
use but_api_macros::api_cmd_tauri;
use but_secret::{Sensitive, secret};
use tracing::instrument;

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn secret_get_global(handle: String) -> Result<Option<String>> {
    let sensitive = secret::retrieve(&handle, secret::Namespace::Global)?.map(|s| s.0);
    Ok(sensitive)
}

#[api_cmd_tauri]
#[instrument(err(Debug), err(Debug))]
pub fn secret_set_global(handle: String, secret: Sensitive<String>) -> Result<()> {
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    secret::persist(&handle, &secret, secret::Namespace::Global)
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn secret_delete_global(handle: String) -> Result<()> {
    secret::delete(&handle, secret::Namespace::Global)
}
