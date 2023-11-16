use crate::projects::ProjectId;
use anyhow::Result;
use governor::{
    clock::QuantaClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use nonzero_ext::nonzero;
use once_cell::sync::OnceCell;
use std::{sync::Arc, vec};

use super::events;

#[derive(Clone)]
pub struct Handler {
    quota: Quota,
    limit: Arc<OnceCell<RateLimiter<NotKeyed, InMemoryState, QuantaClock>>>,
}

impl Handler {
    pub fn new() -> Self {
        let quota: Quota = Quota::per_second(nonzero!(1_u32)); // 1 per second at most.
        let limit: OnceCell<RateLimiter<NotKeyed, InMemoryState, QuantaClock>> = OnceCell::new();
        Handler {
            quota: quota,
            limit: limit.into(),
        }
    }

    pub fn handle<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        project_id: &ProjectId,
    ) -> Result<Vec<events::Event>> {
        let path = path.as_ref().to_path_buf();
        let mut events = vec![events::Event::SessionProcessing(*project_id, path)];

        let rate_limiter = self
            .limit
            .get_or_init(|| RateLimiter::direct(self.quota.clone()));

        if rate_limiter.check().is_ok() {
            events.push(events::Event::VirtualBranch(*project_id));
        }
        Ok(events)
    }
}
