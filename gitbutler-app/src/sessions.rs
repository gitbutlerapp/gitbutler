pub mod commands {
    use anyhow::Context;
    use gitbutler_core::error;
    use gitbutler_core::error::Code;
    use gitbutler_core::sessions::{Controller, Session};
    use tauri::{AppHandle, Manager};
    use tracing::instrument;

    use crate::error::Error;

    #[tauri::command(async)]
    #[instrument(skip(handle))]
    pub async fn list_sessions(
        handle: AppHandle,
        project_id: &str,
        earliest_timestamp_ms: Option<u128>,
    ) -> Result<Vec<Session>, Error> {
        let project_id = project_id.parse().context(error::Context::new_static(
            Code::Validation,
            "Malformed project id",
        ))?;
        handle
            .state::<Controller>()
            .list(&project_id, earliest_timestamp_ms)
            .map_err(Into::into)
    }
}
