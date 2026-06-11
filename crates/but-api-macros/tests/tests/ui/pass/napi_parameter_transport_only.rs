// Case: `#[but_api(napi = Type)]` changes only the generated N-API parameter
// transport while leaving the JSON/Tauri wrapper parameter type alone.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture};

#[but_api(napi, napi_param(branch_name = json::FullRefNameBytes))]
pub fn napi_only_transport_surface(branch_name: String) -> anyhow::Result<String> {
    Ok(branch_name)
}

fn main() {
    let _ = napi_only_transport_surface_json("refs/heads/topic".to_owned());
    let _ = napi_only_transport_surface_napi;
    let _ = napi_napi_only_transport_surface::napi_only_transport_surface;
}
