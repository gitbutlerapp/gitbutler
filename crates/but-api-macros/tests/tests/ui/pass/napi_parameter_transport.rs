// Case: `#[but_api(napi)]` parameter transport remap accepts the transport type shape on the
// generated napi surface while the implementation uses the internal type.

use but_api_macros::but_api;

pub use but_api_macros_tests::{RelativeTo, UiRelativeTo, json, panic_capture};

#[but_api(napi)]
pub fn napi_transport_surface(
    #[but_api(UiRelativeTo)] relative_to: RelativeTo,
) -> anyhow::Result<String> {
    let kind = match relative_to {
        RelativeTo::Commit(_) => "commit",
        RelativeTo::Reference(_) => "reference",
    };
    Ok(kind.to_owned())
}

fn main() {
    let _ = napi_transport_surface_napi;
    let _ = napi_napi_transport_surface::napi_transport_surface;
}
