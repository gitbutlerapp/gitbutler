mod refname;

use anyhow::bail;
pub use refname::{LocalRefname, Refname, RemoteRefname, VirtualRefname};

// TODO(ST): return `BString`, probably take BString, as branch names don't have to be valid UTF8
pub fn normalize_branch_name(name: &str) -> anyhow::Result<String> {
    let mut sanitized = gix::validate::reference::name_partial_or_sanitize(name.into());
    fn is_forbidden_in_trailer_or_leader(b: u8) -> bool {
        b == b'-' || b == b'.' || b == b'/'
    }
    while let Some(last) = sanitized.last() {
        if is_forbidden_in_trailer_or_leader(*last) {
            sanitized.pop();
        } else {
            break;
        }
    }
    while let Some(first) = sanitized.first() {
        if is_forbidden_in_trailer_or_leader(*first) {
            sanitized.remove(0);
        } else {
            break;
        }
    }

    let mut previous_is_hyphen = false;
    sanitized.retain(|b| {
        if *b == b'-' {
            if previous_is_hyphen {
                return false;
            }
            previous_is_hyphen = true;
        } else {
            previous_is_hyphen = false;
        }
        true
    });

    if sanitized.is_empty() {
        bail!("Could not turn {name:?} into a valid reference name")
    }
    Ok(sanitized.to_string())
}

/// The name of a reference i.e. `refs/heads/master`
pub type ReferenceName = String;
