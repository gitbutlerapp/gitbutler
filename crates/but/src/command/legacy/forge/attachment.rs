//! Files attached to a review description with `but pr new --attach`.
//!
//! Files are uploaded to gitbutler.com and linked by URL, because no forge accepts
//! file bytes through its review API — a description can only ever carry a link.
//! This mirrors what the desktop app and Lite insert when a file is attached there.

use std::path::{Path, PathBuf};

use anyhow::Context as _;

use crate::error::{BadInput, bad_input};

const ARG_NAME: &str = "--attach";

/// A file named by `--attach`, checked to exist and to fit the upload limit.
#[derive(Debug)]
pub struct Attachment {
    /// The path exactly as given, which is what a description would reference it by.
    path: String,
    /// Alt text for an image, or link text for any other file.
    alt: Option<String>,
}

/// An [`Attachment`] that now lives at a public URL.
#[derive(Debug)]
pub struct UploadedAttachment {
    pub(super) path: String,
    pub(super) label: String,
    pub(super) url: String,
    pub(super) is_image: bool,
}

/// Parse and check `<file>#<alt text>` specs before anything is pushed or uploaded,
/// so a typo fails the command while it is still free to rerun.
///
/// A spec naming an existing file is taken whole, so a path that itself contains
/// `#` still works; otherwise the first `#` starts the alt text. Uploading needs a
/// GitButler account, so this also fails when none is signed in.
pub fn parse(specs: &[String]) -> Result<Vec<Attachment>, BadInput> {
    if specs.is_empty() {
        return Ok(Vec::new());
    }

    let attachments = specs
        .iter()
        .map(|spec| parse_one(spec))
        .collect::<Result<Vec<_>, _>>()?;

    let signed_in = but_api::legacy::users::get_user_profile_local()
        .map_err(|err| bad_input(format!("Could not read the GitButler account: {err:#}")))?
        .is_some();
    if !signed_in {
        return Err(
            bad_input("Attaching files needs a GitButler account, and none is signed in.")
                .arg_name(ARG_NAME)
                .hint("Sign in to GitButler in the desktop app, then run this again."),
        );
    }

    Ok(attachments)
}

fn parse_one(spec: &str) -> Result<Attachment, BadInput> {
    let (path, alt) = if Path::new(spec).is_file() {
        (spec, None)
    } else {
        match spec.split_once('#') {
            Some((path, alt)) => (path, Some(alt.trim()).filter(|alt| !alt.is_empty())),
            None => (spec, None),
        }
    };

    let metadata = std::fs::metadata(path)
        .ok()
        .filter(|metadata| metadata.is_file())
        .ok_or_else(|| {
            bad_input(format!("'{path}' is not a file."))
                .arg_name(ARG_NAME)
                .arg_value(spec)
        })?;
    let limit = gitbutler_user::api::UPLOAD_SIZE_LIMIT as u64;
    if metadata.len() > limit {
        return Err(bad_input(format!(
            "'{path}' is {:.1} MB, over the {} MB upload limit.",
            metadata.len() as f64 / (1024.0 * 1024.0),
            limit / (1024 * 1024)
        ))
        .arg_name(ARG_NAME)
        .arg_value(spec));
    }

    Ok(Attachment {
        path: path.to_owned(),
        alt: alt.map(str::to_owned),
    })
}

/// Upload every attachment, stopping at the first failure so that no review is
/// created with some of its files missing.
pub fn upload(attachments: &[Attachment]) -> anyhow::Result<Vec<UploadedAttachment>> {
    use base64::Engine as _;

    attachments
        .iter()
        .map(|attachment| {
            let path = PathBuf::from(&attachment.path);
            let bytes = std::fs::read(&path)
                .with_context(|| format!("Failed to read attachment '{}'", attachment.path))?;
            let filename = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| attachment.path.clone());
            let upload =
                but_api::legacy::users::upload_file(gitbutler_user::api::UploadFileParams {
                    filename,
                    // The server takes the type from the upload, and whether the file is
                    // embedded as an image follows from it.
                    content_type: mime_guess::from_path(&path)
                        .first()
                        .map(|mime| mime.to_string()),
                    data_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
                })
                .with_context(|| format!("Failed to upload attachment '{}'", attachment.path))?;
            Ok(UploadedAttachment {
                path: attachment.path.clone(),
                label: attachment.alt.clone().unwrap_or(upload.filename),
                url: upload.url,
                is_image: upload.is_image,
            })
        })
        .collect()
}

/// Link `uploads` from `body`.
///
/// Where the body already references an attachment's path, as in
/// `![alt](./login.png)`, that reference is pointed at the upload and keeps its own
/// text; every other attachment is appended, images embedded and other files linked.
pub fn link_in_body(body: &str, uploads: &[UploadedAttachment]) -> String {
    let mut body = body.to_owned();
    let mut appended = Vec::new();
    for upload in uploads {
        let bare = upload.path.strip_prefix("./").unwrap_or(&upload.path);
        let mut referenced = false;
        for target in [bare.to_owned(), format!("./{bare}")] {
            let reference = format!("]({target})");
            if body.contains(&reference) {
                body = body.replace(&reference, &format!("]({})", upload.url));
                referenced = true;
            }
        }
        if !referenced {
            let embed = if upload.is_image { "!" } else { "" };
            appended.push(format!("{embed}[{}]({})", upload.label, upload.url));
        }
    }

    if appended.is_empty() {
        return body;
    }
    let appended = appended.join("\n");
    if body.trim().is_empty() {
        appended
    } else {
        format!("{}\n\n{appended}", body.trim_end())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uploaded(path: &str, label: &str, is_image: bool) -> UploadedAttachment {
        UploadedAttachment {
            path: path.to_owned(),
            label: label.to_owned(),
            url: format!("https://uploads.example/{label}"),
            is_image,
        }
    }

    #[test]
    fn appends_images_embedded_and_other_files_linked() {
        let body = link_in_body(
            "Fixes the login.\n",
            &[
                uploaded("./login.png", "The login error", true),
                uploaded("trace.log", "trace.log", false),
            ],
        );

        // One blank line separates the author's text from the attachments.
        snapbox::assert_data_eq!(
            body,
            snapbox::str![[r#"
Fixes the login.

![The login error](https://uploads.example/The login error)
[trace.log](https://uploads.example/trace.log)
"#]]
        );
    }

    #[test]
    fn empty_body_is_just_the_attachments() {
        let body = link_in_body("", &[uploaded("shot.png", "shot.png", true)]);

        assert_eq!(
            body, "![shot.png](https://uploads.example/shot.png)",
            "no leading blank lines before the only content"
        );
    }

    #[test]
    fn rewrites_a_reference_instead_of_appending() {
        let body = link_in_body(
            "Before:\n\n![broken state](login.png)\n\nand `login.png` stays text.",
            &[uploaded("./login.png", "login.png", true)],
        );

        // The reference keeps its own alt text and the file is not appended again.
        snapbox::assert_data_eq!(
            body,
            snapbox::str![[r#"
Before:

![broken state](https://uploads.example/login.png)

and `login.png` stays text.
"#]]
        );
    }

    #[test]
    fn parse_splits_alt_text_at_the_first_hash() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("login.png");
        std::fs::write(&file, b"png").expect("write file");
        let spec = format!("{}#Issue #12 state", file.display());

        let attachment = parse_one(&spec).expect("existing file parses");

        assert_eq!(attachment.path, file.display().to_string());
        assert_eq!(
            attachment.alt.as_deref(),
            Some("Issue #12 state"),
            "alt text may contain '#' itself"
        );
    }

    #[test]
    fn parse_takes_an_existing_path_with_a_hash_whole() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("shot#1.png");
        std::fs::write(&file, b"png").expect("write file");
        let spec = file.display().to_string();

        let attachment = parse_one(&spec).expect("existing file parses");

        assert_eq!(attachment.path, spec, "the '#' belongs to the file name");
        assert_eq!(attachment.alt, None);
    }

    #[test]
    fn parse_rejects_a_missing_file() {
        let err = parse_one("does-not-exist.png#alt").expect_err("missing file is bad input");

        assert_eq!(err.argument_name(), Some(ARG_NAME));
    }
}
