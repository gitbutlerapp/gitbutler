use std::os::unix::net::UnixStream;

pub fn establish(sock_path: &str) -> UnixStream {
    let raw_stream = UnixStream::connect(sock_path).expect("connect():");

    // Set a timer for 10s.
    raw_stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .expect("set_read_timeout(Some):")
}
