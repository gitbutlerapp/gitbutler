use std::{convert::Infallible, time::Duration};

use crossterm::event::{self, Event};

/// Trait for abstracting event polling so we can hardcode events in tests.
pub(super) trait EventPolling {
    type Error: std::error::Error + Send + Sync + 'static;

    fn poll(self, timeout: Duration) -> Result<impl IntoIterator<Item = Event>, Self::Error>;
}

/// An [`EventPolling`] implementation that polls events for real using crossterm.
#[derive(Copy, Clone)]
pub(super) struct CrosstermEventPolling;

impl EventPolling for CrosstermEventPolling {
    type Error = std::io::Error;

    fn poll(self, timeout: Duration) -> Result<impl IntoIterator<Item = Event>, Self::Error> {
        if event::poll(timeout)? {
            Ok(Some(event::read()?))
        } else {
            Ok(None)
        }
    }
}

/// An [`EventPolling`] implementation that never yields events.
///
/// This is used for non-interactive runs where touching terminal input can stop the process when
/// profilers launch the target in a background process group.
#[derive(Copy, Clone)]
pub(super) struct NoopEventPolling;

impl EventPolling for NoopEventPolling {
    type Error = Infallible;

    fn poll(self, _timeout: Duration) -> Result<impl IntoIterator<Item = Event>, Self::Error> {
        Ok(None)
    }
}
