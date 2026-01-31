pub mod project;
pub mod vbranch;

fn debug_print(this: impl std::fmt::Debug) -> anyhow::Result<()> {
    println!("{this:#?}");
    Ok(())
}
