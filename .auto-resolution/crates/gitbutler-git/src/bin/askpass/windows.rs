use std::{io, time::Duration};

use windows::Win32::System::Pipes::SetNamedPipeHandleState;
#[path = "windows-pipe.rs"]
mod windows_pipe;
use windows_pipe::Pipe;

pub fn establish(sock_path: &str) -> Pipe {
    Pipe::connect(std::path::Path::new(sock_path)).unwrap()
}

/// There are some methods we need in order to run askpass correctly,
/// and those methods are not available out of the box on windows.
/// We stub them using this trait so we don't have to newtype
/// the pipestream itself (which would be extensive and un-DRY).
pub trait UnixCompatibility: Sized {
    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()>;
}

impl UnixCompatibility for Pipe {
    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        // NOTE(qix-): Technically, this shouldn't work (and probably doesn't).
        // NOTE(qix-): The documentation states:
        // NOTE(qix-):
        // NOTE(qix-): > This parameter must be NULL if . . . client and server
        // NOTE(qix-): > processes are on the same computer.
        // NOTE(qix-):
        // NOTE(qix-): This is indeed the case here, but we try to make it work
        // NOTE(qix-): anyway.
        let timeout_ms: Option<*const u32> =
            timeout.map(|timeout| timeout.as_millis() as *const u32);

        let r = unsafe { SetNamedPipeHandleState(self.get_handle(), None, None, timeout_ms) };

        match r {
            Ok(_) => Ok(()),
            Err(err) => Err(io::Error::from_raw_os_error(err.code().0)),
        }
    }
}
