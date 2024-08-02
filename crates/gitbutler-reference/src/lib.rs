mod refname;

use anyhow::bail;
use gitbutler_tagged_string::TaggedString;
pub use refname::{LocalRefname, Refname, RemoteRefname, VirtualRefname};
use regex::Regex;

pub fn normalize_branch_name(name: &str) -> anyhow::Result<String> {
    // Remove specific symbols
    let exclude_pattern = Regex::new(r"[|\+^~<>\\:*]").unwrap();
    let mut result = exclude_pattern.replace_all(name, "-").to_string();

    // Replace spaces with hyphens
    let space_pattern = Regex::new(r"\s+").unwrap();
    result = space_pattern.replace_all(&result, "-").to_string();

    // Remove leading and trailing hyphens and slashes and dots
    let trim_pattern = Regex::new(r"^[-/\.]+|[-/\.]+$").unwrap();
    result = trim_pattern.replace_all(&result, "").to_string();

    let refname = format!("refs/gitbutler/{result}");
    if gix::validate::reference::name(refname.as_str().into()).is_err() {
        bail!("Could not turn {result:?} into a valid reference name")
    }

    Ok(result)
}

pub struct _ReferenceName;
/// The name of a reference i.e. `refs/heads/master`
pub type ReferenceName = TaggedString<_ReferenceName>;
