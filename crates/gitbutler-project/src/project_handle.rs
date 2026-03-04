use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail};

#[cfg(feature = "legacy")]
use crate::LegacyProjectId;

/// A self-describing handle to the path of the project on disk, typically the `.git` directory.
///
/// Note: this currently lives in `gitbutler-project` to avoid layering violations while legacy
/// project storage still exists. Once `gitbutler-project` is dissolved, this type should move
/// back to `but-ctx`.
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
    urlencoding::encode_binary(bytes).into_owned()
}

fn decode(encoded: &str) -> anyhow::Result<Vec<u8>> {
    validate_encoded(encoded)?;
    Ok(urlencoding::decode_binary(encoded.as_bytes()).into_owned())
}

fn validate_encoded(encoded: &str) -> anyhow::Result<()> {
    fn is_unreserved(byte: u8) -> bool {
        // This character set is copied from the URL encoding spec for binary encodings.
        byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
    }
    fn hex_value(hex_bytes: [u8; 2]) -> anyhow::Result<u8> {
        let mut out = [0_u8; 1];
        hex::decode_to_slice(hex_bytes, &mut out)
            .context("Invalid hex digit in encoded ProjectHandle")?;
        Ok(out[0])
    }

    let bytes = encoded.as_bytes();
    let mut pos = 0;
    while pos < bytes.len() {
        let byte = bytes[pos];
        if byte == b'%' {
            if pos + 2 >= bytes.len() {
                bail!("Incomplete percent escape in encoded ProjectHandle: '{encoded}'");
            }
            hex_value([bytes[pos + 1], bytes[pos + 2]])?;
            pos += 3;
        } else {
            if !is_unreserved(byte) {
                bail!(
                    "Encoded ProjectHandle payload contains non-unreserved byte, use percent escapes: '{encoded}'"
                );
            }
            pos += 1;
        }
    }
    Ok(())
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
    use std::path::{Path, PathBuf};

    use super::path_to_string;
    use crate::ProjectHandle;

    #[test]
    fn round_trip() -> anyhow::Result<()> {
        let input = Path::new("/tmp/read me?/a+b/#test");
        let handle = ProjectHandle::from_path(input)?;
        let decoded = PathBuf::try_from(&handle)?;
        assert_eq!(decoded, gix::path::realpath(input)?);
        assert!(handle.as_raw_str().contains('%'));
        Ok(())
    }

    #[test]
    fn from_path_and_decode_round_trip() -> anyhow::Result<()> {
        let input = Path::new("/tmp/read me?/a+b/#test");
        let handle = ProjectHandle::from_path(input)?;
        let decoded = handle.to_path()?;
        assert_eq!(decoded, gix::path::realpath(input)?);
        Ok(())
    }

    #[test]
    fn from_str_round_trip() -> anyhow::Result<()> {
        let handle: ProjectHandle = "%2Ftmp%2Fread%20me".parse()?;
        assert_eq!(handle.to_path()?, PathBuf::from("/tmp/read me"));
        Ok(())
    }

    #[test]
    fn from_str_rejects_relative_payloads() {
        assert!("relative%2Fpath".parse::<ProjectHandle>().is_err());
    }

    #[test]
    fn convenience_path_methods_round_trip() -> anyhow::Result<()> {
        let input = Path::new("/tmp/read me?/a+b/#test");
        let handle = ProjectHandle::from_path(input)?;
        let canonical = gix::path::realpath(input)?;

        assert_eq!(handle.to_path()?, canonical);
        assert_eq!(handle.clone().into_path()?, canonical);
        Ok(())
    }

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
                expected: "%2Fhome%2Falice%2Fsrc%2Fgitbutler%2Frepo%2F.git",
            },
            Case {
                os: "macos",
                canonical_path: "/Users/alice/Library/Application Support/GitButler/repo/.git",
                expected: "%2FUsers%2Falice%2FLibrary%2FApplication%20Support%2FGitButler%2Frepo%2F.git",
            },
            Case {
                os: "windows",
                canonical_path: r"C:\Users\alice\AppData\Local\GitButler\repo\.git",
                expected: "C%3A%5CUsers%5Calice%5CAppData%5CLocal%5CGitButler%5Crepo%5C.git",
            },
            Case {
                os: "windows-unc",
                canonical_path: r"\\server\share\GitButler\repo\.git",
                expected: "%5C%5Cserver%5Cshare%5CGitButler%5Crepo%5C.git",
            },
        ];

        for case in cases {
            let path = Path::new(case.canonical_path);
            let encoded = path_to_string(path)?;
            assert_eq!(encoded, case.expected, "{}: readable mismatch", case.os);
        }

        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn minimal_illformed_utf8_round_trip() -> anyhow::Result<()> {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt as _;
        let input: PathBuf = OsString::from_vec(b"/tmp/\xFF".into()).into();
        let handle = ProjectHandle::from_path(&input)?;

        let canonical = gix::path::realpath(&input)?;
        assert_eq!(handle.to_path()?, canonical);
        Ok(())
    }

    #[test]
    fn display_and_debug_print() -> anyhow::Result<()> {
        let input = Path::new("/tmp/read me?/a+b/#test");
        let handle = ProjectHandle::from_path(input)?;
        let display = handle.to_string();

        assert_eq!(display, handle.as_raw_str());
        assert_eq!(
            format!("{handle:?}"),
            format!("ProjectHandle(\"{display}\")")
        );
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
            let relative_inputs = [
                "a",
                ".",
                "..",
                "a%2Fb",
                "%2E%2Ftmp",
                "%2E%2E%2Ftmp",
                "tmp%2F..%2Frepo",
            ];

            for encoded in relative_inputs {
                assert!(
                    encoded_str_to_path(encoded).is_err(),
                    "expected relative payload to be rejected: '{encoded}'"
                );
            }
        }

        #[test]
        fn requires_escaping_reserved_bytes() {
            assert!(encoded_str_to_path("%2Fa%2Fb").is_ok());
            assert!(encoded_str_to_path("a/b").is_err());
        }

        #[test]
        fn accepts_entire_unreserved_set() -> anyhow::Result<()> {
            let mut bytes = Vec::new();
            bytes.extend(b"0123456789");
            bytes.extend(b"ABCDEFGHIJKLMNOPQRSTUVWXYZ");
            bytes.extend(b"abcdefghijklmnopqrstuvwxyz");
            bytes.extend(b"-._~");

            let mut absolute_bytes = Vec::with_capacity(bytes.len() + 1);
            absolute_bytes.push(b'/');
            absolute_bytes.extend_from_slice(&bytes);

            let encoded = encode(&absolute_bytes);
            assert_eq!(decode(&encoded)?, absolute_bytes);
            assert!(encoded_str_to_path(&encoded).is_ok());

            Ok(())
        }

        #[test]
        fn rejects_unescaped_reserved_bytes_on_absolute_paths() -> anyhow::Result<()> {
            let with_unescaped_separators = "/tmp";
            assert!(encoded_str_to_path(with_unescaped_separators).is_err());
            Ok(())
        }

        #[test]
        fn strict_malformed_input_handling() {
            assert!(encoded_str_to_path("%G0").is_err());
            assert!(encoded_str_to_path("not+base64").is_err());
            assert!(encoded_str_to_path("a%2Fb").is_err());
        }

        #[test]
        fn rejects_malformed_escapes_on_absolute_paths() -> anyhow::Result<()> {
            let bad_hex = "/tmp/%G0";
            assert!(encoded_str_to_path(bad_hex).is_err());

            let incomplete = "/tmp/%";
            assert!(encoded_str_to_path(incomplete).is_err());

            let one_hex_digit = "/tmp/%2";
            assert!(encoded_str_to_path(one_hex_digit).is_err());

            Ok(())
        }
    }

    mod project_handle_or_legacy_project_id {
        use crate::ProjectHandleOrLegacyProjectId;

        #[test]
        fn project_handle_or_legacy_project_id_deserializes_project_handle() -> anyhow::Result<()> {
            let value: ProjectHandleOrLegacyProjectId = serde_json::from_str(r#""%2Ftmp""#)?;
            match value {
                ProjectHandleOrLegacyProjectId::ProjectHandle(handle) => {
                    assert_eq!(handle.to_string(), "%2Ftmp");
                }
                #[cfg_attr(not(feature = "legacy"), allow(unreachable_patterns))]
                other => unreachable!("expected project handle variant, got {other:?}"),
            }
            Ok(())
        }

        #[test]
        fn project_handle_or_legacy_project_id_round_trips_project_handle() -> anyhow::Result<()> {
            let value: ProjectHandleOrLegacyProjectId = serde_json::from_str(r#""%2Ftmp""#)?;
            let serialized = serde_json::to_string(&value)?;
            assert_eq!(serialized, r#""%2Ftmp""#);
            let decoded: ProjectHandleOrLegacyProjectId = serde_json::from_str(&serialized)?;
            assert_eq!(decoded, value);
            Ok(())
        }

        #[test]
        #[cfg(feature = "legacy")]
        fn project_handle_or_legacy_project_id_deserializes_legacy_project_id() -> anyhow::Result<()>
        {
            let expected = "00000000-0000-0000-0000-000000000000";
            let value: ProjectHandleOrLegacyProjectId =
                serde_json::from_str(&format!(r#""{expected}""#))?;
            match value {
                ProjectHandleOrLegacyProjectId::LegacyProjectId(project_id) => {
                    assert_eq!(project_id.to_string(), expected);
                }
                other => panic!("expected legacy project id variant, got {other:?}"),
            }
            Ok(())
        }

        #[test]
        #[cfg(feature = "legacy")]
        fn project_handle_or_legacy_project_id_round_trips_legacy_project_id() -> anyhow::Result<()>
        {
            let expected = "00000000-0000-0000-0000-000000000000";
            let expected_json = format!(r#""{expected}""#);
            let value: ProjectHandleOrLegacyProjectId = serde_json::from_str(&expected_json)?;
            let serialized = serde_json::to_string(&value)?;
            assert_eq!(serialized, expected_json);
            let decoded: ProjectHandleOrLegacyProjectId = serde_json::from_str(&serialized)?;
            assert_eq!(decoded, value);
            Ok(())
        }
    }
}
