use std::{sync, time};

use anyhow::{Context, Result};

use crate::app::{gb_repository, sessions};

use super::events;

pub struct Handler<'handler> {
    gb_repository: sync::Arc<sync::Mutex<&'handler gb_repository::Repository>>,
}

impl<'handler> Handler<'handler> {
    pub fn new(gb_repository: &'handler gb_repository::Repository) -> Self {
        Self {
            gb_repository: sync::Arc::new(sync::Mutex::new(gb_repository)),
        }
    }

    pub fn handle(&self, now: time::SystemTime) -> Result<Vec<events::Event>> {
        match self
            .gb_repository
            .lock()
            .unwrap()
            .get_current_session()
            .context("failed to get current session")?
        {
            None => Ok(vec![]),
            Some(current_session) => {
                if should_flush(now, &current_session) {
                    Ok(vec![events::Event::FlushSession(current_session)])
                } else {
                    Ok(vec![])
                }
            }
        }
    }
}

pub(super) fn should_flush(now: time::SystemTime, session: &sessions::Session) -> bool {
    !is_session_active(now, session) || is_session_too_old(now, session)
}

const ONE_HOUR: time::Duration = time::Duration::new(60 * 60, 0);

fn is_session_too_old(now: time::SystemTime, session: &sessions::Session) -> bool {
    let session_start = time::UNIX_EPOCH
        + time::Duration::from_millis(session.meta.start_timestamp_ms.try_into().unwrap());
    session_start + ONE_HOUR < now
}

const FIVE_MINUTES: time::Duration = time::Duration::new(5 * 60, 0);

fn is_session_active(now: time::SystemTime, session: &sessions::Session) -> bool {
    let session_last_update = time::UNIX_EPOCH
        + time::Duration::from_millis(session.meta.last_timestamp_ms.try_into().unwrap());
    session_last_update + FIVE_MINUTES > now
}
