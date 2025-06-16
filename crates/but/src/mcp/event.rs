use but_action::{OpenAiProvider, reword::CommitEvent};

#[derive(Debug, Clone)]
pub enum Event {
    Commit(CommitEvent),
}

#[derive(Debug, Clone)]
pub struct Handler {
    sender: Option<tokio::sync::mpsc::UnboundedSender<Event>>,
}

impl Handler {
    pub fn new_with_background_handling() -> Self {
        let sender = OpenAiProvider::with(None)
            .and_then(|openai| openai.client().ok())
            .map(|client| {
                let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
                tokio::task::spawn(async move {
                    while let Some(event) = receiver.recv().await {
                        match event {
                            Event::Commit(c) => {
                                let _ = but_action::reword::commit(&client, c).await;
                            }
                        }
                    }
                });
                sender
            });
        Self { sender }
    }

    fn send(&self, event: Event) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(event);
        }
    }

    pub fn process_commit(&self, event: CommitEvent) {
        self.send(Event::Commit(event));
    }
}
