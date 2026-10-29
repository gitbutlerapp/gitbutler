use but_api_macros::but_api;

pub use but_api_macros_tests::{UiRelativeTo, panic_capture};

#[but_api]
pub fn invalid_transport_keyed(
    #[but_api(transport = UiRelativeTo)] relative_to: String,
) -> anyhow::Result<()> {
    let _ = relative_to;
    Ok(())
}

fn main() {}
