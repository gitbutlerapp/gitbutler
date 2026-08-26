// Case: `#[but_api(napi)]` supports Option<ObjectId> and Vec<ObjectId> parameters from JS strings.
// It verifies `_napi` generation compiles when optional and vector parsing is required.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture};

#[but_api(napi)]
pub fn napi_vec_object_ids(
    cursor: Option<gix::ObjectId>,
    commit_ids: Vec<gix::ObjectId>,
) -> anyhow::Result<usize> {
    Ok(usize::from(cursor.is_some()) + commit_ids.len())
}

fn main() {
    let _ = napi_vec_object_ids_napi;
    let _ = napi_napi_vec_object_ids::napi_vec_object_ids;
}
