use std::sync::Arc;
use tokio::sync::Mutex;

use crate::users;

#[derive(Clone)]
pub struct AppState {
    pub users_controller: Arc<Mutex<users::Controller>>,
}
