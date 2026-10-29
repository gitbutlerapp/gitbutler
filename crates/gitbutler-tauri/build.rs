fn main() {
    // Make the UI build directory if it doesn't already exist.
    // We do this here because the tauri context macro expects it to
    // exist at build time, and it's otherwise manually required to create
    // it before building.
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(manifest_dir.file_name().unwrap(), "gitbutler-tauri");
    let build_dir = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("apps")
        .join("desktop")
        .join("build");
    if !build_dir.exists() {
        // NOTE(qix-): Do not use `create_dir_all` here - the parent directory
        // NOTE(qix-): already exists, and we want to fail if not (for some reason).
        #[expect(clippy::expect_fun_call, clippy::create_dir)]
        std::fs::create_dir(&build_dir).expect(
            format!("failed to create apps/desktop/build directory: {build_dir:?}").as_str(),
        );
    }
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

    // The native client brings GPUI, which links its own manifest into every binary, one with all
    // Tauri's has, and a binary may have only one. Its window takes the icon in resource 1, where
    // Tauri puts its own at 32512, and its UI runs on the main thread, which Windows gives 1 MB of
    // stack where macOS gives 8.
    if std::env::var_os("CARGO_FEATURE_BUILTIN_GPUI").is_some()
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
    {
        let channel = identifier
            .strip_prefix("com.gitbutler.app.")
            .unwrap_or("release");
        let icon = manifest_dir.join("icons").join(channel).join("icon.ico");
        let windows = tauri_build::WindowsAttributes::new_without_app_manifest()
            .append_rc_content(format!("1 ICON {:?}", icon.display().to_string()));
        println!("cargo:rustc-link-arg-bins=/STACK:8388608");
        tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
            .unwrap_or_else(|err| panic!("{err:#}"));
    } else {
        tauri_build::build();
    }
}
