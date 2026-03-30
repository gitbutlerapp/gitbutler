use but_server::Config;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "but-server", about = "GitButler remote access server")]
struct Args {
    /// Port to listen on.
    #[arg(long, default_value = "6978")]
    port: u16,

    /// Host to bind to. Defaults to 127.0.0.1 in local mode, 0.0.0.0 when remote access is on.
    #[arg(long)]
    host: Option<String>,

    /// Serve on localhost only without opening a tunnel. No authentication required.
    #[arg(long)]
    local: bool,

    /// Spawn a Cloudflare quick tunnel and use its URL as the allowed remote origin.
    #[arg(long)]
    tunnel: bool,

    /// Cloudflare named tunnel name or UUID to run (e.g. `mytunnel`). Must be paired with --hostname.
    #[arg(long, requires = "hostname")]
    name: Option<String>,

    /// Public hostname routed to --name (e.g. `but.example.com`). Must be paired with --name.
    #[arg(long, requires = "name")]
    hostname: Option<String>,

    /// Allow remote access from this specific origin (e.g. https://my-tunnel.trycloudflare.com).
    #[arg(long)]
    remote_origin: Option<String>,

    /// Prefix all API routes with this path (e.g. /api).
    #[arg(long)]
    base_path: Option<String>,

    /// Disable authentication entirely. DANGEROUS — only use on trusted networks.
    #[arg(long)]
    dangerously_allow_anyone: bool,

    /// GitButler API base URL.
    #[arg(long, default_value = "https://app.gitbutler.com")]
    api_url: String,
}

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

    // To be able to use the askpass broker when running but-server, it needs to be hooked up to
    // the websocket. As the askpass broker historically hasn't been initialized for but-server,
    // it does not seem worthwhile to hook that up right now.
    gitbutler_repo_actions::askpass::disable();

    let args = Args::parse();
    let config = Config {
        port: Some(args.port),
        host: args.host,
        tunnel: !args.local && args.tunnel && args.name.is_none(),
        tunnel_name: args.name,
        tunnel_hostname: args.hostname,
        remote_origin: args.remote_origin,
        base_path: args.base_path,
        allow_anyone: args.dangerously_allow_anyone,
        api_url: Some(args.api_url),
        project_path: None,
    };
    but_server::run(config).await
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
