use anyhow::Result;

pub trait RunCommand {
    fn run(self) -> Result<()>;
}
