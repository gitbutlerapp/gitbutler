//! Implement the file-monitoring agent that informs about changes in interesting locations.
#![deny(unsafe_code)]
#![allow(clippy::doc_markdown, clippy::missing_errors_doc)]

mod events;

pub use events::InternalEvent;
mod file_monitor;
pub use file_monitor::spawn;
pub use file_monitor::{FETCH_HEAD, GB_FLUSH, HEAD, HEAD_ACTIVITY, INDEX, LOCAL_REFS_DIR};
