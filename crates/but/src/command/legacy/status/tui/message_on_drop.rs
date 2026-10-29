use std::sync::mpsc::Sender;

use crate::command::legacy::status::tui::Message;

/// Create a `MessageOnDrop` which, as the name implies, will send a message from its `Drop`
/// implementation.
///
/// This can be used as a sort of RAII guard that'll guarantee we clean up state.
pub fn message_on_drop(msg: Message, messages: &mut Vec<Message>) -> MessageOnDrop {
    let (tx, rx) = std::sync::mpsc::channel::<Message>();

    messages.push(Message::RegisterOutOfBandMessage(rx));

    MessageOnDrop {
        tx,
        msg: Some(Box::new(msg)),
    }
}

#[derive(Debug)]
#[must_use]
pub struct MessageOnDrop {
    tx: Sender<Message>,
    msg: Option<Box<Message>>,
}

impl Drop for MessageOnDrop {
    fn drop(&mut self) {
        if let Some(msg) = self.msg.take() {
            _ = self.tx.send(*msg);
        }
    }
}
