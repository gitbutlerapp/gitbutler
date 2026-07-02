use std::{convert::Infallible, time::Duration};

use crossterm::event::{self, Event, KeyCode};

/// Trait for abstracting event polling so we can hardcode events in tests.
pub trait EventPolling {
    type Error: std::error::Error + Send + Sync + 'static;

    fn poll(self, timeout: Duration) -> Result<impl IntoIterator<Item = Event>, Self::Error>;
}

/// An [`EventPolling`] implementation that polls events for real using crossterm.
#[derive(Default)]
pub struct CrosstermEventPolling {
    filter: TerminalInputFilter,
}

impl EventPolling for &mut CrosstermEventPolling {
    type Error = std::io::Error;

    fn poll(self, timeout: Duration) -> Result<impl IntoIterator<Item = Event>, Self::Error> {
        if !event::poll(timeout)? {
            return Ok(self.filter.flush());
        }

        let mut events = self.filter.push(event::read()?)?;
        let mut events_read = 1;
        while events_read < MAX_EVENTS_PER_POLL && event::poll(Duration::ZERO)? {
            events.extend(self.filter.push(event::read()?)?);
            events_read += 1;
        }
        Ok(events)
    }
}

/// Prevent a very high-rate input stream from starving rendering indefinitely.
const MAX_EVENTS_PER_POLL: usize = 1024;

/// Filters terminal protocol bytes that occasionally leak through as key events.
///
/// With mouse capture enabled, terminals encode mouse input as escape sequences. For example, a
/// wheel-up event in SGR mouse mode looks like `ESC [ < 64 ; x ; y M`. Crossterm normally parses
/// that as [`Event::Mouse`], but with very high-rate scrolling some terminals can deliver parts of
/// the sequence such that crossterm reports them as ordinary key presses instead:
/// `Esc`, `[`, `<`, digits, semicolons, and a final `M`.
///
/// Letting those leaked bytes reach normal keybind handling is dangerous: `Esc` can navigate back
/// and the final shifted `M` can trigger unrelated actions. This filter sits at the input boundary
/// and drops only complete leaked SGR mouse sequences. If the buffered input stops matching that
/// shape, or if no more input is available, the original events are replayed so normal keys such as
/// a real `Esc` continue to work.
#[derive(Debug, Default)]
struct TerminalInputFilter {
    pending: Vec<Event>,
}

impl TerminalInputFilter {
    fn push(&mut self, event: Event) -> Result<Vec<Event>, std::io::Error> {
        if self.pending.is_empty() {
            if event_is_escape_key(&event) {
                self.pending.push(event);
                return Ok(Vec::new());
            }

            return Ok(vec![event]);
        }

        self.pending.push(event);
        match classify_sgr_mouse_sequence(&self.pending) {
            SgrMouseSequence::Complete => {
                self.pending.clear();
                Ok(Vec::new())
            }
            SgrMouseSequence::Prefix if self.pending.len() <= MAX_SGR_MOUSE_SEQUENCE_LEN => {
                Ok(Vec::new())
            }
            SgrMouseSequence::Prefix | SgrMouseSequence::NotMatch => Ok(self.flush()),
        }
    }

    fn flush(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.pending)
    }
}

const MAX_SGR_MOUSE_SEQUENCE_LEN: usize = 32;

#[derive(Debug, Copy, Clone)]
enum SgrMouseSequence {
    Prefix,
    Complete,
    NotMatch,
}

fn classify_sgr_mouse_sequence(events: &[Event]) -> SgrMouseSequence {
    let chars = match sgr_mouse_sequence_chars(events) {
        Some(chars) => chars,
        None => return SgrMouseSequence::NotMatch,
    };

    if chars.len() == 1 {
        return SgrMouseSequence::Prefix;
    }
    if chars.get(1) != Some(&'[') {
        return SgrMouseSequence::NotMatch;
    }
    if chars.len() == 2 {
        return SgrMouseSequence::Prefix;
    }
    if chars.get(2) != Some(&'<') {
        return SgrMouseSequence::NotMatch;
    }

    let mut groups = 0;
    let mut digits_in_group = 0;
    for ch in chars.iter().skip(3).copied() {
        match ch {
            '0'..='9' => digits_in_group += 1,
            ';' => {
                if digits_in_group == 0 || groups == 2 {
                    return SgrMouseSequence::NotMatch;
                }
                groups += 1;
                digits_in_group = 0;
            }
            'M' | 'm' => {
                return if groups == 2 && digits_in_group > 0 {
                    SgrMouseSequence::Complete
                } else {
                    SgrMouseSequence::NotMatch
                };
            }
            _ => return SgrMouseSequence::NotMatch,
        }
    }

    SgrMouseSequence::Prefix
}

fn sgr_mouse_sequence_chars(events: &[Event]) -> Option<Vec<char>> {
    let mut chars = Vec::with_capacity(events.len());
    for event in events {
        let Event::Key(key) = event else {
            return None;
        };
        let ch = match key.code {
            KeyCode::Esc => '\u{1b}',
            KeyCode::Char(ch) => ch,
            _ => return None,
        };
        chars.push(ch);
    }
    Some(chars)
}

fn event_is_escape_key(event: &Event) -> bool {
    matches!(event, Event::Key(key) if key.code == KeyCode::Esc)
}

/// An [`EventPolling`] implementation that never yields events.
///
/// This is used for non-interactive runs where touching terminal input can stop the process when
/// profilers launch the target in a background process group.
#[derive(Copy, Clone)]
pub struct NoopEventPolling;

impl EventPolling for NoopEventPolling {
    type Error = Infallible;

    fn poll(self, _timeout: Duration) -> Result<impl IntoIterator<Item = Event>, Self::Error> {
        Ok(None)
    }
}

impl EventPolling for &mut NoopEventPolling {
    type Error = Infallible;

    fn poll(self, _timeout: Duration) -> Result<impl IntoIterator<Item = Event>, Self::Error> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{
        KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseEvent, MouseEventKind,
    };

    use super::*;

    #[test]
    fn drops_leaked_sgr_mouse_sequence() {
        let mut filter = TerminalInputFilter::default();
        let mut output = Vec::new();
        for event in sgr_mouse_sequence() {
            output.extend(filter.push(event).expect("filter is infallible"));
        }
        output.extend(filter.flush());

        assert!(
            output.is_empty(),
            "leaked mouse protocol bytes should not reach keybind handling"
        );
    }

    #[test]
    fn replays_events_when_sequence_is_not_sgr_mouse() {
        let mut filter = TerminalInputFilter::default();

        assert!(
            filter
                .push(key(KeyCode::Esc))
                .expect("filter is infallible")
                .is_empty(),
            "escape is buffered until we know whether it starts a mouse sequence"
        );
        let output = filter
            .push(key(KeyCode::Char('x')))
            .expect("filter is infallible");

        assert_eq!(
            output,
            vec![key(KeyCode::Esc), key(KeyCode::Char('x'))],
            "non-mouse input should be replayed unchanged"
        );
    }

    #[test]
    fn flushes_partial_sequence_when_no_more_input_is_available() {
        let mut filter = TerminalInputFilter::default();

        assert!(
            filter
                .push(key(KeyCode::Esc))
                .expect("filter is infallible")
                .is_empty(),
            "escape is initially buffered"
        );

        assert_eq!(
            filter.flush(),
            vec![key(KeyCode::Esc)],
            "a real escape key should be emitted when no mouse sequence follows"
        );
    }

    #[test]
    fn passes_non_key_events_through() {
        let mut filter = TerminalInputFilter::default();
        let event = Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 10,
            row: 20,
            modifiers: KeyModifiers::NONE,
        });

        assert_eq!(
            filter.push(event.clone()).expect("filter is infallible"),
            vec![event],
            "already parsed mouse events should pass through"
        );
    }

    fn sgr_mouse_sequence() -> impl IntoIterator<Item = Event> {
        [
            key(KeyCode::Esc),
            key(KeyCode::Char('[')),
            key(KeyCode::Char('<')),
            key(KeyCode::Char('6')),
            key(KeyCode::Char('4')),
            key(KeyCode::Char(';')),
            key(KeyCode::Char('1')),
            key(KeyCode::Char('6')),
            key(KeyCode::Char('9')),
            key(KeyCode::Char(';')),
            key(KeyCode::Char('2')),
            key(KeyCode::Char('2')),
            key(KeyCode::Char('M')),
        ]
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
    }
}
