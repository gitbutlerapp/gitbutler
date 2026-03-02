// Case: `#[but_api(napi)]` parameter remapping matrix.
// It covers simple types, `usize`/`isize` numeric remap, complex serde value remap, and `BString`.
// Extend when: napi argument type mapping, conversion, or `ts_arg_type` generation rules change.

use but_api_macros::but_api;

pub use but_api_macros_tests::{ComplexParam, json, panic_capture};

#[but_api(napi)]
pub fn napi_surface(
    name: String,
    flag: bool,
    count: usize,
    delta: isize,
    payload: ComplexParam,
    bytes: bstr::BString,
) -> anyhow::Result<(usize, isize)> {
    let _ = (name, flag, payload, bytes);
    Ok((count, delta))
}

fn main() {
    let _ = napi_surface_napi;
    let _ = napi_napi_surface::napi_surface;
}
