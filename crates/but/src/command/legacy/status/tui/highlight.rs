use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use ratatui::{style::Stylize as _, text::Span};

use crate::CliId;

#[derive(Debug, Default)]
pub(super) struct Highlights {
    ids: Vec<HighlightedCliId>,
}

#[derive(Debug)]
struct HighlightedCliId {
    id: Arc<CliId>,
    expire_at: Instant,
}

impl Highlights {
    pub(super) fn insert(&mut self, id: Arc<CliId>) {
        if self.contains(&id) {
            return;
        }
        self.ids.push(HighlightedCliId {
            id,
            expire_at: Instant::now() + Duration::from_secs_f32(0.15),
        });
    }

    pub(super) fn contains(&self, id: &CliId) -> bool {
        self.ids.iter().any(|h| &*h.id == id)
    }

    pub(super) fn update(&mut self) -> bool {
        let now = Instant::now();
        let len_before = self.ids.len();
        self.ids.retain(|id| id.expire_at > now);
        let len_after = self.ids.len();
        len_before != len_after
    }
}

pub(super) fn with_highlight(span: Span<'static>) -> Span<'static> {
    span.black().on_white().not_dim()
}
