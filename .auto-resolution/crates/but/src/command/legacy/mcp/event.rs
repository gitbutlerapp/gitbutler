use but_action::{CredentialsKind, OpenAiProvider, reword::CommitEvent};

#[derive(Debug, Clone)]
pub enum Event {
    Commit(CommitEvent),
}

#[derive(Debug, Clone)]
pub struct Handler {
    sender: Option<tokio::sync::mpsc::UnboundedSender<Event>>,
    credentials_kind: Option<CredentialsKind>,
}

impl Handler {
    pub fn new_in_background() -> Self {
        let (credentials_kind, sender) = OpenAiProvider::with(None)
            .and_then(|openai| {
                openai
                    .client()
                    .ok()
                    .map(|client| (openai.credentials_kind(), client))
            })
            .map(|(credentials_kind, client)| {
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
                (Some(credentials_kind), Some(sender))
            })
            .unwrap_or((None, None));

        Self {
            sender,
            credentials_kind,
        }
    }

    fn send(&self, event: Event) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(event);
        }
    }

    pub fn credentials_kind(&self) -> Option<CredentialsKind> {
        self.credentials_kind.clone()
    }

    pub fn process_commit(&self, event: CommitEvent) {
        self.send(Event::Commit(event));
    }
}
