mod refname;
use gitbutler_tagged_string::TaggedString;
pub use refname::{LocalRefname, Refname, RemoteRefname, VirtualRefname};
use regex::Regex;

pub fn normalize_branch_name(name: &str) -> String {
    // Remove specific symbols
    let exclude_pattern = Regex::new(r"[|\+^~<>\\:*]").unwrap();
    let mut result = exclude_pattern.replace_all(name, "-").to_string();

    // Replace spaces with hyphens
    let space_pattern = Regex::new(r"\s+").unwrap();
    result = space_pattern.replace_all(&result, "-").to_string();

    // Remove leading and trailing hyphens and slashes
    let trim_pattern = Regex::new(r"^[-/]+|[-/]+$").unwrap();
    result = trim_pattern.replace_all(&result, "").to_string();

    result
}

pub struct _ReferenceName;
/// The name of a reference ie. `refs/heads/master`
pub type ReferenceName = TaggedString<_ReferenceName>;
