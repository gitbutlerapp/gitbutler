use std::{fs, net::Ipv4Addr, path::Path, time::Duration};

use tauri::{AppHandle, Manager};
use tracing::{instrument, metadata::LevelFilter, subscriber::set_global_default};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{Layer, fmt::format::FmtSpan, layer::SubscriberExt};

pub fn init(app_handle: &AppHandle, performance_logging: bool) {
    let logs_dir = app_handle
        .path()
        .app_log_dir()
        .expect("failed to get logs dir");
    fs::create_dir_all(&logs_dir).expect("failed to create logs dir");

    let log_prefix = "GitButler";
    let log_suffix = "log";
    let max_log_files = 14;
    remove_old_logs(&logs_dir).ok();
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .max_log_files(max_log_files)
        .filename_prefix(log_prefix)
        .filename_suffix(log_suffix)
        .build(&logs_dir)
        .expect("initializing rolling file appender failed");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
    // As the file-writer only checks `max_log_files` on file rotation, it bascially never happens.
    // Run it now.
    prune_old_logs(&logs_dir, Some(log_prefix), Some(log_suffix), max_log_files).ok();

    app_handle.manage(guard); // keep the guard alive for the lifetime of the app

    let format_for_humans = tracing_subscriber::fmt::format()
        .with_file(true)
        .with_line_number(true)
        .with_target(false)
        .compact();

    let log_level_filter = std::env::var("LOG_LEVEL")
        .unwrap_or("info".to_string())
        .to_lowercase()
        .parse()
        .unwrap_or(LevelFilter::INFO);

    let use_colors_in_logs = cfg!(not(feature = "windows"));
    let subscriber = tracing_subscriber::registry()
        .with(
            // subscriber for https://github.com/tokio-rs/console
            console_subscriber::ConsoleLayer::builder()
                .server_addr(get_server_addr(app_handle))
                .retention(Duration::from_secs(3600)) // 1h
                .publish_interval(Duration::from_secs(1))
                .recording_path(logs_dir.join("tokio-console"))
                .spawn(),
        )
        .with(
            // subscriber that writes spans to a file
            tracing_subscriber::fmt::layer()
                .event_format(format_for_humans.clone())
                .with_ansi(false)
                .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
                .with_writer(file_writer)
                .with_filter(log_level_filter),
        );
    if performance_logging {
        set_global_default(
            subscriber.with(
                tracing_forest::ForestLayer::from(
                    tracing_forest::printer::PrettyPrinter::new().writer(std::io::stdout),
                )
                .with_filter(log_level_filter),
            ),
        )
    } else {
        set_global_default(
            subscriber.with(
                // subscriber that writes spans to stdout
                tracing_subscriber::fmt::layer()
                    .event_format(format_for_humans)
                    .with_ansi(use_colors_in_logs)
                    .with_span_events(FmtSpan::CLOSE)
                    .with_filter(log_level_filter),
            ),
        )
    }
    .expect("failed to set subscriber");
}

fn get_server_addr(app_handle: &AppHandle) -> (Ipv4Addr, u16) {
    let config = app_handle.config();
    let product_name = config.product_name.as_ref().expect("product name not set");
    let port = if product_name.eq("GitButler") {
        6667
    } else if product_name.eq("GitButler Nightly") {
        6668
    } else {
        6669
    };
    (Ipv4Addr::LOCALHOST, port)
}

/// Originally based on https://github.com/tokio-rs/tracing/blob/44861cad7a821f08b3c13aba14bb8ddf562b7053/tracing-appender/src/rolling.rs#L571
#[instrument(err(Debug))]
fn prune_old_logs(
    log_directory: &Path,
    log_filename_prefix: Option<&str>,
    log_filename_suffix: Option<&str>,
    max_files: usize,
) -> anyhow::Result<()> {
    let mut files: Vec<_> = {
        let dir = fs::read_dir(log_directory)?;
        dir.filter_map(|entry| {
            let entry = entry.ok()?;
            let metadata = entry.metadata().ok()?;

            // the appender only creates files, not directories or symlinks,
            // so we should never delete a dir or symlink.
            if !metadata.is_file() {
                return None;
            }

            let filename = entry.file_name();
            let filename = filename.to_str()?;
            if let Some(prefix) = log_filename_prefix
                && !filename.starts_with(prefix)
            {
                return None;
            }

            if let Some(suffix) = log_filename_suffix
                && !filename.ends_with(suffix)
            {
                return None;
            }

            let created = metadata.created().ok()?;
            Some((entry, created))
        })
        .collect()
    };

    if files.len() < max_files {
        return Ok(());
    }

    // sort the files by their creation timestamps.
    files.sort_by_key(|(_, created_at)| *created_at);

    // delete files, so that (n-1) files remain, because we will create another log file
    for (file, _) in files.iter().take(files.len() - (max_files - 1)) {
        if let Err(err) = fs::remove_file(file.path()) {
            tracing::warn!(
                "Failed to remove extra log file {}: {}",
                file.path().display(),
                err,
            );
        }
    }

    Ok(())
}

#[instrument(err(Debug))]
fn remove_old_logs(log_directory: &Path) -> anyhow::Result<()> {
    let dir = fs::read_dir(log_directory)?;
    let old_log_files = dir.filter_map(|entry| {
        let entry = entry.ok()?;
        let metadata = entry.metadata().ok()?;

        if !metadata.is_file() {
            return None;
        }

        let filename = entry.file_name();
        let filename = filename.to_str()?;
        if !filename.starts_with("GitButler.log") {
            return None;
        }

        Some(entry.path())
    });
    for file_path in old_log_files {
        if let Err(err) = fs::remove_file(&file_path) {
            tracing::warn!(
                "Failed to remove old log file {}: {}",
                file_path.display(),
                err,
            );
        }
    }

    Ok(())
}
