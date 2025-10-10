#[tokio::main]
async fn main() -> anyhow::Result<()> {
    trace::init()?;
    // On macOS, in dev mode with debug assertions, we encounter popups each time
    // the binary is rebuilt. To counter that, use a git-credential based implementation.
    // This isn't an issue for actual release build (i.e. nightly, production),
    // hence the specific condition.
    if cfg!(debug_assertions) && cfg!(target_os = "macos") {
        gitbutler_secret::secret::git_credentials::setup().ok();
    }
    but_server::run().await;
    Ok(())
}

mod trace {
    use tracing::metadata::LevelFilter;
    use tracing_subscriber::Layer;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    pub fn init() -> anyhow::Result<()> {
        tracing_subscriber::registry()
            .with(
                tracing_forest::ForestLayer::from(
                    tracing_forest::printer::PrettyPrinter::new().writer(std::io::stderr),
                )
                .with_filter(LevelFilter::DEBUG),
            )
            .init();
        Ok(())
    }
}
