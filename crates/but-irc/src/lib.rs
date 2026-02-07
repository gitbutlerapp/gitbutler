//! IRC client implementation for GitButler.
//!
//! This crate provides an IRC client built on top of the [`irc`](https://docs.rs/irc) crate,
//! with GitButler-specific protocol extensions for session management.
//!
//! # Features
//!
//! - Standard IRC over TLS (port 6697)
//! - Automatic capability negotiation (IRCv3)
//! - Data payload encoding in messages
//! - Connection state tracking
//! - GitButler protocol messages (session management, prompts, responses)
//!
//! # Example
//!
//! ```no_run
//! use but_irc::{IrcClient, IrcConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = IrcConfig {
//!         server: "irc.example.com".to_string(),
//!         port: 6697,
//!         use_tls: true,
//!         nick: "mybot".to_string(),
//!         password: None,
//!         username: Some("mybot".to_string()),
//!         realname: Some("My Bot".to_string()),
//!     };
//!
//!     let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
//!     let mut client = IrcClient::new(config, shutdown_rx);
//!     client.connect().await?;
//!
//!     // Get the event receiver
//!     let mut events = client.take_event_receiver().unwrap();
//!
//!     // Join a channel
//!     client.join("#gitbutler")?;
//!
//!     // Handle events
//!     while let Some(event) = events.recv().await {
//!         println!("Event: {:?}", event);
//!     }
//!
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod commands;
pub mod error;
pub mod lifecycle;
pub mod manager;
pub mod message;
pub mod message_store;
pub mod protocol;
pub mod state;

pub use client::{IrcClient, IrcConfig};
pub use error::{IrcError, Result};
pub use manager::{ConnectionId, IrcManager};
pub use message::IrcEvent;
pub use message_store::{
    ChannelInfo, CommitReaction, MessageDirection, MessageStore, StoredMessage,
};
pub use protocol::{GitButlerMessage, GitButlerProtocol};
pub use state::ConnectionState;

/// Default number of messages to request for chat history (CHATHISTORY LATEST on join
/// and CHATHISTORY BEFORE for load-more pagination).
pub const DEFAULT_HISTORY_LIMIT: u32 = 150;
