#[test]
fn every_sdk_endpoint_is_callable_by_its_javascript_name() {
    let callable: std::collections::BTreeSet<_> = inventory::iter::<but_api::CmdEntry>()
        .map(|entry| entry.js_name)
        .collect();
    assert!(
        callable.contains("headInfo"),
        "the registry is linked into the test binary"
    );
    let missing: Vec<_> = inventory::iter::<but_schemars::ApiFnEntry>()
        .map(|entry| entry.js_name)
        .filter(|name| !callable.contains(name))
        .collect();
    assert!(
        missing.is_empty(),
        "a server can serve the whole SDK only if each endpoint registers its `_cmd`: {missing:?}"
    );
}
