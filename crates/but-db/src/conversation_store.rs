use crate::{
    DbHandle,
    schema::{ai_conversations, ai_messages},
};
use anyhow::Result;
use but_agent::{
    store::ConversationStore,
    types::{ConversationId, Message, MessageRole},
};
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use std::{cell::RefCell, str::FromStr as _};

#[derive(Debug, Queryable, Identifiable)]
#[diesel(table_name = ai_conversations)]
struct Conversation {
    id: String,
    #[allow(dead_code)]
    name: String,
}

#[derive(Debug, Queryable, Identifiable)]
#[diesel(table_name = ai_messages)]
struct DbMessage {
    id: String,
    #[allow(dead_code)]
    conversation_id: String,
    role: String,
    content: String,
    tool_call_id: Option<String>,
    #[allow(dead_code)]
    order: i32,
}

pub struct SQLiteConversationStore<'a> {
    conn: RefCell<&'a mut SqliteConnection>,
}

impl ConversationStore for SQLiteConversationStore<'_> {
    fn read_all(&self) -> Result<std::collections::HashMap<ConversationId, Vec<Message>>> {
        let conversations = ai_conversations::table
            .load::<Conversation>(*self.conn.borrow_mut())
            .map_err(|_| anyhow::anyhow!("Failed to read conversations"))?;

        let mut result = std::collections::HashMap::new();

        for conversation in conversations {
            let messages = self.read(ConversationId(
                uuid::Uuid::from_str(&conversation.id).unwrap(),
            ))?;
            result.insert(
                ConversationId(uuid::Uuid::from_str(&conversation.id).unwrap()),
                messages,
            );
        }

        Ok(result)
    }

    fn read(&self, id: ConversationId) -> Result<Vec<Message>> {
        let messages = ai_messages::table
            .filter(ai_messages::conversation_id.eq(&id.0.to_string()))
            .order(ai_messages::order.asc())
            .load::<DbMessage>(*self.conn.borrow_mut())
            .map_err(|_| anyhow::anyhow!("Failed to read messages"))?;

        Ok(messages
            .into_iter()
            .map(|msg| Message {
                role: MessageRole::from_str(&msg.role).unwrap(),
                content: msg.content,
                tool_call_id: msg.tool_call_id,
            })
            .collect())
    }

    fn write(&mut self, id: ConversationId, messages: &[Message]) {
        // Start a transaction
        self.conn
            .borrow_mut()
            .transaction::<_, diesel::result::Error, _>(|conn| {
                // First, ensure the conversation exists
                diesel::insert_or_ignore_into(ai_conversations::table)
                    .values((
                        ai_conversations::id.eq(&id.0.to_string()),
                        ai_conversations::name.eq("Untitled Conversation"),
                    ))
                    .execute(conn)?;

                // Delete existing messages for this conversation
                diesel::delete(ai_messages::table)
                    .filter(ai_messages::conversation_id.eq(&id.0.to_string()))
                    .execute(conn)?;

                // Insert new messages
                for (idx, message) in messages.iter().enumerate() {
                    diesel::insert_into(ai_messages::table)
                        .values((
                            ai_messages::id.eq(uuid::Uuid::new_v4().to_string()),
                            ai_messages::conversation_id.eq(&id.0.to_string()),
                            ai_messages::role.eq(&message.role.to_string()),
                            ai_messages::content.eq(&message.content),
                            ai_messages::tool_call_id.eq(message.tool_call_id.as_deref()),
                            ai_messages::order.eq(idx as i32),
                        ))
                        .execute(conn)?;
                }

                Ok(())
            })
            .expect("Failed to write conversation");
    }
}

pub trait ConversationStoreAccess {
    fn conversation_store(&mut self) -> SQLiteConversationStore<'_>;
}

impl ConversationStoreAccess for DbHandle {
    fn conversation_store(&mut self) -> SQLiteConversationStore<'_> {
        SQLiteConversationStore {
            conn: RefCell::new(&mut self.conn),
        }
    }
}
