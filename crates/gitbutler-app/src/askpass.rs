pub mod commands {
    use gitbutler::{
        askpass::{AskpassBroker, AskpassRequest},
        id::Id,
    };
    use tauri::{AppHandle, Manager};

    #[tauri::command(async)]
    #[tracing::instrument(skip(handle, response))]
    pub async fn submit_prompt_response(
        handle: AppHandle,
        id: Id<AskpassRequest>,
        response: Option<String>,
    ) -> Result<(), ()> {
        handle
            .state::<AskpassBroker>()
            .handle_response(id, response)
            .await;
        Ok(())
    }
}
