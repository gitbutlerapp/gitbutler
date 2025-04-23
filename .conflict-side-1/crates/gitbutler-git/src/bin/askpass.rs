#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(unix)]
#[path = "askpass/unix.rs"]
mod unix;
#[cfg(windows)]
#[path = "askpass/windows.rs"]
mod windows;

use std::io::{BufRead, BufReader, BufWriter, Write};

#[cfg(windows)]
use self::windows::UnixCompatibility;

pub fn main() {
    let pipe_name = std::env::var("GITBUTLER_ASKPASS_PIPE").expect("do not run this binary yourself; it's only meant to be run by GitButler (missing GITBUTLER_ASKPASS_PIPE env var)");
    let pipe_secret = std::env::var("GITBUTLER_ASKPASS_SECRET").expect("do not run this binary yourself; it's only meant to be run by GitButler (missing GITBUTLER_ASKPASS_SECRET env var)");
    let prompt = std::env::args()
        .nth(1)
        .expect("do not run this binary yourself; it's only meant to be run by GitButler (missing prompt arg)");

    #[cfg(unix)]
    let raw_stream = self::unix::establish(&pipe_name);
    #[cfg(windows)]
    let raw_stream = self::windows::establish(&pipe_name);

    let mut reader = BufReader::new(raw_stream.try_clone().unwrap());
    let mut writer = BufWriter::new(raw_stream.try_clone().unwrap());

    // Write the secret.
    writeln!(writer, "{pipe_secret}").expect("write(secret):");

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
