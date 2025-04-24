pub trait PaniclessSubtraction<T> {
    fn sub_or_err(&self, b: T) -> anyhow::Result<u32>;
}

impl PaniclessSubtraction<u32> for u32 {
    fn sub_or_err(&self, b: u32) -> anyhow::Result<u32> {
        self.checked_sub(b)
            .ok_or_else(|| anyhow::anyhow!("Subtraction overflow: {} - {}.", self, b))
    }
}
