use but_api_macros::but_api;

pub use but_api_macros_tests::{UiRelativeTo, panic_capture};

#[but_api]
pub fn invalid_transport_reference(
    #[but_api(UiRelativeTo)] relative_to: &str,
) -> anyhow::Result<()> {
    let _ = relative_to;
    Ok(())
}

fn main() {}
