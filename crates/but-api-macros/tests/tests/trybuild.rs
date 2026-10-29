#[cfg(not(feature = "tauri"))]
#[test]
fn macro_features_compile_without_optional_features() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/pass/default_*.rs");
    t.compile_fail("tests/ui/fail/base_*.rs");
    #[cfg(not(feature = "legacy"))]
    t.compile_fail("tests/ui/fail/default_*.rs");
}

#[cfg(all(feature = "legacy", not(feature = "tauri")))]
#[test]
fn macro_features_compile_with_legacy() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/pass/legacy_*.rs");
}

#[cfg(feature = "tauri")]
#[test]
fn macro_features_compile_with_tauri() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/fail/tauri_*.rs");
}
