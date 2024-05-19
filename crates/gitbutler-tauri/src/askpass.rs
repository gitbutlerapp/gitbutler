pub mod commands {
    use gitbutler_core::{
        askpass::{self, AskpassRequest},
        id::Id,
    };

    #[tauri::command(async)]
    #[tracing::instrument(skip(response))]
    pub async fn submit_prompt_response(
        id: Id<AskpassRequest>,
        response: Option<String>,
    ) -> Result<(), ()> {
        askpass::get_broker().handle_response(id, response).await;
        Ok(())
    }
}
