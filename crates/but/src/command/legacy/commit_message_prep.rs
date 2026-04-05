use anyhow::{Result, bail};

/// Returns the canonical commit message representation used for comparisons and storage.
///
/// This currently trims leading and trailing whitespace.
pub(crate) fn normalize_commit_message(message: &str) -> &str {
    message.trim()
}

/// Returns `true` if `current_message` and `new_message` differ after normalization.
pub(crate) fn should_update_commit_message(current_message: &str, new_message: &str) -> bool {
    normalize_commit_message(current_message) != normalize_commit_message(new_message)
}

/// Prepares a commit message provided directly by the user.
///
/// The message is normalized and then validated to be non-empty.
pub(crate) fn prepare_provided_commit_message(message: &str) -> Result<String> {
    let normalized = normalize_commit_message(message);
    if normalized.is_empty() {
        bail!("Aborting due to empty commit message");
    }
    Ok(normalized.to_string())
}

/// Returns `true` if a commit message should be treated as multi-line for inline rewording.
///
/// Trailing whitespace and trailing newlines are ignored so that messages like `"subject\n"`
/// are still treated as single-line.
pub(crate) fn commit_message_has_multiple_lines(message: &str) -> bool {
    message.trim_end().split_once('\n').is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_trims_whitespace() {
        assert_eq!(normalize_commit_message("  hello\n"), "hello");
    }

    #[test]
    fn prepare_provided_commit_message_rejects_empty_after_trim() {
        let err = prepare_provided_commit_message(" \n\t ")
            .expect_err("whitespace-only messages must fail");
        assert_eq!(err.to_string(), "Aborting due to empty commit message");
    }

    #[test]
    fn should_update_commit_message_uses_normalized_values() {
        assert!(!should_update_commit_message("subject", "  subject\n"));
        assert!(should_update_commit_message("subject", "subject\n\nbody"));
    }

    #[test]
    fn commit_message_has_multiple_lines_ignores_trailing_newline() {
        assert!(!commit_message_has_multiple_lines("subject\n"));
        assert!(commit_message_has_multiple_lines("subject\n\nbody"));
    }
}
