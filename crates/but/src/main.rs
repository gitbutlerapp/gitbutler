#[tokio::main]
async fn main() -> anyhow::Result<()> {
    gitbutler_repo_actions::askpass::disable();
    but::handle_args(std::env::args_os()).await
}
