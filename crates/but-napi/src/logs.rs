//! File-based tracing for the host application, mirroring the desktop app's
//! log conventions (daily-rotated `GitButler.<date>.log` files).

use std::sync::Mutex;

use anyhow::Context as _;
use napi_derive::napi;
use tracing::{Level, metadata::LevelFilter};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{Layer, filter::filter_fn, fmt::format::FmtSpan, layer::SubscriberExt};

/// Keeps the non-blocking writer flushing until [`shutdown_tracing`] drops it.
static WRITER_GUARD: Mutex<Option<WorkerGuard>> = Mutex::new(None);

/// Flush and stop the log file writer. Call once at application shutdown:
/// destructors of process statics never run at exit, so without this the most
/// recently buffered log lines would be lost.
#[napi]
pub fn shutdown_tracing() {
    if let Ok(mut guard) = WRITER_GUARD.lock() {
        guard.take();
    }
}

/// Initialize tracing for the process, writing daily-rotated `GitButler.<date>.log`
/// files into the platform's log directory for the given bundle `identifier`
/// (see `but_path::app_log_dir_for_identifier`), keeping at most 14 of them.
/// Returns the resolved log directory so the host can write its own logs there too.
///
/// Verbosity comes from the `LOG_LEVEL` environment variable (default `info`).
/// With `also_to_stderr`, logs are additionally written to stderr, which is useful
/// during development. Fails if a global tracing subscriber is already installed.
#[napi]
pub fn init_tracing(identifier: String, also_to_stderr: bool) -> napi::Result<String> {
    init(&identifier, also_to_stderr).map_err(|err| napi::Error::from_reason(format!("{err:#}")))
}

fn init(identifier: &str, also_to_stderr: bool) -> anyhow::Result<String> {
    let log_dir = but_path::app_log_dir_for_identifier(identifier)?;
    let log_dir = log_dir.as_path();
    std::fs::create_dir_all(log_dir)
        .with_context(|| format!("failed to create log dir at '{}'", log_dir.display()))?;

    // The appender prunes files beyond `max_log_files` both when built and when rotating.
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .max_log_files(14)
        .filename_prefix("GitButler")
        .filename_suffix("log")
        .build(log_dir)
        .context("failed to initialize the rolling file appender")?;
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
    if let Ok(mut slot) = WRITER_GUARD.lock() {
        *slot = Some(guard);
    }

    let log_level = std::env::var("LOG_LEVEL")
        .unwrap_or_default()
        .parse::<LevelFilter>()
        .unwrap_or(LevelFilter::INFO)
        .into_level();
    let filter = filter_fn(move |meta| should_log(log_level, meta));

    let format = tracing_subscriber::fmt::format()
        .with_file(true)
        .with_line_number(true)
        .with_target(false)
        .compact();

    let file_layer = tracing_subscriber::fmt::layer()
        .event_format(format.clone())
        .with_ansi(false)
        .with_span_events(FmtSpan::CLOSE)
        .with_writer(file_writer)
        .with_filter(filter.clone());
    let stderr_layer = also_to_stderr.then(|| {
        tracing_subscriber::fmt::layer()
            .event_format(format)
            .with_writer(std::io::stderr)
            .with_filter(filter)
    });

    let subscriber = tracing_subscriber::registry()
        .with(file_layer)
        .with(stderr_layer);
    tracing::subscriber::set_global_default(subscriber)
        .context("failed to install the global tracing subscriber")?;
    Ok(log_dir.to_string_lossy().into_owned())
}

/// Like `LevelFilter`, but unless `trace` is requested it only admits events from
/// GitButler's own crates, so third-party noise doesn't drown the log file. Same
/// policy as the desktop app and the `but` CLI.
fn should_log(level: Option<Level>, meta: &tracing::Metadata<'_>) -> bool {
    let Some(level) = level else {
        return false;
    };
    if *meta.level() > level {
        return false;
    }
    if level > Level::DEBUG {
        return true;
    }
    meta.module_path().is_none_or(|p| {
        p.starts_with("gitbutler_") || p.starts_with("but::") || p.starts_with("but_")
    })
}
