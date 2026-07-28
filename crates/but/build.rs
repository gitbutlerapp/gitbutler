fn main() {
    let manifest_dir = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo sets CARGO_MANIFEST_DIR"),
    );
    let mcp_app_dir = manifest_dir.join("src/command/mcp");
    let mcp_app_files = [
        mcp_app_dir.join("workspace.html"),
        mcp_app_dir.join("review.html"),
    ];
    for path in &mcp_app_files {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!("cargo:rustc-check-cfg=cfg(but_mcp_app_built)");

    if mcp_app_files.iter().all(|path| path.is_file()) {
        println!("cargo:rustc-cfg=but_mcp_app_built");
    } else if std::env::var_os("CARGO_FEATURE_PACKAGED_BUT_DISTRIBUTION").is_some() {
        panic!(
            "packaged CLI builds require the generated MCP apps; run \
             `pnpm --filter @gitbutler/but-mcp-app build` before building `but`"
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

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // On windows, we need twice the default size for a simple `but.exe` invocation to
        // not crash with stack overflow.
        const STACK_SIZE_MIB: usize = 2;
        const MIB_IN_BYTES: usize = 1024 * 1024;
        let stack_size_bytes = STACK_SIZE_MIB * MIB_IN_BYTES;

        match std::env::var("CARGO_CFG_TARGET_ENV").as_deref() {
            Ok("msvc") => {
                println!("cargo:rustc-link-arg-bin=but=/STACK:{stack_size_bytes}");
            }
            Ok("gnu") => {
                println!("cargo:rustc-link-arg-bin=but=-Wl,--stack,{stack_size_bytes}");
            }
            _ => {}
        }
    }
}
