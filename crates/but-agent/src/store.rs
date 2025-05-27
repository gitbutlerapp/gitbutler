use crate::types::{ConversationId, Message};

#[derive(Debug)]
pub enum ConversationStoreReadError {
    NotFound,
    FailedToRead,
}

pub trait ConversationStore {
    fn read(&self, id: ConversationId) -> Result<Vec<Message>, ConversationStoreReadError>;
    fn write(&mut self, id: ConversationId, messages: &[Message]);
    fn read_all(
        &self,
    ) -> Result<std::collections::HashMap<ConversationId, Vec<Message>>, ConversationStoreReadError>;
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
        fn read(&self, id: ConversationId) -> Result<Vec<Message>, ConversationStoreReadError> {
            self.map
                .get(&id)
                .cloned()
                .ok_or(ConversationStoreReadError::NotFound)
        }

        fn write(&mut self, id: ConversationId, messages: &[Message]) {
            self.map.insert(id, messages.to_owned());
        }

        fn read_all(
            &self,
        ) -> Result<
            std::collections::HashMap<ConversationId, Vec<Message>>,
            ConversationStoreReadError,
        > {
            Ok(self.map.clone())
        }
    }
}
