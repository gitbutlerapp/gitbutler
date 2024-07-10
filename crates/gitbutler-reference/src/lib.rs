mod refname;
use gitbutler_tagged_string::TaggedString;
pub use refname::{LocalRefname, Refname, RemoteRefname, VirtualRefname};
use regex::Regex;

pub fn normalize_branch_name(name: &str) -> String {
    let pattern = Regex::new("[^A-Za-z0-9_/.#]+").unwrap();
    pattern.replace_all(name, "-").to_string()
}

pub struct _ReferenceName;
/// The name of a reference ie. `refs/heads/master`
pub type ReferenceName = TaggedString<_ReferenceName>;
