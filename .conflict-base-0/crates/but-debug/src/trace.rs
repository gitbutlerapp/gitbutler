//! Tracing setup for `but-debug`.

use anyhow::Result;
use tracing::Level;
use tracing_subscriber::{
    Layer,
    filter::DynFilterFn,
    fmt::{format::FmtSpan, writer::BoxMakeWriter},
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

/// Initialize tracing output according to the requested verbosity.
pub(crate) fn init(trace_level: u8) -> Result<()> {
    if trace_level == 0 {
        return Ok(());
    }

    let level = match trace_level {
        1 => Level::INFO,
        2 => Level::DEBUG,
        _ => Level::TRACE,
    };

    let filter = DynFilterFn::new(move |meta, _cx| {
        if *meta.level() > level {
            return false;
        }
        if level > Level::DEBUG {
            return true;
        }
        if level > Level::INFO
            && meta
                .module_path()
                .is_some_and(|p| p.starts_with("gitbutler_"))
        {
            return true;
        }
        if meta.module_path().is_some_and(|p| {
            p == "but_debug" || p.starts_with("but_debug::") || p.starts_with("but_")
        }) {
            return true;
        }
        false
    });

    let make_writer = BoxMakeWriter::new(std::io::stderr);
    if trace_level >= 4 {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .compact()
                    .with_span_events(FmtSpan::CLOSE)
                    .with_writer(make_writer)
                    .with_filter(filter),
            )
            .init();
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

    Ok(())
}
