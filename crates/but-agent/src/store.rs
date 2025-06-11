use crate::types::{ConversationId, Message};
use anyhow::Result;

pub trait ConversationStore {
    fn read(&self, id: ConversationId) -> Result<Vec<Message>>;
    fn write(&mut self, id: ConversationId, messages: &[Message]);
    fn read_all(&self) -> Result<std::collections::HashMap<ConversationId, Vec<Message>>>;
}

#[cfg(test)]
pub(crate) mod test {
    use super::*;

    pub struct InMemoryConversationStore {
        map: std::collections::HashMap<ConversationId, Vec<Message>>,
    }

    // Construction
    impl InMemoryConversationStore {
        pub fn new() -> Self {
            Self {
                map: std::collections::HashMap::new(),
            }
        }
    }

    impl Default for InMemoryConversationStore {
        fn default() -> Self {
            Self::new()
        }
    }

    impl ConversationStore for InMemoryConversationStore {
        fn read(&self, id: ConversationId) -> Result<Vec<Message>> {
            self.map
                .get(&id)
                .cloned()
                .ok_or(anyhow::anyhow!("Conversation not found"))
        }

        fn write(&mut self, id: ConversationId, messages: &[Message]) {
            self.map.insert(id, messages.to_owned());
        }

        fn read_all(&self) -> Result<std::collections::HashMap<ConversationId, Vec<Message>>> {
            Ok(self.map.clone())
        }
    }
}
