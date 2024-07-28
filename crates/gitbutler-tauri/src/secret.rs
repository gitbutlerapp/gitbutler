use crate::error::Error;
use gitbutler_secret::secret;
use gitbutler_secret::Sensitive;
use std::sync::Mutex;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn secret_get_global(handle: &str) -> Result<Option<String>, Error> {
    Ok(secret::retrieve(handle, secret::Namespace::Global)?.map(|s| s.0))
}

#[tauri::command(async)]
#[instrument(skip(secret), err(Debug), fields(secret = "<redacted>"))]
pub fn secret_set_global(handle: &str, secret: String) -> Result<(), Error> {
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    Ok(secret::persist(
        handle,
        &Sensitive(secret),
        secret::Namespace::Global,
    )?)
}
