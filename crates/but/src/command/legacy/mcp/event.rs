use but_action::reword::CommitEvent;
use but_llm::LLMProvider;

#[derive(Debug, Clone)]
pub enum Event {
    Commit(CommitEvent),
}

#[derive(Debug, Clone)]
pub struct Handler {
    sender: Option<tokio::sync::mpsc::UnboundedSender<Event>>,
}

impl Handler {
    pub fn new_in_background() -> Self {
        let sender = LLMProvider::default_openai()
            .map(|llm| {
                let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
                tokio::task::spawn(async move {
                    while let Some(event) = receiver.recv().await {
                        match event {
                            Event::Commit(c) => {
                                let mut ctx = c.ctx.clone().into_thread_local();
                                let context_lines = ctx.settings.context_lines;
                                let _res = (|| -> anyhow::Result<_> {
                                    let mut meta = ctx.meta()?;
                                    let (_guard, repo, mut ws, _db) =
                                        ctx.workspace_mut_and_db_mut()?;
                                    but_action::reword::commit(
                                        &llm,
                                        c,
                                        &repo,
                                        &mut ws,
                                        &mut meta,
                                        context_lines,
                                    )
                                })();
                            }
                        }
                    }
                });
                Some(sender)
            })
            .unwrap_or_default();

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
