use std::collections::BTreeSet;

use anyhow::Context;
use bstr::{BString, ByteSlice, ByteVec};
use gix::refs;
use unicode_segmentation::UnicodeSegmentation;

use crate::{RepositoryExt, branch::normalize_short_name, extract_remote_name_and_short_name};

/// Produce a branch named after the author configured in `repo`, or default it to a generic name
pub fn canned_refname(repo: &gix::Repository) -> anyhow::Result<gix::refs::FullName> {
    let short_name = match repo.commit_signatures().map(|(author, _committer)| author) {
        Ok(author) => generate_short_name_from_signature(&author)?,
        Err(_) => "branch-1".into(),
    };

    Ok(refs::Category::LocalBranch.to_full_name(short_name.as_bstr())?)
}

/// Like [`canned_refname`], but produces a unique name that at the time of return form this function
/// didn't exist in the repository.
/// It uses `template` as starting point for its search, appending or incrementing a number until a
/// reference of that name doesn't exist anymore in `repo`.
/// `rn` is returned unchanged if the reference *already* doesn't exist.
///
/// ### Note
///
/// This function is racy and two callers could see the same name. Only one will then be able
/// to crate the branch. This is acceptable and common, i.e. the name is first presented to the user,
/// then later it's used.
pub fn find_unique_refname(
    repo: &gix::Repository,
    template: &gix::refs::FullNameRef,
) -> anyhow::Result<gix::refs::FullName> {
    // TODO(perf): ideally we remove that special case of auto-associating local branches with seemingly matching
    //             RTBs, and avoid this lookup and reference traversal.
    let remote_names = repo.remote_names();
    let rtb_lut: BTreeSet<_> = repo
        .references()?
        .prefixed("refs/remotes/")?
        .filter_map(Result::ok)
        .filter_map(|rn| {
            extract_remote_name_and_short_name(rn.name(), &remote_names)
                .map(|(_rn, short_name)| short_name)
        })
        .collect();

    // Check if the original name is available
    let short_name = template.shorten();
    if !rtb_lut.contains(short_name) && repo.try_find_reference(template)?.is_none() {
        return Ok(template.to_owned());
    }

    // Extract base name and number if it already has a numerical suffix
    let trailing_number = short_name
        .rfind("-")
        .map(|pos| (&short_name[pos + 1..], pos))
        .and_then(|(maybe_num, pos)| {
            let num = maybe_num.to_str().ok()?.parse::<usize>().ok()?;
            Some((&short_name[..pos], num + 1))
        });
    let (base, start_num) = trailing_number.unwrap_or((short_name, 1));

    // Try incrementing numbers until we find one that doesn't exist
    let category = template
        .category()
        .with_context(|| format!("Input branch {template} could not be categorized"))?;
    let mut candidate_short = BString::default();
    for num in start_num.. {
        candidate_short.clear();
        candidate_short.push_str(base);
        candidate_short.push(b'-');
        candidate_short.push_str(num.to_string());

        if rtb_lut.contains(candidate_short.as_bstr()) {
            continue;
        }
        let candidate_full = category.to_full_name(candidate_short.as_bstr())?;

        if repo.try_find_reference(&candidate_full)?.is_none() {
            return Ok(candidate_full);
        }
    }
    unreachable!("infinite loop should find a unique name")
}

/// Find a new unique branch name, one that doesn't exist in `repo` yet.
///
/// See caveats of [`find_unique_refname()`].
pub fn unique_canned_refname(repo: &gix::Repository) -> anyhow::Result<gix::refs::FullName> {
    let name = canned_refname(repo)?;
    find_unique_refname(repo, name.as_ref())
}

fn generate_short_name_from_signature(author: &gix::actor::Signature) -> anyhow::Result<BString> {
    let name = author.name.to_str_lossy();
    // Split by whitespace to get words
    let words: Vec<&str> = name.split_whitespace().collect();

    // Try to extract initials from words
    let prefix: Vec<&str> = words
        .iter()
        .filter_map(|word| {
            // Find the first usable character from the word
            for grapheme in word.graphemes(true) {
                let Some(c) = grapheme.chars().next() else {
                    continue;
                };

                // For Chinese, Japanese, Korean characters, include them directly
                if is_cjk(c) {
                    return Some(grapheme);
                }

                // For other scripts, only include alphabetic characters (excluding RTL scripts)
                if c.is_alphabetic() && !is_rtl_script(c) {
                    return Some(grapheme);
                }
            }
            None
        })
        .collect();

    let is_primarily_direct_chars = || {
        prefix.iter().all(|s| {
            let first_char = s.chars().next();
            first_char.is_some_and(is_cjk)
        })
    };
    let branch_name = if prefix.is_empty() {
        "branch-1".to_string()
    } else if is_primarily_direct_chars() {
        // For CJK and emojis, take up to 3 characters directly from the name
        let direct_chars: String = name
            .graphemes(true)
            .filter(|g| {
                let first_char = g.chars().next();
                first_char.is_some_and(is_cjk)
            })
            .take(3)
            .collect();
        format!("{direct_chars}-branch-1")
    } else {
        // For other scripts, use initials
        format!(
            "{}-branch-1",
            prefix
                .into_iter()
                .map(|p| p.to_lowercase())
                .collect::<String>()
        )
    };

    normalize_short_name(&*branch_name)
}

/// Check if a character belongs to a right-to-left script
fn is_rtl_script(c: char) -> bool {
    matches!(c,
        '\u{0600}'..='\u{06FF}' | // Arabic
        '\u{0750}'..='\u{077F}' | // Arabic Supplement
        '\u{08A0}'..='\u{08FF}' | // Arabic Extended-A
        '\u{FB50}'..='\u{FDFF}' | // Arabic Presentation Forms-A
        '\u{FE70}'..='\u{FEFF}' | // Arabic Presentation Forms-B
        '\u{0590}'..='\u{05FF}' | // Hebrew
        '\u{FB1D}'..='\u{FB4F}'   // Hebrew Presentation Forms
    )
}

/// Check if a character is CJK (Chinese, Japanese, Korean)
fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}' | // CJK Unified Ideographs
        '\u{3400}'..='\u{4DBF}' | // CJK Unified Ideographs Extension A
        '\u{20000}'..='\u{2A6DF}' | // CJK Unified Ideographs Extension B
        '\u{2A700}'..='\u{2B73F}' | // CJK Unified Ideographs Extension C
        '\u{2B740}'..='\u{2B81F}' | // CJK Unified Ideographs Extension D
        '\u{2B820}'..='\u{2CEAF}' | // CJK Unified Ideographs Extension E
        '\u{F900}'..='\u{FAFF}' | // CJK Compatibility Ideographs
        '\u{2F800}'..='\u{2FA1F}' | // CJK Compatibility Ideographs Supplement
        '\u{3040}'..='\u{309F}' | // Hiragana
        '\u{30A0}'..='\u{30FF}' | // Katakana
        '\u{AC00}'..='\u{D7AF}'   // Hangul Syllables
    )
}
