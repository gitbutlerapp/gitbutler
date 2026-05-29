use std::{
    io::{self, Write},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

#[derive(Clone, Copy, Debug)]
pub(crate) enum KeyboardMode {
    Kitty,
    ModifyOtherKeys,
}

static ACTIVE_KEYBOARD_MODE: Mutex<Option<KeyboardMode>> = Mutex::new(None);
static TERMINAL_INPUT_SUSPENDED: AtomicBool = AtomicBool::new(false);

pub(crate) fn set_active_mode(mode: KeyboardMode) {
    if let Ok(mut active_mode) = ACTIVE_KEYBOARD_MODE.lock() {
        *active_mode = Some(mode);
    }
}

pub(crate) fn clear_active_mode() {
    if let Ok(mut active_mode) = ACTIVE_KEYBOARD_MODE.lock() {
        *active_mode = None;
    }
}

pub(crate) fn restore_active_mode() {
    let mode = ACTIVE_KEYBOARD_MODE.lock().ok().and_then(|mode| *mode);
    if let Some(mode) = mode {
        let _ = restore_mode(mode);
        clear_active_mode();
    }
}

pub(crate) fn suspend_active_mode() {
    TERMINAL_INPUT_SUSPENDED.store(true, Ordering::Release);
    // Give the raw stdin polling thread a chance to observe the suspension before handing stdin
    // to an external program like $EDITOR. This avoids racing the editor for its first bytes.
    thread::sleep(Duration::from_millis(20));

    let mode = ACTIVE_KEYBOARD_MODE.lock().ok().and_then(|mode| *mode);
    if let Some(mode) = mode {
        let _ = restore_mode(mode);
    }
}

pub(crate) fn resume_active_mode() {
    let mode = ACTIVE_KEYBOARD_MODE.lock().ok().and_then(|mode| *mode);
    if let Some(mode) = mode {
        let _ = enable_mode(mode);
    }
    TERMINAL_INPUT_SUSPENDED.store(false, Ordering::Release);
}

pub(crate) fn terminal_input_suspended() -> bool {
    TERMINAL_INPUT_SUSPENDED.load(Ordering::Acquire)
}

pub(crate) fn query_kitty_keyboard_protocol() -> io::Result<()> {
    write_stdout(b"\x1b[?u")
}

pub(crate) fn enable_mode(mode: KeyboardMode) -> io::Result<()> {
    match mode {
        KeyboardMode::Kitty => write_stdout(b"\x1b[>7u"),
        KeyboardMode::ModifyOtherKeys => write_stdout(b"\x1b[>4;2m"),
    }
}

fn restore_mode(mode: KeyboardMode) -> io::Result<()> {
    match mode {
        KeyboardMode::Kitty => write_stdout(b"\x1b[<u"),
        KeyboardMode::ModifyOtherKeys => write_stdout(b"\x1b[>4;0m"),
    }
}

fn write_stdout(bytes: &[u8]) -> io::Result<()> {
    let mut stdout = io::stdout();
    stdout.write_all(bytes)?;
    stdout.flush()
}
