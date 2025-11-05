#[tokio::main]
async fn main() -> anyhow::Result<()> {
    but::handle_args(std::env::args_os()).await
}
