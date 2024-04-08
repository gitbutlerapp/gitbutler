pub mod commands {
    use anyhow::Context;
    use std::collections::HashMap;

    use gitbutler_core::deltas::{Controller, Delta};
    use gitbutler_core::error;
    use gitbutler_core::error::Code;
    use tauri::{AppHandle, Manager};
    use tracing::instrument;

    use crate::error::Error;

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn list_deltas(
        handle: AppHandle,
        project_id: &str,
        session_id: &str,
        paths: Option<Vec<&str>>,
    ) -> Result<HashMap<String, Vec<Delta>>, Error> {
        let session_id = session_id.parse().context(error::Context::new_static(
            Code::Validation,
            "Malformed session id",
        ))?;
        let project_id = project_id.parse().context(error::Context::new_static(
            Code::Validation,
            "Malformed project id",
        ))?;
        handle
            .state::<Controller>()
            .list_by_session_id(&project_id, &session_id, &paths)
            .map_err(Into::into)
    }
}
