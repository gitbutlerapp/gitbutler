use std::os::unix::net::UnixStream;

pub fn establish(sock_path: &str) -> UnixStream {
    UnixStream::connect(sock_path).expect("connect():")
}
