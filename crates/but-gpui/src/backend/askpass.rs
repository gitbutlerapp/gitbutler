//! Credential prompts from Git, like SSH key passphrases, routed to the UI.

use super::runtime;

#[derive(Clone, Copy)]
pub struct PromptId(but_askpass::AskpassRequestId);

pub struct Prompt {
    pub id: PromptId,
    /// What Git asks, like "Enter passphrase for key '…':".
    pub text: String,
}

/// Route credential prompts to `on_prompt`. Call once at startup; every prompt must get a
/// [`respond()`].
pub fn init(on_prompt: impl Fn(Prompt) + Send + Sync + 'static) {
    but_askpass::try_init(move |event| {
        on_prompt(Prompt {
            id: PromptId(event.id),
            text: event.prompt,
        })
    })
    .ok();
}

/// Answer a prompt; `None` cancels it.
pub fn respond(id: PromptId, response: Option<String>) {
    if let Some(Some(broker)) = but_askpass::try_get_broker() {
        runtime().spawn(async move { broker.handle_response(id.0, response).await });
    }
}
