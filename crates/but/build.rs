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
    println!("cargo:rustc-env=IDENTIFIER={}", identifier);

    // Read version from Cargo.toml
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let cargo_toml = std::fs::read_to_string(format!("{}/Cargo.toml", manifest_dir)).unwrap();
    let version = cargo_toml
        .lines()
        .find(|line| line.trim_start().starts_with("version = "))
        .and_then(|line| line.split('=').nth(1))
        .map(|v| v.trim().trim_matches('"'))
        .unwrap_or("0.0.0");
    // Set build date as version string
    let build_date = chrono::Utc::now().format("%Y%m%d").to_string();
    let full_version = format!("v{}-{}", version, build_date);
    println!("cargo:rustc-env=GIX_VERSION={}", full_version);
}
