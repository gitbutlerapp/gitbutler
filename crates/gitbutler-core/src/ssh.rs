use std::{env, fs, path::Path};

use ssh2::{CheckResult, KnownHostFileKind};

use crate::git;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Ssh(ssh2::Error),
    #[error(transparent)]
    Io(std::io::Error),
    #[error("mismatched host key")]
    MismatchedHostKey,
    #[error("failed to check the known hosts")]
    Failure,
}

pub fn check_known_host(remote_url: &git::Url) -> Result<(), Error> {
    if remote_url.scheme != git::Scheme::Ssh {
        return Ok(());
    }

    let host = if let Some(host) = remote_url.host.as_ref() {
        host
    } else {
        return Ok(());
    };

    let port = remote_url.port.unwrap_or(22);

    let mut session = ssh2::Session::new().map_err(Error::Ssh)?;
    session.set_tcp_stream(
        std::net::TcpStream::connect(format!("{}:{}", host, port)).map_err(Error::Io)?,
    );
    session.handshake().map_err(Error::Ssh)?;

    let mut known_hosts = session.known_hosts().map_err(Error::Ssh)?;

    // Initialize the known hosts with a global known hosts file
    let dotssh = Path::new(&env::var("HOME").unwrap()).join(".ssh");
    let file = dotssh.join("known_hosts");
    if !file.exists() {
        fs::create_dir_all(&dotssh).map_err(Error::Io)?;
        fs::File::create(&file).map_err(Error::Io)?;
    }

    known_hosts
        .read_file(&file, KnownHostFileKind::OpenSSH)
        .map_err(Error::Ssh)?;

    // Now check to see if the seesion's host key is anywhere in the known
    // hosts file
    let (key, key_type) = session.host_key().unwrap();
    match known_hosts.check(host, key) {
        CheckResult::Match => Ok(()),
        CheckResult::Mismatch => Err(Error::MismatchedHostKey),
        CheckResult::Failure => Err(Error::Failure),
        CheckResult::NotFound => {
            tracing::info!("adding host key for {}", host);
            known_hosts
                .add(host, key, "added by gitbutler client", key_type.into())
                .map_err(Error::Ssh)?;
            known_hosts
                .write_file(&file, KnownHostFileKind::OpenSSH)
                .map_err(Error::Ssh)?;
            Ok(())
        }
    }
}
