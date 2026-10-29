fn main() -> anyhow::Result<()> {
    #[cfg(unix)]
    if but_utils::credential_cache::initialize(std::env::current_exe()?, std::env::args_os())? {
        return Ok(());
    }
    run()
}

#[tokio::main]
async fn run() -> anyhow::Result<()> {
    but_askpass::disable();
    but::handle_args(std::env::args_os()).await
}
