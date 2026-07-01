use std::{
    ffi::OsString,
    path::Path,
    process::{Command, Stdio},
};

#[cfg(target_os = "macos")]
use std::path::PathBuf;

use crate::open::spawn::spawn_and_reap;

use serde::Serialize;

/// Supported editor configuration used internally to launch editors.
#[derive(Clone)]
pub struct EditorSpec<'a> {
    /// Identifier used to refer to the editor.
    pub id: &'a str,
    /// Name of the editor.
    pub name: &'a str,
    /// The CLI argument formatter for e.g. opening a specific line in a file.
    cli_arg_supplier: CliArgumentSupplier,
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    /// Name of or path to the executable.
    pub executable: &'a str,
    #[cfg(target_os = "macos")]
    /// macOS bundle identifier for the application.
    pub bundle_identifier: &'a str,
    #[cfg(target_os = "macos")]
    /// Location of the CLI wrapper inside the application bundle, if it exists.
    pub cli_wrapper_path: Option<&'a str>,
}

impl<'a> EditorSpec<'a> {
    #[cfg(target_os = "macos")]
    fn resolve_cli_wrapper_abspath(&self) -> anyhow::Result<PathBuf> {
        let app_dir_path = self.find_app_directory()?;
        let cli_wrapper_path = self
            .cli_wrapper_path
            .ok_or_else(|| anyhow::anyhow!("No CLI wrapper configured for {}", self.name))?;
        Ok(app_dir_path.join(cli_wrapper_path))
    }

    #[cfg(target_os = "macos")]
    fn find_app_directory(&self) -> anyhow::Result<PathBuf> {
        use objc2_app_kit::NSWorkspace;
        use objc2_foundation::NSString;

        let workspace = NSWorkspace::sharedWorkspace();
        let bundle_identifier = NSString::from_str(self.bundle_identifier);
        let app_url = workspace
            .URLForApplicationWithBundleIdentifier(&bundle_identifier)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Could not find application for '{}'",
                    self.bundle_identifier
                )
            })?;

        app_url.to_file_path().ok_or_else(|| {
            anyhow::anyhow!(
                "Could not resolve application path for '{}'",
                self.bundle_identifier
            )
        })
    }
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

#[derive(Clone)]
enum CliArgumentSupplier {
    VSCodeLike,
    Zed,
    Sublime,
    #[cfg(target_os = "macos")]
    Xcode,
}

impl CliArgumentSupplier {
    /// Add argument(s) to `cmd` to open the file on the specific line, or error if it's not
    /// supported.
    fn open_on_line<'a>(
        &self,
        cmd: &'a mut Command,
        path: &Path,
        line_nr: i32,
    ) -> anyhow::Result<&'a mut Command> {
        match self {
            Self::VSCodeLike => cmd.arg("--goto").arg(self.path_with_line_nr(path, line_nr)),
            Self::Zed => cmd.arg(self.path_with_line_nr(path, line_nr)),
            Self::Sublime => cmd.arg(self.path_with_line_nr(path, line_nr)),
            #[cfg(target_os = "macos")]
            Self::Xcode => cmd.arg("--line").arg(line_nr.to_string()).arg(path),
        };

        Ok(cmd)
    }

    fn path_with_line_nr(&self, path: &Path, line_nr: i32) -> OsString {
        let mut arg = path.as_os_str().to_owned();
        arg.push(":");
        arg.push(line_nr.to_string());
        arg
    }
}

pub(crate) const EDITORS: &[EditorSpec] = &[
    EditorSpec {
        id: "cursor",
        name: "Cursor",
        cli_arg_supplier: CliArgumentSupplier::VSCodeLike,
        #[cfg(target_os = "linux")]
        executable: "cursor",
        #[cfg(target_os = "windows")]
        executable: "Cursor.exe",
        #[cfg(target_os = "macos")]
        // This looks insane but it's actually the correct bundle ID, see https://forum.cursor.com/t/cursor-bundle-identifier/779
        bundle_identifier: "com.todesktop.230313mzl4w4u92",
        #[cfg(target_os = "macos")]
        cli_wrapper_path: Some("Contents/Resources/app/bin/cursor"),
    },
    EditorSpec {
        id: "sublime",
        name: "Sublime Text",
        cli_arg_supplier: CliArgumentSupplier::Sublime,
        #[cfg(target_os = "linux")]
        executable: "subl",
        #[cfg(target_os = "windows")]
        executable: "subl.exe",
        #[cfg(target_os = "macos")]
        bundle_identifier: "com.sublimetext.4",
        #[cfg(target_os = "macos")]
        cli_wrapper_path: Some("Contents/SharedSupport/bin/subl"),
    },
    EditorSpec {
        id: "vscode",
        name: "VS Code",
        cli_arg_supplier: CliArgumentSupplier::VSCodeLike,
        #[cfg(target_os = "linux")]
        executable: "code",
        #[cfg(target_os = "windows")]
        executable: "code.exe",
        #[cfg(target_os = "macos")]
        bundle_identifier: "com.microsoft.VSCode",
        #[cfg(target_os = "macos")]
        cli_wrapper_path: Some("Contents/Resources/app/bin/code"),
    },
    #[cfg(target_os = "macos")]
    EditorSpec {
        id: "xcode",
        name: "Xcode",
        cli_arg_supplier: CliArgumentSupplier::Xcode,
        bundle_identifier: "com.apple.dt.Xcode",
        cli_wrapper_path: Some("Contents/Developer/usr/bin/xed"),
    },
    EditorSpec {
        id: "zed",
        name: "Zed",
        cli_arg_supplier: CliArgumentSupplier::Zed,
        #[cfg(target_os = "linux")]
        executable: "zed",
        #[cfg(target_os = "windows")]
        executable: "zed.exe",
        #[cfg(target_os = "macos")]
        bundle_identifier: "dev.zed.Zed",
        #[cfg(target_os = "macos")]
        cli_wrapper_path: Some("Contents/MacOS/cli"),
    },
];

/// Low-level API to open a `path` with a specified `editor`.
///
/// # WARNING
/// It is up to the caller to assure that the `path` is safe to open and that the `editor` is safe
/// to use. Therefore, this should never be exposed to an untrusted context, such as the GUI
/// renderer.
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub fn open_in_editor_unchecked(
    editor: &EditorSpec<'_>,
    path: &Path,
    line_nr: Option<i32>,
) -> anyhow::Result<()> {
    let mut cmd = Command::new(editor.executable);

    if let Some(line_nr) = line_nr {
        editor
            .cli_arg_supplier
            .open_on_line(&mut cmd, path, line_nr)?
    } else {
        cmd.arg(path)
    };

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
pub fn open_in_editor_unchecked(
    editor: &EditorSpec<'_>,
    path: &Path,
    line_nr: Option<i32>,
) -> anyhow::Result<()> {
    if let Some(line_nr) = line_nr {
        let cli_abspath = editor.resolve_cli_wrapper_abspath()?;
        let mut cmd = Command::new(cli_abspath);
        editor
            .cli_arg_supplier
            .open_on_line(&mut cmd, path, line_nr)?;
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
        spawn_and_reap(cmd, editor.name, &path.to_string_lossy())?;
    } else {
        let mut cmd = Command::new("/usr/bin/open");
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
    }

    Ok(())
}
