fn main() {
    // Commit 78a5cb236add69e11f96d2e9d5c4d32b2bb5d68e updates dependencies which
    // introduces a linking issue on macOS.
    //
    // = note: some arguments are omitted. use `--verbose` to show all linker arguments
    //     = note: ld: warning: ignoring duplicate libraries: '-lSystem', '-liconv', '-lobjc'
    // ld: warning: object file (/Users/byron/dev/github.com/gitbutlerapp/gitbutler/target/debug/deps/liblibsqlite3_sys-40703282059d93f5.rlib[4](c877a2978823c39d-sqlite3.o)) was built for newer 'macOS' version (26.0) than being linked (11.0)
    //
    // Try if it still is an issue by disabling this line and running
    //      cargo run -p but-server
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=framework=AppKit");
    }
}
