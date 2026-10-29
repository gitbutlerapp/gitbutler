//! What a terminal the app opens starts from: the user's shell on a pseudo-terminal, and the
//! colours and size their Ghostty config sets.

mod ghostty;
mod pty;

use std::path::Path;

use futures::channel::mpsc;

pub use ghostty::Ghostty;
pub use pty::{Pty, Size};

/// A shell just started: where to send it input, what it writes, and how to show it.
pub struct Opened {
    pub pty: Pty,
    pub output: mpsc::UnboundedReceiver<Vec<u8>>,
    pub ghostty: Ghostty,
}

/// Start the user's shell in `cwd`, sized `size`, reading their Ghostty config alongside.
pub fn open(cwd: &Path, size: Size) -> anyhow::Result<Opened> {
    let (pty, output) = Pty::spawn(cwd, size)?;
    Ok(Opened {
        pty,
        output,
        ghostty: Ghostty::load(),
    })
}
