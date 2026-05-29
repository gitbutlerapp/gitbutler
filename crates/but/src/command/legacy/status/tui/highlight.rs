use std::time::{Duration, Instant};

use ratatui::{style::Style, text::Span};

const DURATION: Duration = Duration::from_millis(150);

#[derive(Debug)]
pub(super) struct Highlights<T> {
    ids: Vec<Highlighted<T>>,
}

impl<T> Default for Highlights<T> {
    fn default() -> Self {
        Self { ids: Vec::new() }
    }
}

#[derive(Debug)]
struct Highlighted<T> {
    id: T,
    expire_at: Instant,
}

impl<T> Highlights<T>
where
    T: PartialEq,
{
    pub(super) fn insert(&mut self, id: T) {
        if self.contains(&id) {
            return;
        }
        self.ids.push(Highlighted {
            id,
            expire_at: Instant::now() + DURATION,
        });
    }

    pub(super) fn contains(&self, id: &T) -> bool {
        self.ids.iter().any(|h| &h.id == id)
    }

    pub(super) fn update(&mut self) -> bool {
        let now = Instant::now();
        let len_before = self.ids.len();
        self.ids.retain(|id| id.expire_at > now);
        let len_after = self.ids.len();
        len_before != len_after
    }
}

pub(super) fn style() -> Style {
    Style::new().black().on_white().not_dim()
}

pub(super) fn with_highlight(span: Span<'static>) -> Span<'static> {
    span.style(style())
}
