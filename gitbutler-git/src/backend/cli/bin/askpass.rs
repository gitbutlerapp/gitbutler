#[cfg(not(target_os = "windows"))]
#[path = "askpass-unix.rs"]
mod unix;

#[cfg(target_os = "windows")]
compile_error!("Windows support is not yet implemented.");

pub fn main() {
    let pipe_name = std::env::var("GITBUTLER_ASKPASS_PIPE").expect("do not run this binary yourself; it's only meant to be run by GitButler (missing GITBUTLER_ASKPASS_PIPE env var)");
    let pipe_secret = std::env::var("GITBUTLER_ASKPASS_SECRET").expect("do not run this binary yourself; it's only meant to be run by GitButler (missing GITBUTLER_ASKPASS_SECRET env var)");
    let prompt = std::env::args().nth(1).expect("do not run this binary yourself; it's only meant to be run by GitButler (missing prompt arg)");

    #[cfg(not(target_os = "windows"))]
    unix::main(&pipe_name, &pipe_secret, &prompt);
}
