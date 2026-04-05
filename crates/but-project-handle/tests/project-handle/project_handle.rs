use std::path::{Path, PathBuf};

use but_project_handle::{ProjectHandle, ProjectHandleOrLegacyProjectId};

#[test]
fn round_trip() -> anyhow::Result<()> {
    let input = Path::new("/tmp/read me?/a+b/#test");
    let handle = ProjectHandle::from_path(input)?;
    let decoded = PathBuf::try_from(&handle)?;
    assert_eq!(decoded, gix::path::realpath(input)?);
    assert!(
        handle
            .to_string()
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    );
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
    let handle: ProjectHandle = "L3RtcC9yZWFkIG1l".parse()?;
    assert_eq!(handle.to_path()?, PathBuf::from("/tmp/read me"));
    Ok(())
}

#[test]
fn from_str_rejects_relative_payloads() {
    assert!("cmVsYXRpdmUvcGF0aA".parse::<ProjectHandle>().is_err());
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

    assert_eq!(format!("{handle}"), display);
    assert_eq!(
        format!("{handle:?}"),
        format!("ProjectHandle(\"{display}\")")
    );
    Ok(())
}

#[test]
fn project_handle_or_legacy_project_id_deserializes_project_handle() -> anyhow::Result<()> {
    let value: ProjectHandleOrLegacyProjectId = serde_json::from_str(r#""L3RtcA""#)?;
    match value {
        ProjectHandleOrLegacyProjectId::ProjectHandle(handle) => {
            assert_eq!(handle.to_string(), "L3RtcA");
        }
        #[cfg_attr(not(feature = "legacy"), allow(unreachable_patterns))]
        other => unreachable!("expected project handle variant, got {other:?}"),
    }
    Ok(())
}

#[test]
fn project_handle_or_legacy_project_id_round_trips_project_handle() -> anyhow::Result<()> {
    let value: ProjectHandleOrLegacyProjectId = serde_json::from_str(r#""L3RtcA""#)?;
    let serialized = serde_json::to_string(&value)?;
    assert_eq!(serialized, r#""L3RtcA""#);
    let decoded: ProjectHandleOrLegacyProjectId = serde_json::from_str(&serialized)?;
    assert_eq!(decoded, value);
    Ok(())
}

#[test]
#[cfg(feature = "legacy")]
fn project_handle_or_legacy_project_id_deserializes_legacy_project_id() -> anyhow::Result<()> {
    let expected = "00000000-0000-0000-0000-000000000000";
    let value: ProjectHandleOrLegacyProjectId = serde_json::from_str(&format!(r#""{expected}""#))?;
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
fn project_handle_or_legacy_project_id_round_trips_legacy_project_id() -> anyhow::Result<()> {
    let expected = "00000000-0000-0000-0000-000000000000";
    let expected_json = format!(r#""{expected}""#);
    let value: ProjectHandleOrLegacyProjectId = serde_json::from_str(&expected_json)?;
    let serialized = serde_json::to_string(&value)?;
    assert_eq!(serialized, expected_json);
    let decoded: ProjectHandleOrLegacyProjectId = serde_json::from_str(&serialized)?;
    assert_eq!(decoded, value);
    Ok(())
}
