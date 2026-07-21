//! Commit writing with signing, date handling, and Gerrit trailer support.
//!
//! This is the home of the commit-creation wrapper; it lives here so
//! graph-level rebasing (`but-graph`) and higher layers share one
//! implementation.
use std::fmt::Display;

use anyhow::{Context as _, bail};
use bstr::{BStr, BString, ByteSlice};
use gix::config::Source;

use crate::{
    ChangeId, RepositoryExt,
    commit::{Headers, SignCommit},
};

/// What to do with the committer (actor) and the commit time when [creating a new commit](create()).
#[derive(Debug, Copy, Clone)]
pub enum DateMode {
    /// Update both the committer and author time.
    CommitterUpdateAuthorUpdate,
    /// Obtain the current committer and the current local time and update it, keeping only the author time.
    CommitterUpdateAuthorKeep,
    /// Keep the currently set committer-time and author-time.
    CommitterKeepAuthorKeep,
}

/// Set `user.name` to `name` if unset and `user.email` to `email` if unset, or error if both are already set
/// as per `repo` configuration, and write the changes back to the file at `destination`, keeping
/// user comments and custom formatting.
pub fn save_author_if_unset_in_repo<'a, 'b>(
    repo: &gix::Repository,
    destination: Source,
    name: impl Into<&'a BStr>,
    email: impl Into<&'b BStr>,
) -> anyhow::Result<()> {
    let config = repo.config_snapshot();
    let name = config
        .string(gix::config::tree::User::NAME)
        .is_none()
        .then_some(name.into());
    let email = config
        .string(gix::config::tree::User::EMAIL)
        .is_none()
        .then_some(email.into());
    let config_path = destination
        .storage_location(&mut |name| std::env::var_os(name))
        .context("Failed to determine storage location for Git user configuration")?;
    // TODO(gix): there should be a `gix::Repository` version of this that takes care of this detail.
    let config_path = if config_path.is_relative() {
        if destination == gix::config::Source::Local {
            repo.common_dir().join(config_path)
        } else {
            repo.git_dir().join(config_path)
        }
    } else {
        config_path.into_owned()
    };

    if !config_path.exists() {
        std::fs::create_dir_all(config_path.parent().context("Git user config is never /")?)?;
        std::fs::File::create(&config_path)?;
    }

    let mut config = gix::config::File::from_path_no_includes(config_path.clone(), destination)?;
    let mut something_was_set = false;
    if let Some(name) = name {
        config.set_raw_value(gix::config::tree::User::NAME, name)?;
        something_was_set = true;
    }
    if let Some(email) = email {
        config.set_raw_value(gix::config::tree::User::EMAIL, email)?;
        something_was_set = true;
    }

    if !something_was_set {
        bail!("Refusing to overwrite an existing user.name and user.email");
    }

    config.write_to(
        &mut std::fs::OpenOptions::new()
            .write(true)
            .create(false)
            .truncate(true)
            .open(config_path)?,
    )?;

    Ok(())
}

/// Use the given `commit` and possibly sign it, replacing a possibly existing signature,
/// or removing the signature if GitButler is not configured to keep it.
///
/// Signatures will be removed automatically if signing is disabled to prevent an amended commit
/// to use the old signature.
///
/// change_id can be used to either ste or override the existing change_id
/// header.
pub fn create(
    repo: &gix::Repository,
    mut commit: gix::objs::Commit,
    committer: DateMode,
    sign_commit: SignCommit,
    change_id: Option<ChangeId>,
) -> anyhow::Result<gix::ObjectId> {
    match committer {
        DateMode::CommitterUpdateAuthorKeep => {
            update_committer(repo, &mut commit)?;
        }
        DateMode::CommitterKeepAuthorKeep => {}
        DateMode::CommitterUpdateAuthorUpdate => {
            update_committer(repo, &mut commit)?;
            update_author_time(repo, &mut commit)?;
        }
    }
    let settings = repo.git_settings()?;
    if settings.gitbutler_gerrit_mode.unwrap_or(false) {
        set_gerrit_trailers(&mut commit);
    }

    if let Some(change_id) = change_id {
        let mut headers = Headers::try_from_commit(&commit).unwrap_or_else(Headers::empty);
        headers.change_id = Some(change_id);

        headers.set_in_commit(&mut commit);
    }

    crate::commit::create(repo, commit, None, sign_commit)
}

/// Update the committer of `commit` to be the current one.
pub fn update_committer(repo: &gix::Repository, commit: &mut gix::objs::Commit) -> anyhow::Result<()> {
    commit.committer = repo
        .committer()
        .transpose()?
        .context("Need committer to be configured when creating a new commit")?
        .into();
    Ok(())
}

/// Update only the author-time of `commit`.
pub fn update_author_time(
    repo: &gix::Repository,
    commit: &mut gix::objs::Commit,
) -> anyhow::Result<()> {
    let author = repo
        .author()
        .transpose()?
        .context("Need author to be configured when creating a new commit")?;
    commit.author.time = author.time()?;
    Ok(())
}

/// A Gerrit-style change id (`I<sha1-of-change-id>`), derived from GitButler's own change id.
#[derive(Clone, Debug)]
pub struct GerritChangeId(String);

impl From<&ChangeId> for GerritChangeId {
    fn from(value: &ChangeId) -> Self {
        let mut hash = gix::hash::hasher(gix::hash::Kind::Sha1);
        hash.update((*value).as_bytes());
        Self(format!(
            "I{hex_hash_of_change_id}",
            hex_hash_of_change_id = hash.try_finalize().expect("no SHATTERED attack").to_hex()
        ))
    }
}
impl Display for GerritChangeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Add a `Change-Id: I…` trailer derived from the commit's GitButler change id
/// to the commit message, if it has a change id and no such trailer yet.
pub fn set_gerrit_trailers(commit: &mut gix::objs::Commit) {
    if let Some(headers) = Headers::try_from_commit(commit)
        && let Some(change_id) = headers.change_id
    {
        commit.message = with_change_id_trailer(commit.message.clone(), change_id);
    }
}

fn with_change_id_trailer(msg: BString, change_id: ChangeId) -> BString {
    let change_id = GerritChangeId::from(&change_id);
    let change_id_line = format!("Change-Id: {change_id}");
    let msg_bytes = msg.as_slice();

    if msg_bytes.find(b"\nChange-Id:").is_some() {
        return msg;
    }

    let lines: Vec<&[u8]> = msg_bytes.lines().collect();

    let is_trailer = |line: &[u8]| -> bool {
        if line.is_empty() {
            return false;
        }
        // A trailer has format "Token: value"
        if let Some(colon_pos) = line.find_byte(b':') {
            if colon_pos == 0 {
                return false;
            }
            let token = &line[..colon_pos];
            !token.contains(&b' ') && colon_pos + 1 < line.len()
        } else {
            false
        }
    };

    let mut last_non_empty = lines.len();
    for (i, line) in lines.iter().enumerate().rev() {
        if !line.is_empty() {
            last_non_empty = i + 1;
            break;
        }
    }

    let mut insert_pos = last_non_empty;
    let mut found_signed_off_by = false;
    let mut found_any_trailer = false;

    for i in (0..last_non_empty).rev() {
        let line = lines[i];

        if is_trailer(line) {
            found_any_trailer = true;
            if line.starts_with(b"Signed-off-by:") {
                found_signed_off_by = true;
                insert_pos = i;
            } else if !found_signed_off_by {
                // This is a non-Signed-off-by trailer, insert after it
                insert_pos = i + 1;
            }
        } else if !line.is_empty() {
            break;
        }
    }

    let mut result = BString::from(Vec::new());
    for (i, line) in lines.iter().enumerate() {
        if i == insert_pos {
            result.extend_from_slice(change_id_line.as_bytes());
            result.push(b'\n');
        }
        result.extend_from_slice(line);
        result.push(b'\n');
    }

    // If we're inserting at the end and didn't insert yet
    if insert_pos == lines.len() {
        // Only add a blank line separator if there were NO trailers found
        // (i.e., we're creating a new trailer block from scratch)
        // If there were trailers, we're appending to the existing trailer block
        if !found_any_trailer && !lines.is_empty() {
            let needs_separator = if let Some(last_line) = lines.last() {
                !last_line.is_empty()
            } else {
                false
            };
            if needs_separator {
                result.push(b'\n');
            }
        }
        result.extend_from_slice(change_id_line.as_bytes());
        result.push(b'\n');
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_is_41_characters_long() {
        let commit_change_id = ChangeId::generate();
        let change_id = GerritChangeId::from(&commit_change_id);
        let output = format!("{change_id}");
        assert_eq!(output.len(), 41); // "I" + 40 hex chars
        assert!(output.starts_with('I'));
    }

    #[test]
    fn test_add_trailer_no_existing_trailers() {
        let commit_change_id = ChangeId::generate();
        let change_id = GerritChangeId::from(&commit_change_id);
        let change_id_line = format!("Change-Id: {change_id}\n");

        let msg = BString::from("Initial commit\n");
        let updated_msg = with_change_id_trailer(msg.clone(), commit_change_id);
        assert!(
            updated_msg
                .as_slice()
                .windows(change_id_line.len())
                .any(|w| w == change_id_line.as_bytes())
        );
    }

    #[test]
    fn test_add_trailer_already_has_change_id() {
        let commit_change_id = ChangeId::generate();
        let change_id = GerritChangeId::from(&commit_change_id);
        let change_id_line = format!("Change-Id: {change_id}\n");

        let msg_with_change_id = BString::from(format!("Initial commit\n{change_id_line}"));
        let updated_msg = with_change_id_trailer(msg_with_change_id.clone(), commit_change_id);
        assert_eq!(updated_msg, msg_with_change_id);
    }

    #[test]
    fn test_add_trailer_with_signed_off_by() {
        let commit_change_id = ChangeId::generate();
        let change_id = GerritChangeId::from(&commit_change_id);
        let change_id_line = format!("Change-Id: {change_id}\n");

        let msg_with_signed_off =
            BString::from("Initial commit\n\nSigned-off-by: User <alice@example.com>\n");
        let updated_msg =
            with_change_id_trailer(msg_with_signed_off.clone(), commit_change_id.clone());
        let updated_msg_str = updated_msg.as_bstr();
        let change_id_index = updated_msg_str.find(&change_id_line).unwrap();
        let signed_off_index = updated_msg_str.find("Signed-off-by:").unwrap();
        assert!(change_id_index < signed_off_index);

        // Case 4: Has Pick-to trailer (no extra blank line should be added)
        let msg_with_pick_to = BString::from(
            "macOS: Handle non-square system tray notification icons\n\
             If the provided icon is non-square the system will end up clipping it,\n\
             so let's pre-generate a square icon if needed.\n\
             \n\
             Pick-to: 6.10\n",
        );
        let updated_msg =
            with_change_id_trailer(msg_with_pick_to.clone(), commit_change_id.clone());
        let updated_msg_str = updated_msg.to_string();

        assert!(updated_msg_str.contains(&format!("Pick-to: 6.10\n{change_id_line}")));

        assert!(
            !updated_msg_str.contains("Pick-to: 6.10\n\nChange-Id:"),
            "Should not have blank line between trailers"
        );

        // Case 5: Has multiple trailers including Signed-off-by
        let msg_with_multiple = BString::from(
            "Fix bug in authentication\n\
             \n\
             Pick-to: 6.10\n\
             Acked-by: Reviewer <reviewer@example.com>\n\
             Signed-off-by: Author <author@example.com>\n",
        );
        let updated_msg = with_change_id_trailer(msg_with_multiple.clone(), commit_change_id);
        let updated_msg_str = updated_msg.to_string();

        let acked_by_idx = updated_msg_str.find("Acked-by:").unwrap();
        let change_id_idx = updated_msg_str.find("Change-Id:").unwrap();
        let signed_off_idx = updated_msg_str.find("Signed-off-by:").unwrap();

        assert!(
            acked_by_idx < change_id_idx && change_id_idx < signed_off_idx,
            "Change-Id should be between Acked-by and Signed-off-by"
        );

        assert!(
            !updated_msg_str.contains("Acked-by: Reviewer <reviewer@example.com>\n\nChange-Id:"),
            "Should not have blank line after Acked-by"
        );
        assert!(
            !updated_msg_str.contains(&format!("{change_id_line}\nSigned-off-by:")),
            "Should not have blank line after Change-Id"
        );
    }
}
