//! A butler agent

pub mod agent;
pub mod llm;
pub mod open_router;
pub mod store;
pub mod types;

#[cfg(test)]
#[allow(dead_code)]
fn get_token() -> Option<gitbutler_secret::Sensitive<String>> {
    // return Some(gitbutler_secret::Sensitive("this is secret".into()));
    gitbutler_secret::secret::retrieve(
        "gitbutler-agent-token",
        gitbutler_secret::secret::Namespace::Global,
    )
    .unwrap()
}

#[cfg(test)]
#[allow(dead_code)]
fn set_token(token: Option<&str>) {
    if let Some(token) = token {
        gitbutler_secret::secret::persist(
            "gitbutler-agent-token",
            &gitbutler_secret::Sensitive(token.into()),
            gitbutler_secret::secret::Namespace::Global,
        )
        .unwrap();
    } else {
        gitbutler_secret::secret::delete(
            "gitbutler-agent-token",
            gitbutler_secret::secret::Namespace::Global,
        )
        .unwrap();
    }
}

#[cfg(test)]
mod test {
    use crate::agent::*;
    use crate::llm::test::*;
    use crate::store::test::*;
    use crate::types::*;

    fn system_prompt() -> String {
        "You are a great agent :flower:".into()
    }

    #[test]
    fn llm_recieved_tools() {
        let available_tools = vec![Tool {
            tool_type: ToolType::Function,
            function: ToolFunction {
                name: "foo".into(),
                description: "it does foo".into(),
                parameters: ToolFunctionParameters {
                    parameters_type: ToolFunctionParametersType::Object,
                    properties: std::collections::BTreeMap::new(),
                    additional_properties: false,
                    required: vec![],
                },
                strict: true,
            },
        }];

        let available_tools_with_handler = available_tools
            .iter()
            .map(|t| ToolWithHandler {
                tool: t.clone(),
                handler: Box::new(|_| "".into()),
            })
            .collect();

        let moved_available_tools = available_tools.clone();
        let llm = MockLLM {
            callback: |_| "".into(),
            tools_callback: move |tools| {
                assert_eq!(tools, moved_available_tools);
            },
        };

        let conversation_store =
            std::cell::RefCell::new(Box::new(InMemoryConversationStore::new()));

        let responses = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let moved_responses = responses.clone();
        let callback = move |response| {
            moved_responses.lock().unwrap().push(response);
        };

        let config = AgentConfig {
            llm: Box::new(llm),
            conversation_store,
            callback,
            system_prompt: system_prompt(),
            tools: available_tools_with_handler,
        };

        agent_perform(&config, Action::StartNewThread);

        let id = responses.lock().unwrap().last().unwrap().id();

        agent_perform(
            &config,
            Action::SendMessage {
                id,
                message: "Hello!".into(),
            },
        );
        assert_eq!(responses.lock().unwrap().len(), 3);
    }

    #[test]
    fn send_message() {
        let llm = MockLLM {
            callback: |message| format!("Mocked response: {}", message),
            tools_callback: |_| {},
        };
        let conversation_store =
            std::cell::RefCell::new(Box::new(InMemoryConversationStore::new()));

        let responses = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let moved_responses = responses.clone();
        let callback = move |response| {
            moved_responses.lock().unwrap().push(response);
        };

        let config = AgentConfig {
            llm: Box::new(llm),
            conversation_store,
            callback,
            system_prompt: system_prompt(),
            tools: vec![],
        };

        agent_perform(&config, Action::StartNewThread);

        let id = responses.lock().unwrap().last().unwrap().id();

        agent_perform(
            &config,
            Action::SendMessage {
                id,
                message: "Hello!".into(),
            },
        );

        assert_eq!(responses.lock().unwrap().len(), 3);
        assert!(matches!(
            responses.lock().unwrap().last().unwrap(),
            Response::ReplyReceived { .. }
        ));

        assert_eq!(
            config.conversation_store.borrow().read(id).unwrap(),
            vec![
                Message {
                    role: MessageRole::System,
                    content: system_prompt(),
                },
                Message {
                    role: MessageRole::User,
                    content: "Hello!".into(),
                },
                Message {
                    role: MessageRole::Assistant,
                    content: "Mocked response: Hello!".into(),
                },
            ],
        );
    }

    #[test]
    fn create_thread() {
        let llm = MockLLM {
            callback: |message| format!("Mocked response: {}", message),
            tools_callback: |_| {},
        };
        let conversation_store =
            std::cell::RefCell::new(Box::new(InMemoryConversationStore::new()));

        let responses = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let moved_responses = responses.clone();
        let callback = move |response| {
            moved_responses.lock().unwrap().push(response);
        };

        let config = AgentConfig {
            llm: Box::new(llm),
            conversation_store,
            callback,
            system_prompt: system_prompt(),
            tools: vec![],
        };

        agent_perform(&config, Action::StartNewThread);

        assert_eq!(responses.lock().unwrap().len(), 1);
        assert!(matches!(
            responses.lock().unwrap().last().unwrap(),
            Response::ThreadCreated { .. }
        ));

        let id = responses.lock().unwrap().last().unwrap().id();

        assert_eq!(
            config.conversation_store.borrow().read(id).unwrap(),
            vec![Message {
                role: MessageRole::System,
                content: system_prompt(),
            }],
        );
    }
}
