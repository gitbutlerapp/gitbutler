use but_api_macros::but_api;

pub use but_api_macros_tests::panic_capture;

#[but_api]
pub fn invalid_shared_form(
    _ctx: &but_ctx::Context,
    _perm: &mut but_ctx::access::RepoShared,
) -> anyhow::Result<()> {
    Ok(())
}

fn main() {}
