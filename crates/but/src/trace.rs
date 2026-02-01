use tracing::Level;
use tracing_subscriber::{
    Layer, filter::DynFilterFn, fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt,
};

pub fn init(level: u8) -> anyhow::Result<()> {
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

    if level >= 4 {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .compact()
                    .with_span_events(FmtSpan::CLOSE)
                    .with_writer(std::io::stderr)
                    .with_filter(filter),
            )
            .init()
    } else {
        tracing_subscriber::registry()
            .with(
                tracing_forest::ForestLayer::from(
                    tracing_forest::printer::PrettyPrinter::new().writer(std::io::stderr),
                )
                .with_filter(filter),
            )
            .init();
    }
    Ok(())
}
