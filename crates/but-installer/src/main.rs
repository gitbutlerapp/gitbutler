//! GitButler installer binary
//!
//! This binary provides a command-line interface for installing GitButler.
//! The actual installation logic is in the library.

use but_installer::ui;

fn main() {
    if let Err(e) = but_installer::run_installation() {
        ui::error(&e.to_string());
        std::process::exit(1);
    }
}
