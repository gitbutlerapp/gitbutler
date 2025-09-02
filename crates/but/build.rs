fn main() {
    let identifier = if let Ok(channel) = std::env::var("CHANNEL") {
        match channel.as_str() {
            "nightly" => "com.gitbutler.app.nightly",
            "release" => "com.gitbutler.app",
            _ => "com.gitbutler.app",
        }
    } else {
        "com.gitbutler.app"
    };
    println!("cargo:rustc-env=IDENTIFIER={identifier}");
}
