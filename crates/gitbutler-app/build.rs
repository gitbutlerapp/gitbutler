fn main() {
    // Make the UI build directory if it doesn't already exist.
    // We do this here because the tauri context macro expects it to
    // exist at build time, and it's otherwise manually required to create
    // it before building.
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(manifest_dir.file_name().unwrap(), "gitbutler-app");
    let build_dir = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("gitbutler-ui/build");
    if !build_dir.exists() {
        // NOTE(qix-): Do not use `create_dir_all` here - the parent directory
        // NOTE(qix-): already exists, and we want to fail if not (for some reason).
        #[allow(clippy::expect_fun_call, clippy::create_dir)]
        std::fs::create_dir(&build_dir).expect(
            format!(
                "failed to create gitbutler-ui build directory: {:?}",
                build_dir
            )
            .as_str(),
        );
    }

    tauri_build::build();
}
