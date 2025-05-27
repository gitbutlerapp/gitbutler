use but_agent::{
    store::{ConversationStore, ConversationStoreReadError},
    types::{ConversationId, Message},
};
use gitbutler_command_context::CommandContext;

pub struct FSConversationStore {
    path: std::path::PathBuf,
}

pub trait ConversationStoreAccess {
    fn conversation_store(&self) -> FSConversationStore;
}

impl ConversationStoreAccess for CommandContext {
    fn conversation_store(&self) -> FSConversationStore {
        FSConversationStore {
            path: self.project().gb_dir().join("conversation_store.json"),
        }
    }
}

impl ConversationStore for FSConversationStore {
    fn read_all(
        &self,
    ) -> Result<std::collections::HashMap<ConversationId, Vec<Message>>, ConversationStoreReadError>
    {
        let contents = std::fs::read_to_string(&self.path).unwrap_or("{}".to_string());
        let map: std::collections::HashMap<ConversationId, Vec<Message>> =
            serde_json::from_str(&contents).unwrap_or_default();
        Ok(map)
    }

    fn read(&self, id: ConversationId) -> Result<Vec<Message>, ConversationStoreReadError> {
        let map = self.read_all()?;
        let messages = map.get(&id).cloned().unwrap_or_default();

        Ok(messages)
    }

    fn write(&mut self, id: ConversationId, messages: &[Message]) {
        let contents = std::fs::read_to_string(&self.path).unwrap_or("{}".to_string());
        let mut map: std::collections::HashMap<ConversationId, Vec<Message>> =
            serde_json::from_str(&contents).unwrap_or_default();
        map.insert(id, messages.to_vec());
        let contents = serde_json::to_string(&map).unwrap();
        std::fs::create_dir_all(self.path.parent().unwrap()).unwrap();
        std::fs::write(&self.path, contents).unwrap();
    }
}
