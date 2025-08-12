//! The API layer is what can be used to create GitButler applications.
//!
//! ### Coordinating Filesystem Access
//!
//! For them to behave correctly in multi-threaded scenarios, be sure to use an *exclusive or shared* lock
//! on this level.
//! Lower-level crates like `but-workspace` won't use filesystem-based locking beyond what Git offers natively.
use std::sync::Arc;

use but_settings::AppSettingsWithDiskSync;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::broadcaster::Broadcaster;

pub mod broadcaster;
pub mod commands;
pub mod error;
pub mod hex_hash;

#[derive(Clone)]
pub struct App {
    pub app_settings: Arc<AppSettingsWithDiskSync>,
    pub broadcaster: Arc<Mutex<Broadcaster>>,
    pub archival: Arc<gitbutler_feedback::Archival>,
}

#[derive(Deserialize)]
pub struct NoParams {}
