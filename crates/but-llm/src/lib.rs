mod chat;
mod ollama;
mod openai;

pub use chat::{ChatMessage, StreamToolCallResult, ToolCall, ToolCallContent, ToolResponseContent};

pub use openai::{
    CredentialsKind, OpenAiProvider, stream_response_blocking, structured_output_blocking,
    tool_calling_loop, tool_calling_loop_stream,
};
