// Note that this file contains substantial portions of code
// from https://github.com/notify-rs/notify/blob/main/notify-types/src/debouncer_full.rs,
// and what follows is a reproduction of its license.
//
// Copyright (c) 2023 Notify Contributors
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use std::ops::{Deref, DerefMut};

#[cfg(test)]
use mock_instant::Instant;

#[cfg(not(test))]
use std::time::Instant;

use notify::Event;

/// A debounced event is emitted after a short delay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebouncedEvent {
    /// The original event.
    pub event: Event,

    /// The time at which the event occurred.
    pub time: Instant,
}

impl DebouncedEvent {
    pub fn new(event: Event, time: Instant) -> Self {
        Self { event, time }
    }
}

impl Deref for DebouncedEvent {
    type Target = Event;

    fn deref(&self) -> &Self::Target {
        &self.event
    }
}

impl DerefMut for DebouncedEvent {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.event
    }
}

impl Default for DebouncedEvent {
    fn default() -> Self {
        Self {
            event: Default::default(),
            time: Instant::now(),
        }
    }
}

impl From<Event> for DebouncedEvent {
    fn from(event: Event) -> Self {
        Self {
            event,
            time: Instant::now(),
        }
    }
}
