mod refname;

pub use refname::{LocalRefname, Refname, RemoteRefname, VirtualRefname};

// TODO(ST): return `BString`, probably take BString, as branch names don't have to be valid UTF8
pub fn normalize_branch_name(name: &str) -> anyhow::Result<String> {
    Ok(but_core::branch::normalize_short_name(name)?.to_string())
}
