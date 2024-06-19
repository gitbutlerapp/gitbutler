use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, IntoRawHandle, RawHandle};
use std::path::Path;
use windows::core::PWSTR;
use windows::Win32::Foundation::{
    ERROR_PIPE_NOT_CONNECTED, GENERIC_READ, GENERIC_WRITE, HANDLE, WIN32_ERROR,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FlushFileBuffers, ReadFile, WriteFile, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ,
    FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::Pipes::{WaitNamedPipeW, NMPWAIT_USE_DEFAULT_WAIT};

#[derive(Debug)]
struct Handle {
    inner: HANDLE,
}

unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

#[derive(Debug)]
pub struct Pipe {
    handle: Handle,
}

impl Pipe {
    pub fn connect(path: &Path) -> io::Result<Pipe> {
        let mut os_str: OsString = path.as_os_str().into();
        os_str.push("\x00");
        let mut wide_path: Vec<u16> = os_str.encode_wide().collect();

        let pwstr_path = PWSTR(wide_path.as_mut_ptr());
        let _ = unsafe { WaitNamedPipeW(pwstr_path, NMPWAIT_USE_DEFAULT_WAIT) };
        let handle_res = unsafe {
            CreateFileW(
                pwstr_path,
                GENERIC_READ.0 | GENERIC_WRITE.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        };

        match handle_res {
            Ok(handle) => Ok(Pipe {
                handle: Handle { inner: handle },
            }),
            Err(err) => Err(io::Error::from_raw_os_error(err.code().0)),
        }
    }

    pub fn get_handle(&self) -> HANDLE {
        self.handle.inner
    }
}

impl Drop for Pipe {
    fn drop(&mut self) {
        let _ = unsafe { FlushFileBuffers(self.handle.inner) };
    }
}

impl Read for Pipe {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut bytes_read = 0u32;
        let res = unsafe { ReadFile(self.handle.inner, Some(buf), Some(&mut bytes_read), None) };
        match res {
            Ok(_) => Ok(bytes_read as usize),
            Err(err) => match WIN32_ERROR::from_error(&err) {
                Some(ERROR_PIPE_NOT_CONNECTED) => Ok(0),
                _ => Err(io::Error::from_raw_os_error(err.code().0)),
            },
        }
    }
}

impl Write for Pipe {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut bytes_written = 0u32;

        let res =
            unsafe { WriteFile(self.handle.inner, Some(buf), Some(&mut bytes_written), None) };

        match res {
            Ok(_) => Ok(bytes_written as usize),
            Err(err) => Err(io::Error::from_raw_os_error(err.code().0)),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        let res = unsafe { FlushFileBuffers(self.handle.inner) };

        match res {
            Ok(_) => Ok(()),
            Err(err) => Err(io::Error::from_raw_os_error(err.code().0)),
        }
    }
}

impl AsRawHandle for Pipe {
    fn as_raw_handle(&self) -> RawHandle {
        self.handle.inner.0 as RawHandle
    }
}

impl IntoRawHandle for Pipe {
    fn into_raw_handle(self) -> RawHandle {
        self.handle.inner.0 as RawHandle
    }
}

impl FromRawHandle for Pipe {
    unsafe fn from_raw_handle(handle: RawHandle) -> Self {
        let handle = HANDLE(handle as isize);
        Pipe {
            handle: Handle { inner: handle },
        }
    }
}
