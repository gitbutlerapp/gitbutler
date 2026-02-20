#[tokio::main]
async fn main() -> anyhow::Result<()> {
    #[cfg(feature = "legacy")]
    gitbutler_repo_actions::askpass::disable();
    but::handle_args(std::env::args_os()).await
}
