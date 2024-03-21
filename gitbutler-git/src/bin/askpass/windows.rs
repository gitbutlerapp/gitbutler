use std::{
    io,
    os::windows::io::{AsRawHandle, FromRawHandle},
    time::Duration,
};
use windows_named_pipe::PipeStream;

pub fn establish(sock_path: &str) -> PipeStream {
    PipeStream::connect(sock_path).unwrap()
}

/// There are some methods we need in order to run askpass correctly,
/// and those methods are not available out of the box on windows.
/// We stub them using this trait so we don't have to newtype
/// the pipestream itself (which would be extensive and un-DRY).
pub trait UnixCompatibility: Sized {
    fn try_clone(&self) -> Option<Self>;
    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()>;
}

impl UnixCompatibility for PipeStream {
    fn try_clone(&self) -> Option<Self> {
        Some(unsafe { Self::from_raw_handle(self.as_raw_handle()) })
    }

    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        // NOTE(qix-): Technically, this shouldn't work (and probably doesn't).
        // NOTE(qix-): The documentation states:
        // NOTE(qix-):
        // NOTE(qix-): > This parameter must be NULL if . . . client and server
        // NOTE(qix-): > processes are on the same computer.
        // NOTE(qix-):
        // NOTE(qix-): This is indeed the case here, but we try to make it work
        // NOTE(qix-): anyway.
        #[allow(unused_assignments)]
        let mut timeout_ms: winapi::shared::minwindef::DWORD = 0;
        let timeout_ptr: winapi::shared::minwindef::LPDWORD = if let Some(timeout) = timeout {
            timeout_ms = timeout.as_millis() as winapi::shared::minwindef::DWORD;
            &mut timeout_ms as *mut _
        } else {
            std::ptr::null_mut()
        };

        let r = unsafe {
            winapi::um::namedpipeapi::SetNamedPipeHandleState(
                self.as_raw_handle() as winapi::um::winnt::HANDLE,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                timeout_ptr,
            )
        };

        if r == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}
