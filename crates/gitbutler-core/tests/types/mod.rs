use gitbutler_core::types::default_true::DefaultTrue;
use gitbutler_core::types::Sensitive;

#[test]
#[allow(clippy::bool_assert_comparison)]
fn default_true() {
    let default_true = DefaultTrue::default();
    assert!(default_true);
    assert_eq!(default_true, true);
    assert_eq!(!default_true, false);
    assert!(!!default_true);

    if !(*default_true) {
        unreachable!("default_true is false")
    }

    let mut default_true = DefaultTrue::default();
    *default_true = false;
    assert!(!default_true);
}

#[test]
fn sensitive_does_not_debug_print_itself() {
    let s = Sensitive("password");
    assert_eq!(format!("{s:?}"), "\"<redacted>\"");
}
