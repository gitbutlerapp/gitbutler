//! The API layer is what can be used to create GitButler applications.
//!
//! ### Coordinating Filesystem Access
//!
//! For them to behave correctly in multi-threaded scenarios, be sure to use an *exclusive or shared* lock
//! on this level.
//! Lower-level crates like `but-workspace` won't use filesystem-based locking beyond what Git offers natively.
use std::sync::Arc;

use but_broadcaster::Broadcaster;
use but_claude::bridge::Claudes;
use serde::Deserialize;
use tokio::sync::Mutex;

pub mod commands;
pub use commands::*;
pub mod error;
pub mod hex_hash;

#[derive(Clone)]
pub struct App {
    pub broadcaster: Arc<Mutex<Broadcaster>>,
    pub archival: Arc<but_feedback::Archival>,
    pub claudes: Arc<Claudes>,
}

#[derive(Deserialize)]
pub struct NoParams {}
