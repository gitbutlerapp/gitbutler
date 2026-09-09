// Case: `invalidates` wraps the body so a successful call signals its tags.
// It verifies:
// - sync `&Context` / `&mut Context` and async `ThreadSafeContext` bodies still compile
//   with `?` and early `return` inside the wrapper
// - `<FN_NAME>_INVALIDATES` carries the declared names
// - an endpoint without a context is left unwrapped
// Extend when: the signalling expansion or the constant's shape changes.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture, tags};

#[but_api(napi, invalidates = [Reviews])]
pub fn sync_signal(_ctx: &but_ctx::Context, value: i32) -> anyhow::Result<i32> {
    if value < 0 {
        return Err(anyhow::anyhow!("negative"));
    }
    let one: i32 = "1".parse()?;
    Ok(value + one)
}

#[but_api(napi, invalidates = [Reviews, Checks])]
pub fn sync_signal_mut(ctx: &mut but_ctx::Context, value: i32) -> anyhow::Result<i32> {
    let _ = &ctx.project_data_dir;
    Ok(value)
}

#[but_api(napi, invalidates = [Checks])]
pub async fn async_signal(ctx: but_ctx::ThreadSafeContext, value: i32) -> anyhow::Result<i32> {
    let _ = ctx.project_data_dir;
    let one: i32 = "1".parse()?;
    Ok(value + one)
}

#[but_api(napi, invalidates = [Checks])]
pub fn signal_without_context(value: i32) -> anyhow::Result<i32> {
    Ok(value)
}

fn main() {
    assert_eq!(SYNC_SIGNAL_INVALIDATES, &["Reviews"]);
    assert_eq!(SYNC_SIGNAL_MUT_INVALIDATES, &["Reviews", "Checks"]);
    assert_eq!(ASYNC_SIGNAL_INVALIDATES, &["Checks"]);
    assert_eq!(SIGNAL_WITHOUT_CONTEXT_INVALIDATES, &["Checks"]);
    let _ = sync_signal_napi;
    let _ = async_signal_napi;
}
