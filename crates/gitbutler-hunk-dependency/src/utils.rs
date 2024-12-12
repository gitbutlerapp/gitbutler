/// Subtract two unsigned integers and return an error if the result is negative.
pub fn panicless_subtraction(a: u32, b: u32, context: &str) -> anyhow::Result<u32> {
    a.checked_sub(b)
        .ok_or_else(|| anyhow::anyhow!("Subtraction overflow: {} - {}. {}", a, b, context))
}
