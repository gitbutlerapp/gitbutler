use but_api_macros::but_api;

pub use but_api_macros_tests::{UiRelativeTo, panic_capture};

#[but_api]
pub fn invalid_transport_builtin(
    #[but_api(UiRelativeTo)] commit_id: gix::ObjectId,
) -> anyhow::Result<()> {
    let _ = commit_id;
    Ok(())
}

fn main() {}
