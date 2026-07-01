use std::path::Path;

use anyhow::Context;
use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    Layer,
    filter::DynFilterFn,
    fmt::{format::FmtSpan, writer::BoxMakeWriter},
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

pub fn init(level: u8, log_file_path: Option<&Path>) -> anyhow::Result<Option<WorkerGuard>> {
    let level_t = match level {
        1 => Level::INFO,
        2 => Level::DEBUG,
        _ => Level::TRACE,
    };

    let filter = DynFilterFn::new(move |meta, _cx| {
        if *meta.level() > level_t {
            return false;
        }
        if level_t > Level::DEBUG {
            return true;
        }
        if level_t > Level::INFO
            && meta
                .module_path()
                .is_some_and(|p| p.starts_with("gitbutler_"))
        {
            return true;
        }
        if meta
            .module_path()
            .is_some_and(|p| p == "but" || p.starts_with("but::") || p.starts_with("but_"))
        {
            return true;
        }
        false
    });

    let (make_writer, with_ansi, guard) = if let Some(log_file_path) = log_file_path {
        let file = std::fs::File::create(log_file_path)
            .with_context(|| format!("failed to open log file path {}", log_file_path.display()))?;
        let (non_blocking, guard) = tracing_appender::non_blocking(file);
        (BoxMakeWriter::new(non_blocking), false, Some(guard))
    } else {
        (BoxMakeWriter::new(std::io::stderr), true, None)
    };

    if level >= 4 {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .compact()
                    .with_span_events(FmtSpan::CLOSE)
                    .with_writer(make_writer)
                    .with_ansi(with_ansi)
                    .with_filter(filter),
            )
            .init()
    } else {
        tracing_subscriber::registry()
            .with(
                tracing_forest::ForestLayer::from(
                    tracing_forest::printer::PrettyPrinter::new().writer(make_writer),
                )
                .with_filter(filter),
            )
            .init();
    }

    Ok(guard)
}
