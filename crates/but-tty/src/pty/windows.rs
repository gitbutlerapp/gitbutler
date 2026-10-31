//! The user's shell on a Windows pseudoconsole, with what it writes read on its own thread.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use std::{mem, ptr, sync, thread};

use anyhow::Context as _;
use parking_lot::Mutex;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Console::{COORD, CreatePseudoConsole, HPCON, ResizePseudoConsole};
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::{
    CREATE_UNICODE_ENVIRONMENT, CreateProcessW, DeleteProcThreadAttributeList,
    EXTENDED_STARTUPINFO_PRESENT, INFINITE, InitializeProcThreadAttributeList,
    LPPROC_THREAD_ATTRIBUTE_LIST, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, PROCESS_INFORMATION,
    STARTF_USESTDHANDLES, STARTUPINFOEXW, STARTUPINFOW, UpdateProcThreadAttribute,
    WaitForSingleObject,
};
use windows::core::{Owned, PCWSTR, PWSTR};

use super::{Output, PAUSE_MS, Queue, Size, queue};

/// Closed by whichever comes first, the shell exiting or the [`Pty`] dropping, and never while
/// the lock is held, as closing can wait for the reader to drain what the shell wrote last.
type Console = Arc<Mutex<Option<Owned<HPCON>>>>;

/// A shell running on a pseudoconsole, hung up when dropped.
pub struct Pty {
    /// What to send the shell, written on a thread of its own, as a shell that isn't reading
    /// leaves a write waiting.
    input: sync::mpsc::Sender<Vec<u8>>,
    console: Console,
}

impl Size {
    fn coord(self) -> COORD {
        COORD {
            X: i16::try_from(self.cols).unwrap_or(i16::MAX),
            Y: i16::try_from(self.rows).unwrap_or(i16::MAX),
        }
    }
}

impl Pty {
    /// Starts the user's shell (%COMSPEC%, else cmd.exe) in `cwd`, sized `size`.
    /// The receiver yields what the shell writes, in chunks, and ends once it exits, which
    /// closes the pseudoconsole on the jobs it left.
    pub fn spawn(cwd: &Path, size: Size) -> anyhow::Result<(Pty, Output)> {
        let (input_read, input_write) = pipe().context("Couldn't open the shell's input")?;
        let (output_read, output_write) = pipe().context("Couldn't open the shell's output")?;
        let console =
            open(size, &input_read, &output_write).context("Couldn't open a pseudoconsole")?;
        drop((input_read, output_write));
        let (queue, output) = queue();
        let ended = queue.clone();
        thread::Builder::new()
            .name("pty reader".into())
            .spawn(move || drain(output_read, queue))?;

        let shell = shell();
        let process = start(&shell, cwd, &console)
            .with_context(|| format!("Couldn't start {}", shell.display()))?;
        let console = Arc::new(Mutex::new(Some(console)));

        let (input, queued) = sync::mpsc::channel::<Vec<u8>>();
        thread::Builder::new()
            .name("pty writer".into())
            .spawn(move || {
                for bytes in queued {
                    if (&input_write).write_all(&bytes).is_err() {
                        return;
                    }
                }
            })?;

        let waited = console.clone();
        thread::Builder::new()
            .name("pty waiter".into())
            .spawn(move || {
                // SAFETY: the process handle is open while `process` is.
                unsafe { WaitForSingleObject(raw(&process), INFINITE) };
                let closing = waited.lock().take();
                drop(closing);
                ended.close();
            })?;

        Ok((Pty { input, console }, output))
    }

    // ponytail: a pseudoconsole doesn't say who's in front, so a program that dies blocked shows
    // blocked until the terminal closes; walk the console's processes if that's seen.
    pub fn shell_in_front(&self) -> bool {
        false
    }

    /// Sends input to the shell, once it reads what was sent before.
    pub fn write(&self, bytes: &[u8]) -> io::Result<()> {
        (self.input.send(bytes.to_vec())).map_err(|_| io::ErrorKind::BrokenPipe.into())
    }

    pub fn resize(&self, size: Size) -> io::Result<()> {
        let console = self.console.lock();
        let console = console.as_ref().ok_or(io::ErrorKind::BrokenPipe)?;
        // SAFETY: the pseudoconsole is open while the lock is held.
        unsafe { ResizePseudoConsole(**console, size.coord()) }?;
        Ok(())
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        let Some(console) = self.console.lock().take() else {
            return;
        };
        // Closing it ends the shell and what it runs, as a terminal hanging up does.
        thread::Builder::new()
            .name("pty closer".into())
            .spawn(move || drop(console))
            .ok();
    }
}

fn pipe() -> io::Result<(File, File)> {
    let (mut read, mut write) = (HANDLE(ptr::null_mut()), HANDLE(ptr::null_mut()));
    // SAFETY: both pointers are to live locals.
    unsafe { CreatePipe(&raw mut read, &raw mut write, None, 0) }?;
    // SAFETY: `CreatePipe` succeeded, so both are open handles nothing else owns.
    let (read, write) = unsafe {
        (
            OwnedHandle::from_raw_handle(read.0),
            OwnedHandle::from_raw_handle(write.0),
        )
    };
    Ok((File::from(read), File::from(write)))
}

fn open(size: Size, input: &File, output: &File) -> io::Result<Owned<HPCON>> {
    // SAFETY: both handles are open for the call, and the pseudoconsole keeps its own.
    let console = unsafe { CreatePseudoConsole(size.coord(), raw(input), raw(output), 0) }?;
    // SAFETY: `CreatePseudoConsole` succeeded, so it's open and nothing else owns it.
    Ok(unsafe { Owned::new(console) })
}

fn shell() -> OsString {
    std::env::var_os("COMSPEC")
        .filter(|shell| !shell.is_empty())
        .unwrap_or_else(|| "cmd.exe".into())
}

fn start(shell: &OsStr, cwd: &Path, console: &HPCON) -> io::Result<OwnedHandle> {
    let mut command_line = command_line(shell)?;
    let cwd = wide(cwd.as_os_str())?;
    let environment = environment();
    let mut attributes = Attributes::pseudoconsole(*console)?;
    let startup = STARTUPINFOEXW {
        StartupInfo: STARTUPINFOW {
            cb: mem::size_of::<STARTUPINFOEXW>() as u32,
            // With no handles of its own given, the shell takes the pseudoconsole's rather than
            // this process's.
            dwFlags: STARTF_USESTDHANDLES,
            ..Default::default()
        },
        lpAttributeList: attributes.list(),
    };
    let mut process = PROCESS_INFORMATION::default();
    // SAFETY: each pointer is to a live local; the strings end in a NUL and the environment
    // block in two.
    unsafe {
        CreateProcessW(
            PCWSTR::null(),
            Some(PWSTR(command_line.as_mut_ptr())),
            None,
            None,
            false,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
            Some(environment.as_ptr().cast()),
            PCWSTR(cwd.as_ptr()),
            (&raw const startup).cast::<STARTUPINFOW>(),
            &raw mut process,
        )
    }?;
    // SAFETY: `CreateProcessW` succeeded, so both are open handles nothing else owns.
    let (process, thread) = unsafe {
        (
            OwnedHandle::from_raw_handle(process.hProcess.0),
            OwnedHandle::from_raw_handle(process.hThread.0),
        )
    };
    drop(thread);
    Ok(process)
}

/// The program, quoted as Windows reads a command line's first word: up to the next quote, with
/// no escapes.
fn command_line(program: &OsStr) -> io::Result<Vec<u16>> {
    if program.encode_wide().any(|unit| unit == u16::from(b'"')) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a program's path can't hold a quote",
        ));
    }
    let mut quoted = OsString::from("\"");
    quoted.push(program);
    quoted.push("\"");
    wide(&quoted)
}

/// This process's environment as a terminal's, sorted by name without regard to case, as
/// `CreateProcessW` wants it.
fn environment() -> Vec<u16> {
    let mut variables: BTreeMap<OsString, (OsString, OsString)> = std::env::vars_os()
        .map(|(name, value)| (name.to_ascii_uppercase(), (name, value)))
        .collect();
    variables.remove(OsStr::new("TERMINFO"));
    for (name, value) in [
        ("TERM", "xterm-256color"),
        ("COLORTERM", "truecolor"),
        // Ghostty's, the emulator's, as Claude Code reports progress only to terminals it knows.
        ("TERM_PROGRAM", "ghostty"),
        ("TERM_PROGRAM_VERSION", "1.3.2"),
    ] {
        variables.insert(name.into(), (name.into(), value.into()));
    }
    let mut block = Vec::new();
    for (name, value) in variables.into_values() {
        block.extend(name.encode_wide());
        block.push(u16::from(b'='));
        block.extend(value.encode_wide());
        block.push(0);
    }
    block.push(0);
    block
}

fn wide(text: &OsStr) -> io::Result<Vec<u16>> {
    let mut wide: Vec<u16> = text.encode_wide().collect();
    if wide.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a path or argument can't hold a NUL",
        ));
    }
    wide.push(0);
    Ok(wide)
}

/// A process's attributes, naming only the pseudoconsole it attaches to.
struct Attributes(Vec<usize>);

impl Attributes {
    fn pseudoconsole(console: HPCON) -> io::Result<Attributes> {
        let mut size = 0;
        // SAFETY: with no list, it only writes the size one needs to `size`.
        unsafe { InitializeProcThreadAttributeList(None, 1, None, &raw mut size) }.ok();
        let mut list = vec![0; size.div_ceil(mem::size_of::<usize>())];
        // SAFETY: `list` holds at least `size` bytes, aligned as a pointer.
        unsafe {
            InitializeProcThreadAttributeList(
                Some(LPPROC_THREAD_ATTRIBUTE_LIST(list.as_mut_ptr().cast())),
                1,
                None,
                &raw mut size,
            )
        }?;
        let mut attributes = Attributes(list);
        // SAFETY: the list has room for this one attribute, whose value is the pseudoconsole's
        // handle itself rather than a pointer to it.
        unsafe {
            UpdateProcThreadAttribute(
                attributes.list(),
                0,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                Some(ptr::without_provenance(console.0.cast_unsigned())),
                mem::size_of::<HPCON>(),
                None,
                None,
            )
        }?;
        Ok(attributes)
    }

    fn list(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        LPPROC_THREAD_ATTRIBUTE_LIST(self.0.as_mut_ptr().cast())
    }
}

impl Drop for Attributes {
    fn drop(&mut self) {
        // SAFETY: the list was initialized, and is deleted only here.
        unsafe { DeleteProcThreadAttributeList(self.list()) };
    }
}

/// Reads until the pseudoconsole closes, even once no one's listening, as closing it can wait
/// until what the shell wrote last is read.
fn drain(mut output: File, queue: Queue) {
    let mut buffer = vec![0; 64 * 1024];
    loop {
        while queue.full() {
            thread::sleep(Duration::from_millis(PAUSE_MS.into()));
        }
        match output.read(&mut buffer) {
            Ok(0) => return,
            Ok(n) => {
                let read = buffer.get(..n).unwrap_or_default().to_vec();
                queue.send(read).ok();
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return,
        }
    }
}

fn raw(handle: &impl AsRawHandle) -> HANDLE {
    HANDLE(handle.as_raw_handle())
}
