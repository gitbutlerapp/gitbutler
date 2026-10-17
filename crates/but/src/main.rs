/// Remove this process's registered Gitoxide tempfiles on termination, then
/// restore the default signal action.
///
/// Gitoxide installs those handlers on first use of the tempfile registry, so
/// this has to run before askpass or command handling can create a lock.
fn install_registered_tempfile_cleanup() {
    use gix::tempfile::signal::handler::Mode;

    gix::tempfile::signal::setup(Mode::DeleteTempfilesOnTerminationAndRestoreDefaultBehaviour);
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    install_registered_tempfile_cleanup();
    but_askpass::disable();
    but::handle_args(std::env::args_os()).await
}

#[cfg(test)]
mod signal_tests;
