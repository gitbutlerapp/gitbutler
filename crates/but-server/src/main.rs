#[tokio::main]
async fn main() -> anyhow::Result<()> {
    trace::init()?;
    // On macOS, in dev mode with debug assertions, we encounter popups each time
    // the binary is rebuilt. To counter that, use a git-credential based implementation.
    // This isn't an issue for actual release build (i.e. nightly, production),
    // hence the specific condition.
    if cfg!(debug_assertions) && cfg!(target_os = "macos") {
        but_secret::secret::git_credentials::setup().ok();
    }
    but_server::run().await;
    Ok(())
}

mod trace {
    use tracing::metadata::LevelFilter;
    use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

    pub fn init() -> anyhow::Result<()> {
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::default().add_directive(LevelFilter::DEBUG.into()));

        tracing_subscriber::registry()
            .with(
                tracing_forest::ForestLayer::from(
                    tracing_forest::printer::PrettyPrinter::new().writer(std::io::stderr),
                )
                .with_filter(filter),
            )
            .init();
        Ok(())
    }
}
