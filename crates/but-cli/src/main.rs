//! A debug-CLI for making `but`-crates functionality available in real-world repositories.
use anyhow::Result;

mod args;
use args::Args;

mod command;

fn main() -> Result<()> {
    let args: Args = clap::Parser::parse();

    if args.trace {
        trace::init()?;
    }
    let _op_span = tracing::info_span!("cli-op").entered();

    match args.cmd {
        args::Subcommands::Status => command::status::doit(),
    }
}

mod trace {
    use tracing::metadata::LevelFilter;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::Layer;

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
