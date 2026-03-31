use but_api_macros::but_api;

pub use but_api_macros_tests::panic_capture;

#[but_api]
pub fn missing_context(_perm: &mut but_ctx::access::RepoExclusive) -> anyhow::Result<()> {
    Ok(())
}

fn main() {}
