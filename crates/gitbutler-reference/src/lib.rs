mod refname;
pub use refname::{LocalRefname, Refname, RemoteRefname, VirtualRefname};
use regex::Regex;

pub fn normalize_branch_name(name: &str) -> String {
    let pattern = Regex::new("[^A-Za-z0-9_/.#]+").unwrap();
    pattern.replace_all(name, "-").to_string()
}
