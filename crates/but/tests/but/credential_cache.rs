use but_testsupport::gix_testtools;
use gix::credentials::program::cache::{self, DaemonCommand};

#[test]
fn credential_cache_starts_without_git_or_a_runtime() -> anyhow::Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let tmp = gix_testtools::tempfile::tempdir()?;
    let socket = tmp.path().join("cache/socket");
    // SAFETY: the isolated test process runs only this test before starting any threads.
    unsafe { std::env::set_var("PATH", tmp.path().join("no-programs")) };
    let command = DaemonCommand {
        executable: env!("CARGO_BIN_EXE_but").into(),
        args: vec![but_utils::credential_cache::DAEMON_ARGUMENT.into()],
    };
    let credential = b"protocol=https\nhost=example.com\nusername=alice\npassword=secret\n\n";
    let outcome = (|| -> anyhow::Result<Vec<u8>> {
        cache::request(&socket, "store", 60, credential, Some(&command))?;
        Ok(cache::request(
            &socket,
            "get",
            60,
            b"protocol=https\nhost=example.com\n\n",
            None,
        )?)
    })();
    // Shut down even if the request failed, before assertions can unwind.
    cache::request(&socket, "exit", 0, b"", None)?;
    snapbox::assert_data_eq!(
        String::from_utf8(outcome?)?,
        snapbox::str![[r#"
capability[]=authtype
username=alice
password=secret

"#]]
    );
    Ok(())
}
