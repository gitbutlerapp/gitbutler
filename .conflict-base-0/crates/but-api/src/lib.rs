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
pub struct IpcContext {
    pub app_settings: Arc<AppSettingsWithDiskSync>,
    pub user_controller: Arc<gitbutler_user::Controller>,
    pub broadcaster: Arc<Mutex<Broadcaster>>,
    pub archival: Arc<gitbutler_feedback::Archival>,
}

#[derive(Deserialize)]
pub struct NoParams {}
