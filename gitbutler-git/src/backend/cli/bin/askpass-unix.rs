use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

pub fn main(sock_path: &str, secret: &str, prompt: &str) {
    let mut stream = UnixStream::connect(sock_path).expect("connect():");

    // Set a timer for 10s.
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .expect("set_read_timeout():");

    // Write the secret.
    stream
        .write_all(secret.as_bytes())
        .expect("write_all(secret):");

    // Write the prompt that Git gave us.
    stream
        .write_all(prompt.as_bytes())
        .expect("write_all(prompt):");

    // Wait for the response.
    let mut buf = [0; 2048];
    let n = stream.read(&mut buf).expect("read():");

    // TODO(qix-): Figure out a way to do a single timeout
    // TODO(qix-): but allow any response size.
    if n == buf.len() {
        panic!("response too long");
    }

    // Write the response back to Git.
    std::io::stdout()
        .write_all(&buf[..n])
        .expect("write_all(stdout):");
}
