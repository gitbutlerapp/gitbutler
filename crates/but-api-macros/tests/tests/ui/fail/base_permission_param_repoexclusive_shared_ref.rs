use but_api_macros::but_api;

pub use but_api_macros_tests::panic_capture;

#[but_api]
pub fn invalid_exclusive_form(
    _ctx: &mut but_ctx::Context,
    _perm: &but_ctx::access::RepoExclusive,
) -> anyhow::Result<()> {
    Ok(())
}

fn main() {}
