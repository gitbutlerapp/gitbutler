use std::{path::Path, process::Stdio};

use serde::Serialize;

/// Supported editor configuration used internally to launch editors.
#[derive(Clone)]
pub struct EditorSpec<'a> {
    /// Identifier used to refer to the editor.
    pub id: &'a str,
    /// Name of the editor.
    pub name: &'a str,
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    /// Name of or path to the executable.
    pub executable: &'a str,
    #[cfg(target_os = "macos")]
    /// macOS bundle identifier for the application.
    pub bundle_identifier: &'a str,
}

/// Supported editor configuration for API clients.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "napi", napi_derive::napi(object))]
pub struct Editor {
    /// Identifier used to refer to the editor.
    pub id: String,
    /// Name of the editor.
    pub name: String,
}

impl From<&EditorSpec<'_>> for Editor {
    fn from(editor: &EditorSpec<'_>) -> Self {
        Self {
            id: editor.id.to_string(),
            name: editor.name.to_string(),
        }
    }
}

pub(crate) const EDITORS: &[EditorSpec] = &[
    EditorSpec {
        id: "cursor",
        name: "Cursor",
        #[cfg(target_os = "linux")]
        executable: "cursor",
        #[cfg(target_os = "windows")]
        executable: "Cursor.exe",
        #[cfg(target_os = "macos")]
        // This looks insane but it's actually the correct bundle ID, see https://forum.cursor.com/t/cursor-bundle-identifier/779
        bundle_identifier: "com.todesktop.230313mzl4w4u92",
    },
    EditorSpec {
        id: "sublime",
        name: "Sublime Text",
        #[cfg(target_os = "linux")]
        executable: "subl",
        #[cfg(target_os = "windows")]
        executable: "subl.exe",
        #[cfg(target_os = "macos")]
        bundle_identifier: "com.sublimetext.4",
    },
    EditorSpec {
        id: "vscode",
        name: "VS Code",
        #[cfg(target_os = "linux")]
        executable: "code",
        #[cfg(target_os = "windows")]
        executable: "code.exe",
        #[cfg(target_os = "macos")]
        bundle_identifier: "com.microsoft.VSCode",
    },
    #[cfg(target_os = "macos")]
    EditorSpec {
        id: "xcode",
        name: "Xcode",
        bundle_identifier: "com.apple.dt.Xcode",
    },
    EditorSpec {
        id: "zed",
        name: "Zed",
        #[cfg(target_os = "linux")]
        executable: "zed",
        #[cfg(target_os = "windows")]
        executable: "zed.exe",
        #[cfg(target_os = "macos")]
        bundle_identifier: "dev.zed.Zed",
    },
];

/// Low-level API to open a `path` with a specified `editor`.
///
/// # WARNING
/// It is up to the caller to assure that the `path` is safe to open and that the `editor` is safe
/// to use. Therefore, this should never be exposed to an untrusted context, such as the GUI
/// renderer.
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub fn open_in_editor_unchecked(path: &Path, editor: &EditorSpec<'_>) -> anyhow::Result<()> {
    use crate::open::spawn::spawn_and_reap;

    let mut cmd = std::process::Command::new(editor.executable);
    cmd.arg(path);
    cmd.stdout(Stdio::null()).stderr(Stdio::null());

    spawn_and_reap(cmd, editor.executable, &path.to_string_lossy())?;

    Ok(())
}

/// Low-level API to open a `path` with a specified `editor`.
///
/// # WARNING
/// It is up to the caller to assure that the `path` is safe to open and that the `editor` is safe
/// to use. Therefore, this should never be exposed to an untrusted context, such as the GUI
/// renderer.
///
/// For untrusted clients, use [`open_in_editor`] instead.
#[cfg(target_os = "macos")]
pub fn open_in_editor_unchecked(path: &Path, editor: &EditorSpec<'_>) -> anyhow::Result<()> {
    let mut cmd = std::process::Command::new("/usr/bin/open");
    let status = cmd
        .arg("-b")
        .arg(editor.bundle_identifier)
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;

    if !status.success() {
        anyhow::bail!(
            "failed to open {path:?} with editor bundle identifier '{}'",
            editor.bundle_identifier
        );
    }

    Ok(())
}
