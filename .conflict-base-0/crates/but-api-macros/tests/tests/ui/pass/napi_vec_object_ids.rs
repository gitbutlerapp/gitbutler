// Case: `#[but_api(napi)]` supports Vec<ObjectId> parameters from JS string arrays.
// It verifies `_napi` generation compiles when vector parsing is required.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture};

#[but_api(napi)]
pub fn napi_vec_object_ids(commit_ids: Vec<gix::ObjectId>) -> anyhow::Result<usize> {
    Ok(commit_ids.len())
}

fn main() {
    let _ = napi_vec_object_ids_napi;
    let _ = napi_napi_vec_object_ids::napi_vec_object_ids;
}
