fn debug_print(this: impl std::fmt::Debug) -> anyhow::Result<()> {
    println!("{:#?}", this);
    Ok(())
}

pub mod status {
    use crate::command::debug_print;

    pub fn doit() -> anyhow::Result<()> {
        debug_print("call into but-core")
    }
}
