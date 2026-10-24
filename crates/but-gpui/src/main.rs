//! The GPUI client on its own, as `gitbutler-gpui`. The desktop app can also start it, with
//! `--gpui`, when built with its `builtin-gpui` feature.
// A release build on Windows opens no console window beside the app's.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() {
    use_production_server();
    but_gpui::init_logging();
    but_gpui::run(None);
}

/// Sign in, and use the account, on GitButler's production server, where people's accounts
/// are, rather than the staging one `gitbutler-user` takes a build from source to want.
/// Either of the variables it reads still names another.
fn use_production_server() {
    const SERVER: [&str; 2] = ["GITBUTLER_API_URL", "PUBLIC_API_BASE_URL"];
    if SERVER.iter().all(|key| std::env::var_os(key).is_none()) {
        // SAFETY: first thing in `main`, before any other thread could read the environment.
        unsafe { std::env::set_var(SERVER[0], "https://app.gitbutler.com") };
    }
}
