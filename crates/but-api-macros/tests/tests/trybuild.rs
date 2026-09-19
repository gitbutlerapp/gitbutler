#[cfg(not(feature = "napi"))]
#[test]
fn macro_features_compile_without_optional_features() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/pass/default_*.rs");
    t.compile_fail("tests/ui/fail/base_*.rs");
    #[cfg(not(feature = "legacy"))]
    t.compile_fail("tests/ui/fail/default_*.rs");
}

#[cfg(all(feature = "legacy", not(feature = "napi")))]
#[test]
fn macro_features_compile_with_legacy() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/pass/legacy_*.rs");
}

#[cfg(feature = "napi")]
#[test]
fn macro_features_compile_with_napi() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/pass/default_*.rs");
    t.pass("tests/ui/pass/legacy_*.rs");
    t.pass("tests/ui/pass/napi_*.rs");
    t.compile_fail("tests/ui/fail/base_*.rs");
    t.compile_fail("tests/ui/fail/napi_*.rs");
}
