use but_api_macros::but_api;

pub use but_api_macros_tests::panic_capture;

#[but_api]
pub fn thread_safe_context(
    _ctx: but_ctx::ThreadSafeContext,
    _perm: &but_ctx::access::RepoShared,
) -> anyhow::Result<()> {
    Ok(())
}

fn main() {}
