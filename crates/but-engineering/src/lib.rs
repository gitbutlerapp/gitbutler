//! A Slack-like coordination system for coding agents working in the same repository.
//!
//! This crate provides a shared channel for agents to:
//! - Post messages to a shared channel
//! - Read messages (with blocking wait support)
//! - Set/clear status
//! - List active agents
//!
//! All output is JSON. The database is SQLite stored at `.git/gitbutler/but-engineering.db`.

#![deny(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod args;
pub mod command;
pub mod db;
pub mod duration;
pub mod session;
pub mod types;
