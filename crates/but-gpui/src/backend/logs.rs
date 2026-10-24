//! The app's log, written to the folder the desktop app writes its own to, in the same daily
//! `GitButler.<date>.log` files, so switching between the two leaves one log to read.

use std::path::PathBuf;

use anyhow::{Context as _, Result};
use parking_lot::Mutex;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::fmt::writer::Tee;

/// Keeps the thread that writes the log running. Dropping it writes out what's queued.
static WRITER: Mutex<Option<WorkerGuard>> = Mutex::new(None);

/// The folder the log files go in, for the build's channel.
pub fn dir() -> Result<PathBuf> {
    but_path::app_log_dir()
}

/// Log to the files and to stderr, both written by a thread of the log's own, so logging never
/// waits on either. `RUST_LOG` picks what's logged, as `but_gpui=debug`. Without it
/// `LOG_LEVEL`, the desktop app's variable, does: GitButler's own crates at that level, `info`
/// by default, and other crates' warnings and errors, as GPUI's own about the GPU or the
/// display. Spans opening and closing aren't logged. Those lines made up two thirds of Lite's
/// log.
pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // `LevelFilter` reads an empty string as `error`.
        let level = std::env::var("LOG_LEVEL")
            .ok()
            .filter(|level| !level.is_empty())
            .and_then(|level| level.parse().ok())
            .unwrap_or(LevelFilter::INFO);
        let rest = level.min(LevelFilter::WARN);
        EnvFilter::new(format!("{rest},gitbutler_={level},but_={level}"))
    });
    let ((writer, guard), failed) = match files() {
        Ok(files) => (
            tracing_appender::non_blocking(Tee::new(files, std::io::stderr())),
            None,
        ),
        Err(err) => (tracing_appender::non_blocking(std::io::stderr()), Some(err)),
    };
    *WRITER.lock() = Some(guard);
    let installed = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .with_file(true)
        .with_line_number(true)
        .with_target(false)
        .compact()
        .try_init();
    if let Err(err) = installed {
        eprintln!("couldn't start logging: {err}");
    }
    if let Some(err) = failed {
        tracing::warn!("logging to stderr only: {err:#}");
    }
    log_panics();
}

/// Write out what's queued and stop writing the log, for when the app is about to end.
pub fn flush() {
    drop(WRITER.lock().take());
}

fn files() -> Result<RollingFileAppender> {
    let dir = dir()?;
    // Before the appender, which looks for old files to remove there first.
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("couldn't create the log folder {}", dir.display()))?;
    RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("GitButler")
        .filename_suffix("log")
        // Counting the desktop app's, which are named the same.
        .max_log_files(14)
        .build(dir)
        .context("couldn't open the log file")
}

/// Log every panic with its backtrace, then run the hook that was there. A panic on the UI
/// thread ends the app, so what's queued is written out first; anywhere else it fails one
/// task, and the app goes on.
fn log_panics() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("unnamed");
        let backtrace = std::backtrace::Backtrace::force_capture();
        tracing::error!("thread '{name}' {info}\n{backtrace}");
        if name == "main" {
            flush();
        }
        previous(info);
    }));
}
