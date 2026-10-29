fn main() -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        anyhow::ensure!(
            but_utils::credential_cache::initialize(std::env::current_exe()?, std::env::args_os())?,
            "credential-cache helper requires its daemon arguments"
        );
        Ok(())
    }
    #[cfg(not(unix))]
    anyhow::bail!("credential-cache is available only on Unix")
}
