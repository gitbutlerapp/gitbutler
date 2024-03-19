use std::io::{BufRead, BufReader, BufWriter, Write};
use std::os::unix::net::UnixStream;

pub fn main(sock_path: &str, secret: &str, prompt: &str) {
    let raw_stream = UnixStream::connect(sock_path).expect("connect():");

    // Set a timer for 10s.
    raw_stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .expect("set_read_timeout(Some):");

    let mut reader = BufReader::new(raw_stream.try_clone().unwrap());
    let mut writer = BufWriter::new(raw_stream.try_clone().unwrap());

    // Write the secret.
    writeln!(writer, "{secret}").expect("write(secret):");

    // Write the prompt that Git gave us.
    writeln!(writer, "{prompt}").expect("write(prompt):");

    writer.flush().expect("flush():");

    // Clear the timeout (it's now time for the user to provide a response)
    raw_stream
        .set_read_timeout(None)
        .expect("set_read_timeout(None):");

    // Wait for the response.
    let mut password = String::new();
    let nread = reader.read_line(&mut password).expect("read_line():");
    if nread == 0 {
        panic!("read_line() returned 0");
    }

    // Write the response back to Git.
    // `password` already has a newline at the end.
    write!(std::io::stdout(), "{password}").expect("write(password):");
}
