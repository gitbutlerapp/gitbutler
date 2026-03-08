use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

#[cfg(feature = "legacy")]
use crate::LegacyProjectId;

/// A self-describing handle to the path of the project on disk, typically the `.git` directory.
///
/// Use it in places where you can't use `but_ctx::Context::gitdir`.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ProjectHandle(String);

/// JSON input for `project_id` parameters.
///
/// This accepts a [`ProjectHandle`] in all builds, and also accepts a legacy [`LegacyProjectId`]
/// when the `legacy` feature is enabled.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProjectHandleOrLegacyProjectId {
    /// Self-describing project handle.
    ProjectHandle(ProjectHandle),
    /// Legacy UUID project identifier.
    #[cfg(feature = "legacy")]
    LegacyProjectId(LegacyProjectId),
}

/// Lifecycle
impl ProjectHandle {
    /// Create a project handle from `path`.
    pub fn from_path(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = gix::path::realpath(path)?;
        path_to_string(&path).map(Self)
    }

    /// Consume this handle and decode it into its absolute filesystem path.
    pub fn into_path(self) -> anyhow::Result<PathBuf> {
        self.to_path()
    }
}

/// Path conversion
impl ProjectHandle {
    /// Decode this handle into its absolute filesystem path.
    pub fn to_path(&self) -> anyhow::Result<PathBuf> {
        encoded_str_to_path(self.as_raw_str())
    }
}

impl ProjectHandle {
    fn as_raw_str(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Display for ProjectHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_raw_str())
    }
}

impl std::fmt::Debug for ProjectHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ProjectHandle").field(&self.0).finish()
    }
}

impl std::str::FromStr for ProjectHandle {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let path = encoded_str_to_path(value)?;
        path_to_string(&path).map(Self)
    }
}

impl TryFrom<&ProjectHandle> for PathBuf {
    type Error = anyhow::Error;

    fn try_from(value: &ProjectHandle) -> Result<Self, Self::Error> {
        value.to_path()
    }
}

impl TryFrom<ProjectHandle> for PathBuf {
    type Error = anyhow::Error;

    fn try_from(value: ProjectHandle) -> Result<Self, Self::Error> {
        value.into_path()
    }
}

fn encoded_str_to_path(encoded: &str) -> anyhow::Result<PathBuf> {
    let bytes = decode(encoded)?;
    let path = gix::path::try_from_byte_slice(&bytes)
        .map_err(anyhow::Error::from)
        .with_context(|| {
            format!("Encoded ProjectHandle payload is not a valid filesystem path: '{encoded}'")
        })?;
    if !path.is_absolute() {
        bail!(
            "ProjectHandle payload must decode to an absolute filesystem path, got '{}'",
            path.display()
        );
    }
    Ok(path.to_owned())
}

fn path_to_string(path: &Path) -> Result<String, anyhow::Error> {
    let bytes = gix::path::os_str_into_bstr(path.as_os_str())?;
    Ok(encode(bytes))
}

fn encode(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn decode(encoded: &str) -> anyhow::Result<Vec<u8>> {
    URL_SAFE_NO_PAD
        .decode(encoded)
        .with_context(|| format!("ProjectHandle payload is not valid base64url: '{encoded}'"))
}

impl<'de> serde::Deserialize<'de> for ProjectHandle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

impl std::str::FromStr for ProjectHandleOrLegacyProjectId {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if let Ok(handle) = value.parse::<ProjectHandle>() {
            return Ok(Self::ProjectHandle(handle));
        }
        #[cfg(feature = "legacy")]
        if let Ok(project_id) = value.parse::<LegacyProjectId>() {
            return Ok(Self::LegacyProjectId(project_id));
        }
        #[cfg(feature = "legacy")]
        return Err(anyhow::anyhow!(
            "Expected `project_id` to be either a ProjectHandle or a legacy ProjectId, got '{value}'"
        ));
        #[cfg(not(feature = "legacy"))]
        return Err(anyhow::anyhow!(
            "Expected `project_id` to be a ProjectHandle, got '{value}'"
        ));
    }
}

impl<'de> serde::Deserialize<'de> for ProjectHandleOrLegacyProjectId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

impl serde::Serialize for ProjectHandleOrLegacyProjectId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            ProjectHandleOrLegacyProjectId::ProjectHandle(handle) => {
                serializer.serialize_str(&handle.to_string())
            }
            #[cfg(feature = "legacy")]
            ProjectHandleOrLegacyProjectId::LegacyProjectId(project_id) => {
                serializer.serialize_str(&project_id.to_string())
            }
        }
    }
}

impl std::fmt::Display for ProjectHandleOrLegacyProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectHandleOrLegacyProjectId::ProjectHandle(handle) => write!(f, "{handle}"),
            #[cfg(feature = "legacy")]
            ProjectHandleOrLegacyProjectId::LegacyProjectId(project_id) => {
                write!(f, "{project_id}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::path_to_string;
    use std::path::Path;

    #[test]
    fn canonical_paths() -> anyhow::Result<()> {
        struct Case {
            os: &'static str,
            canonical_path: &'static str,
            expected: &'static str,
        }

        let cases = [
            Case {
                os: "linux",
                canonical_path: "/home/alice/src/gitbutler/repo/.git",
                expected: "L2hvbWUvYWxpY2Uvc3JjL2dpdGJ1dGxlci9yZXBvLy5naXQ",
            },
            Case {
                os: "macos",
                canonical_path: "/Users/alice/Library/Application Support/GitButler/repo/.git",
                expected: "L1VzZXJzL2FsaWNlL0xpYnJhcnkvQXBwbGljYXRpb24gU3VwcG9ydC9HaXRCdXRsZXIvcmVwby8uZ2l0",
            },
            Case {
                os: "windows",
                canonical_path: r"C:\Users\alice\AppData\Local\GitButler\repo\.git",
                expected: "QzpcVXNlcnNcYWxpY2VcQXBwRGF0YVxMb2NhbFxHaXRCdXRsZXJccmVwb1wuZ2l0",
            },
            Case {
                os: "windows-unc",
                canonical_path: r"\\server\share\GitButler\repo\.git",
                expected: "XFxzZXJ2ZXJcc2hhcmVcR2l0QnV0bGVyXHJlcG9cLmdpdA",
            },
        ];

        for case in cases {
            let path = Path::new(case.canonical_path);
            let encoded = path_to_string(path)?;
            assert_eq!(encoded, case.expected, "{}: readable mismatch", case.os);
        }

        Ok(())
    }

    mod encoded_str_to_path {
        use crate::project_handle::encoded_str_to_path;

        use crate::project_handle::{decode, encode};

        #[cfg(unix)]
        #[test]
        fn illformed_utf8_payload_round_trip() -> anyhow::Result<()> {
            use crate::project_handle::encode;
            use std::os::unix::ffi::OsStrExt as _;

            let bytes = b"/tmp/\xF1\xF2\xF3\xC0\xC1\xC2";
            #[allow(invalid_from_utf8)]
            let res = std::str::from_utf8(bytes);
            assert!(res.is_err(), "this is illformed UTF8");
            let encoded = encode(bytes);

            let decoded = encoded_str_to_path(&encoded)?;
            assert_eq!(decoded.as_os_str().as_bytes(), bytes);
            Ok(())
        }

        #[test]
        fn decoding_relative_paths_is_rejected() {
            let relative_inputs = ["a", ".", "..", "a/b", "./tmp", "../tmp", "tmp/../repo"];

            for relative in relative_inputs {
                let encoded = encode(relative.as_bytes());
                assert!(
                    encoded_str_to_path(&encoded).is_err(),
                    "expected relative payload to be rejected: '{relative}'"
                );
            }
        }

        #[test]
        fn requires_urlsafe_base64_payloads() {
            assert!(encoded_str_to_path("L3RtcA").is_ok());
            assert!(encoded_str_to_path("%2Ftmp").is_err());
            assert!(encoded_str_to_path("a/b").is_err());
        }

        #[test]
        fn raw_codec_handles_full_byte_range_and_stays_url_safe() -> anyhow::Result<()> {
            let bytes: Vec<u8> = (0_u8..=255).collect();
            let encoded = encode(&bytes);
            assert!(
                encoded
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            );
            assert_eq!(decode(&encoded)?, bytes);
            Ok(())
        }

        #[test]
        fn accepts_urlsafe_text_bytes_for_absolute_paths() -> anyhow::Result<()> {
            let mut absolute_bytes = Vec::with_capacity(64);
            absolute_bytes
                .extend(b"/0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz-._~");

            let encoded = encode(&absolute_bytes);
            assert_eq!(decode(&encoded)?, absolute_bytes);
            assert!(encoded_str_to_path(&encoded).is_ok());
            Ok(())
        }

        #[test]
        fn strict_malformed_input_handling() {
            assert!(encoded_str_to_path("%2Ftmp").is_err());
            assert!(encoded_str_to_path("not+base64").is_err());
            assert!(encoded_str_to_path("a/b").is_err());
            assert!(encoded_str_to_path("a=").is_err());
        }
    }
}
