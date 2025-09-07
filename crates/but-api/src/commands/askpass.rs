//! In place of commands.rs
use but_api_macros::api_cmd;
use gitbutler_id::id::Id;
use gitbutler_repo_actions::askpass::{self, AskpassRequest};
use tracing::instrument;

use crate::error::Error;

#[api_cmd]
#[instrument(skip(response))]
pub async fn submit_prompt_response(
    id: Id<AskpassRequest>,
    response: Option<String>,
) -> Result<(), Error> {
    askpass::get_broker()
        .handle_response(id, response)
        .await;
    Ok(())
}
