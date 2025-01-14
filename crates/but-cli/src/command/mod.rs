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

pub mod stacks {
    use std::path::Path;

    use crate::command::debug_print;

    pub fn doit(current_dir: &Path) -> anyhow::Result<()> {
        let gb_state_path = current_dir.join(".git").join("gitbutler");
        debug_print(but_workspace::stacks(&gb_state_path))
    }
}
