use but_update::AppName;

#[test]
fn app_name_display_format() {
    // These string representations are sent to the server,
    // so they are part of the API contract
    assert_eq!(AppName::Tauri.to_string(), "tauri");
    assert_eq!(AppName::Cli.to_string(), "but-cli");
}

#[test]
fn app_name_has_debug_impl() {
    // Ensures we can debug print AppName
    let tauri = format!("{:?}", AppName::Tauri);
    let cli = format!("{:?}", AppName::Cli);

    assert!(tauri.contains("Tauri"));
    assert!(cli.contains("Cli"));
}
