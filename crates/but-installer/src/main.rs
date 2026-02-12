//! GitButler installer binary
//!
//! This binary provides a command-line interface for installing GitButler.
//! The actual installation logic is in the library.

fn main() {
    #[cfg(unix)]
    if let Err(e) = but_installer::run_installation() {
        but_installer::ui::error(&format!("{:#}", e));
        std::process::exit(1);
    }
    #[cfg(not(unix))]
    {
        eprintln!("but-installer is only supported on Linux and macOS");
        std::process::exit(1);
    }
}
