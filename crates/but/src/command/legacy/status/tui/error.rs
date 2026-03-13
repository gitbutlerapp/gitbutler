use std::{sync::Arc, time::Instant};

use crate::command::legacy::status::tui::Message;

#[derive(Debug)]
pub(super) struct AppError {
    pub(super) inner: Arc<anyhow::Error>,
    pub(super) dismiss_at: Instant,
}

pub(super) trait TuiResultExt<T> {
    fn show_error_in_tui(self, messages: &mut Vec<Message>) -> Option<T>;
}

impl<T> TuiResultExt<T> for Result<T, anyhow::Error> {
    fn show_error_in_tui(self, messages: &mut Vec<Message>) -> Option<T> {
        match self {
            Ok(t) => Some(t),
            Err(err) => {
                messages.push(Message::ShowError(Arc::new(err)));
                None
            }
        }
    }
}
