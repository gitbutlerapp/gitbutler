#[tokio::main]
async fn main() -> anyhow::Result<()> {
    but_askpass::disable();
    but::handle_args(std::env::args_os()).await
}
