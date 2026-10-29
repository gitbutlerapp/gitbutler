// Case: `refreshes` wraps the body so a successful call writes the refresh sentinel.
// It verifies:
// - sync `&Context` / `&mut Context` and async `ThreadSafeContext` bodies still compile
//   with `?` and early `return` inside the wrapper
// - `refreshes` combines with a result conversion
// Extend when: the signalling expansion changes.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture};

#[but_api(refreshes)]
pub fn sync_signal(_ctx: &but_ctx::Context, value: i32) -> anyhow::Result<i32> {
    if value < 0 {
        return Err(anyhow::anyhow!("negative"));
    }
    let one: i32 = "1".parse()?;
    Ok(value + one)
}

#[but_api(refreshes)]
pub fn sync_signal_mut(ctx: &mut but_ctx::Context, value: i32) -> anyhow::Result<i32> {
    let _ = &ctx.project_data_dir;
    Ok(value)
}

#[but_api(refreshes)]
pub async fn async_signal(ctx: but_ctx::ThreadSafeContext, value: i32) -> anyhow::Result<i32> {
    let _ = ctx.project_data_dir;
    let one: i32 = "1".parse()?;
    Ok(value + one)
}

#[but_api(json::HexHash, refreshes)]
pub fn signal_with_conversion(_ctx: &but_ctx::Context) -> anyhow::Result<json::HexHash> {
    Ok(json::HexHash(
        "0123456789abcdef0123456789abcdef01234567".into(),
    ))
}

fn main() {}
