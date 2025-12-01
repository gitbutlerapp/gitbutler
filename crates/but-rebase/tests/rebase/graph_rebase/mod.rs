mod cherry_pick;
mod editor_creation;
mod insert;
mod rebase_identities;
mod replace;

pub fn set_var(key: &str, value: &str) {
    unsafe {
        std::env::set_var(key, value);
    }
}
