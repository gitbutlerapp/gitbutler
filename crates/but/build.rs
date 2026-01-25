fn main() {
    let identifier = if let Ok(channel) = std::env::var("CHANNEL") {
        match channel.as_str() {
            "nightly" => "com.gitbutler.app.nightly",
            "release" => "com.gitbutler.app",
            _ => "com.gitbutler.app.dev",
        }
    } else {
        "com.gitbutler.app.dev"
    };
    println!("cargo:rustc-env=IDENTIFIER={identifier}");

    #[cfg(windows)]
    {
        // The `but` CLI has a large clap command tree which can overflow the Windows default stack.
        // Increase the stack reserve size so `but --help` and integration tests can run reliably.
        println!("cargo:rustc-link-arg=/STACK:8388608");
    }
}
