// Case: `#[but_api(Type)]` on an owned parameter remaps wrapper transport types
// while keeping the implementation signature domain-oriented.

use but_api_macros::but_api;

pub use but_api_macros_tests::{RelativeTo, UiRelativeTo, json, panic_capture};

#[but_api]
pub fn transport_surface(
    #[but_api(UiRelativeTo)] relative_to: RelativeTo,
) -> anyhow::Result<String> {
    let kind = match relative_to {
        RelativeTo::Commit(_) => "commit",
        RelativeTo::Reference(_) => "reference",
    };
    Ok(kind.to_owned())
}

fn main() {
    let _ = transport_surface_json(UiRelativeTo::Commit("abc".into()));

    #[cfg(feature = "legacy")]
    {
        let _ = transport_surface_cmd(serde_json::json!({
            "relativeTo": {
                "type": "commit",
                "subject": "abc"
            }
        }));
    }
}
