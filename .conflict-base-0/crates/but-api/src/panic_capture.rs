//! Panic capture helpers used when converting unwind payloads into `anyhow::Error`.
//!
//! The application should call [`install_panic_hook()`] once at startup.
//!
//! Then, if an unwind happens, the hook stores panic metadata in `LAST_PANIC_INFO`, and
//! `panic_payload_to_anyhow()` combines that snapshot with the unwind payload to
//! produce a detailed error.

use std::{
    any::Any,
    cell::RefCell,
    panic::{self, PanicHookInfo},
    sync::Once,
};

/// Snapshot of panic metadata captured by the installed hook.
#[derive(Debug)]
struct PanicInfoSnapshot {
    /// Message extracted from the panic payload, when the payload is a string type.
    payload_message: Option<String>,
    /// Source location in `file:line:column` format, when provided by the runtime.
    location: Option<String>,
    /// Name of the thread that panicked, or `"<unnamed>"` if no name exists.
    thread_name: String,
    /// Debug-formatted thread identifier for disambiguating worker threads.
    thread_id: String,
    /// The backtrace captured at panic-hook time on the panicking thread.
    backtrace: String,
}

thread_local! {
    /// Most recent panic snapshot for the current thread.
    ///
    /// This is cleared when the information is consumed via [`panic_payload_to_anyhow()`].
    static LAST_PANIC_INFO: RefCell<Option<PanicInfoSnapshot>> = const { RefCell::new(None) };
}

/// Ensures panic-hook installation happens exactly once per process.
static INSTALL_PANIC_HOOK: Once = Once::new();

/// Converts a caught panic payload into an `anyhow::Error`.
///
/// The error prefers metadata captured by the panic hook (panic message, location,
/// thread info, and panic-time backtrace). If no snapshot is available, it falls
/// back to payload-derived message plus a backtrace captured during conversion.
///
/// Assumes that the panic hook has been installed via [`install_panic_hook()`], or
/// else the panic message will not have a backtrace.
pub(crate) fn panic_payload_to_anyhow(
    function_name: &'static str,
    panic_payload: Box<dyn Any + Send + 'static>,
) -> anyhow::Error {
    let payload_message = panic_payload_message(panic_payload.as_ref());
    let snapshot = LAST_PANIC_INFO.with(|slot| slot.borrow_mut().take());

    if let Some(snapshot) = snapshot {
        let panic_message = snapshot
            .payload_message
            .as_deref()
            .or(payload_message.as_deref())
            .unwrap_or("panic payload was not a string");
        let panic_location = snapshot.location.as_deref().unwrap_or("<unknown location>");
        anyhow::anyhow!(
            "panic while executing `{}` on thread '{}' ({}) at {}: {}\n\npanic backtrace:\n{}",
            function_name,
            snapshot.thread_name,
            snapshot.thread_id,
            panic_location,
            panic_message,
            snapshot.backtrace
        )
    } else {
        let panic_message = payload_message
            .as_deref()
            .unwrap_or("panic payload was not a string");
        anyhow::anyhow!(
            "panic while executing `{}`: {}\n\npanic backtrace unavailable from hook; conversion backtrace:\n{}",
            function_name,
            panic_message,
            std::backtrace::Backtrace::force_capture()
        )
    }
}

/// Installs a hook that captures panic context and then delegates to the previous hook.
/// Can be called multiple times, but only the first call will install the hook.
pub fn install_panic_hook() {
    INSTALL_PANIC_HOOK.call_once(|| {
        let previous_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            capture_panic_info(panic_info);
            previous_hook(panic_info);
        }));
    });
}

/// Captures panic metadata from `panic_info` into thread-local storage.
fn capture_panic_info(panic_info: &PanicHookInfo<'_>) {
    let thread = std::thread::current();
    let snapshot = PanicInfoSnapshot {
        payload_message: panic_payload_message(panic_info.payload()),
        location: panic_info
            .location()
            .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column())),
        thread_name: thread.name().unwrap_or("<unnamed>").to_owned(),
        thread_id: format!("{:?}", thread.id()),
        backtrace: std::backtrace::Backtrace::force_capture().to_string(),
    };
    LAST_PANIC_INFO.with(|slot| {
        *slot.borrow_mut() = Some(snapshot);
    });
}

/// Extracts a human-readable panic payload message when the payload is string-like.
///
/// Returns `Some(String)` for `&'static str` and `String` payloads, otherwise `None`.
fn panic_payload_message(payload: &(dyn Any + Send)) -> Option<String> {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        return Some((*message).to_owned());
    }
    if let Some(message) = payload.downcast_ref::<String>() {
        return Some(message.clone());
    }
    None
}
