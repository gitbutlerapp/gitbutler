// NOTE(qix-): Cargo doesn't let us specify binaries based on the platform,
// NOTE(qix-): unfortunately. This utility is not used on Windows but is
// NOTE(qix-): build anyway. We'll address this at a later time.
// NOTE(qix-):
// NOTE(qix-): For now, we just stub out the main function on windows and panic.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(unix)]
include!("setsid/unix.rs");
#[cfg(windows)]
include!("setsid/windows.rs");
