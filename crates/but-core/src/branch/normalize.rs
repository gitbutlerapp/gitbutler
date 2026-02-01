use anyhow::bail;
use bstr::{BStr, BString};

/// Assuming `name` is a short-name like `feat/hi`, `main` or `something the user typed`, convert it into
/// a version of this which would be a valid *short* reference name.
pub fn normalize_short_name<'a>(name: impl Into<&'a BStr>) -> anyhow::Result<BString> {
    let name = name.into();
    let mut sanitized = gix::validate::reference::name_partial_or_sanitize(name);
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
    Ok(sanitized)
}
