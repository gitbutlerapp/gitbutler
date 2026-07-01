// Case: `#[but_api(napi)]` maps `&gix::refs::FullNameRef` parameters to `String`.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture};

#[but_api(napi)]
pub fn full_name_roundtrip(existing_branch: &gix::refs::FullNameRef) -> anyhow::Result<String> {
    Ok(existing_branch.to_string())
}

fn main() {
    let _ = full_name_roundtrip_napi;
    let _ = napi_full_name_roundtrip::full_name_roundtrip;
}
