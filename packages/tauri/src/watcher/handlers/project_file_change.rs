use std::{sync::Arc, time::Duration, vec};

use anyhow::Result;
use governor::{
    clock::QuantaClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};

use crate::projects::ProjectId;

use super::events;

#[derive(Clone)]
pub struct Handler {
    vbranch_calculation_limit: Arc<RateLimiter<NotKeyed, InMemoryState, QuantaClock>>,
}

impl Handler {
    pub fn new() -> Self {
        let quota = Quota::with_period(Duration::from_millis(100)).expect("valid quota");
        Handler {
            vbranch_calculation_limit: Arc::new(RateLimiter::direct(quota)),
        }
    }

    #[allow(clippy::unnecessary_wraps)]
    pub fn handle<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        project_id: &ProjectId,
    ) -> Result<Vec<events::Event>> {
        let path = path.as_ref().to_path_buf();
        let mut events = vec![events::Event::CalculateDeltas(*project_id, path)];

        if self.vbranch_calculation_limit.check().is_ok() {
            events.push(events::Event::CalculateVirtualBranches(*project_id));
        }
        Ok(events)
    }
}
