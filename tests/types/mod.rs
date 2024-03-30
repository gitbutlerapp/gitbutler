use gitbutler::types::default_true::DefaultTrue;

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
