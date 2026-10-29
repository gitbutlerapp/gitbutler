#[ctor::ctor]
fn init() {
    // These tests do not function with the askpass broker enabled
    but_askpass::disable();
}
