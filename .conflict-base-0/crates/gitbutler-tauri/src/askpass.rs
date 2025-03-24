pub mod commands {
    use gitbutler_id::id::Id;
    use gitbutler_repo_actions::askpass::{self, AskpassRequest};

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
