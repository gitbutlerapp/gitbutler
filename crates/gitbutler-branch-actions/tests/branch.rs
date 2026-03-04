mod virtual_branches;

#[ctor::ctor]
fn init() {
    // These tests do not function with the askpass broker enabled
    gitbutler_repo_actions::askpass::disable();
}
