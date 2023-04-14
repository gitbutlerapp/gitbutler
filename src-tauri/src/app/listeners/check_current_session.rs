use std::time;

use anyhow::{Context, Result};

use crate::app::gb_repository;

pub struct Listener<'listener> {
    gb_repository: &'listener gb_repository::Repository,
}

const FIVE_MINUTES: u128 = time::Duration::new(5 * 60, 0).as_millis();
const ONE_HOUR: u128 = time::Duration::new(60 * 60, 0).as_millis();

impl<'listener> Listener<'listener> {
    pub fn new(gb_repository: &'listener gb_repository::Repository) -> Self {
        Self { gb_repository }
    }

    pub fn register(&self, ts: time::SystemTime) -> Result<()> {
        let current_session = self
            .gb_repository
            .get_current_session()
            .context("failed to get current session")?;
        if current_session.is_none() {
            return Ok(());
        }
        let current_session = current_session.unwrap();

        let now = ts
            .duration_since(time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let elapsed_last = now - current_session.meta.last_timestamp_ms;
        if elapsed_last < FIVE_MINUTES {
            return Ok(());
        }

        let elapsed_start = now - current_session.meta.start_timestamp_ms;
        if elapsed_start < ONE_HOUR {
            return Ok(());
        }

        Ok(())
    }
}
