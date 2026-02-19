use std::any::Any;
use std::cell::RefCell;
use std::panic::{self, PanicHookInfo};
use std::sync::Once;

#[derive(Debug)]
struct PanicInfoSnapshot {
    payload_message: Option<String>,
    location: Option<String>,
    thread_name: String,
    thread_id: String,
    backtrace: String,
}

thread_local! {
    static LAST_PANIC_INFO: RefCell<Option<PanicInfoSnapshot>> = const { RefCell::new(None) };
}

static INSTALL_PANIC_HOOK: Once = Once::new();

pub(crate) fn prepare_for_call() {
    install_panic_hook();
    LAST_PANIC_INFO.with(|slot| {
        slot.borrow_mut().take();
    });
}

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
        let panic_message = payload_message.as_deref().unwrap_or("panic payload was not a string");
        anyhow::anyhow!(
            "panic while executing `{}`: {}\n\npanic backtrace unavailable from hook; conversion backtrace:\n{}",
            function_name,
            panic_message,
            std::backtrace::Backtrace::force_capture()
        )
    }
}

fn install_panic_hook() {
    INSTALL_PANIC_HOOK.call_once(|| {
        let previous_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            capture_panic_info(panic_info);
            previous_hook(panic_info);
        }));
    });
}

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

fn panic_payload_message(payload: &(dyn Any + Send)) -> Option<String> {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        return Some((*message).to_owned());
    }
    if let Some(message) = payload.downcast_ref::<String>() {
        return Some(message.clone());
    }
    None
}
