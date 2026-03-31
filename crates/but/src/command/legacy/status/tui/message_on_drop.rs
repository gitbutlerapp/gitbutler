use std::{rc::Rc, sync::mpsc::Sender};

use crate::command::legacy::status::tui::Message;

/// Create a `MessageOnDrop` which, as the name implies, will send a message from its `Drop`
/// implementation.
///
/// This can be used as a sort of RAII guard that'll guarantee we clean up state, such as unlocking
/// the details view.
pub(super) fn message_on_drop(msg: Message, messages: &mut Vec<Message>) -> MessageOnDrop {
    let (tx, rx) = std::sync::mpsc::channel::<Message>();

    messages.push(Message::RegisterMessageOnDrop(Rc::new(rx)));

    MessageOnDrop(Rc::new(Shared {
        tx,
        msg: Some(Box::new(msg)),
    }))
}

#[derive(Debug, Clone)]
#[must_use]
pub(super) struct MessageOnDrop(#[expect(dead_code)] Rc<Shared>);

#[derive(Debug)]
struct Shared {
    tx: Sender<Message>,
    msg: Option<Box<Message>>,
}

impl Drop for Shared {
    fn drop(&mut self) {
        if let Some(msg) = self.msg.take() {
            _ = self.tx.send(*msg);
        }
    }
}
