use gitbutler_secret::Sensitive;

#[test]
fn sensitive_does_not_debug_print_itself() {
    let s = Sensitive("password");
    assert_eq!(format!("{s:?}"), "\"<redacted>\"");
}
