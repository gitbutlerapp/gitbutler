use std::{fs, net::Ipv4Addr, time::Duration};

use tauri::{AppHandle, Manager};
use tracing::{metadata::LevelFilter, subscriber::set_global_default};
use tracing_subscriber::{fmt::format::FmtSpan, layer::SubscriberExt, Layer};

pub fn init(app_handle: &AppHandle) {
    let logs_dir = app_handle
        .path_resolver()
        .app_log_dir()
        .expect("failed to get app log dir");
    fs::create_dir_all(&logs_dir).expect("failed to create logs dir");

    let file_appender = tracing_appender::rolling::never(&logs_dir, "GitButler.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
    app_handle.manage(guard); // keep the guard alive for the lifetime of the app

    let format_for_humans = tracing_subscriber::fmt::format()
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(false)
        .compact();

    let filter_for_humans = LevelFilter::DEBUG;

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
            // subscriber that writes spans to stdout
            tracing_subscriber::fmt::layer()
                .event_format(format_for_humans.clone())
                .with_filter(filter_for_humans),
        )
        .with(
            // subscriber that writes spans to a file
            tracing_subscriber::fmt::layer()
                .event_format(format_for_humans)
                .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
                .with_writer(file_writer)
                .with_filter(filter_for_humans),
        );

    set_global_default(subscriber).expect("failed to set subscriber");
}

fn get_server_addr(app_handle: &AppHandle) -> (Ipv4Addr, u16) {
    let config = app_handle.config();
    let product_name = config
        .package
        .product_name
        .as_ref()
        .expect("product name not set");
    let port = if product_name.eq("GitButler") {
        6667
    } else if product_name.eq("GitButler Nightly") {
        6668
    } else {
        6669
    };
    (Ipv4Addr::LOCALHOST, port)
}
