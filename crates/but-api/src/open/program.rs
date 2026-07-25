use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::LazyLock,
};

use crate::open::spawn::spawn_and_reap;

use nonempty::NonEmpty;
use serde::Deserialize;

/// Name of the user-defined programs file
pub const USER_DEFINED_PROGRAMS_FILENAME: &str = "programs.json";

/// Placeholder used for filepath interpolation.
pub const FILEPATH_PLACEHOLDER: &str = "{{filepath}}";

/// Placeholder used for line number.
pub const LINE_NUMBER_PLACEHOLDER: &str = "{{line_number}}";

/// Wildcard used to match any file extension (even the empty one)
pub const FILE_EXTENSION_WILDCARD: &str = "*";

/// JSON transport types for programs.
pub mod json {
    use super::{ProgramCategory as InternalProgramCategory, ProgramSpec};
    use serde::Serialize;

    /// Program category exposed to API clients.
    #[derive(Clone, Debug, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[cfg_attr(feature = "napi", napi_derive::napi(string_enum = "camelCase"))]
    pub enum ProgramCategory {
        /// A text editor/IDE.
        Editor,
        /// A file manager such as Finder, Explorer or Thunar.
        FileManager,
        /// Anything that does not fit another category.
        Other,
    }

    impl From<&InternalProgramCategory> for ProgramCategory {
        fn from(value: &InternalProgramCategory) -> Self {
            match value {
                InternalProgramCategory::Editor => Self::Editor,
                InternalProgramCategory::FileManager => Self::FileManager,
                InternalProgramCategory::Other => Self::Other,
                #[cfg(debug_assertions)]
                InternalProgramCategory::Test => Self::Other,
            }
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

    impl From<&ProgramSpec> for Editor {
        fn from(editor: &ProgramSpec) -> Self {
            Self {
                id: editor.id.to_string(),
                name: editor.name.to_string(),
            }
        }
    }

    /// Supported program configuration for API clients.
    #[derive(Serialize, Clone)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[cfg_attr(feature = "napi", napi_derive::napi(object))]
    pub struct Program {
        /// Identifier used to refer to the program.
        pub id: String,
        /// Name of the program.
        pub name: String,
        /// Category of the program.
        pub category: ProgramCategory,
        /// File extensions associated with the program, without leading periods.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub extensions: Option<Vec<String>>,
    }

    impl From<&ProgramSpec> for Program {
        fn from(program: &ProgramSpec) -> Self {
            Self {
                id: program.id.clone(),
                name: program.name.clone(),
                category: (&program.category).into(),
                extensions: program.extensions.clone(),
            }
        }
    }
}

/// Internal program category.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum ProgramCategory {
    #[default]
    /// A text editor/IDE.
    Editor,
    /// A file manager such as Finder, Explorer or Thunar.
    FileManager,
    /// Anything that doesn't fit within other types and is not worthwhile to add a new type for.
    Other,
    #[cfg(debug_assertions)]
    /// Purely for testing, should not be included in production builds.
    Test,
}

impl From<UserDefinedProgramCategory> for ProgramCategory {
    fn from(value: UserDefinedProgramCategory) -> Self {
        match value {
            UserDefinedProgramCategory::Editor => ProgramCategory::Editor,
            UserDefinedProgramCategory::FileManager => ProgramCategory::FileManager,
            UserDefinedProgramCategory::Other => ProgramCategory::Other,
        }
    }
}

/// Supported program to open a file or directory in.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgramSpec {
    /// Identifier used to refer to the program.
    pub id: String,
    /// Name of the program.
    pub name: String,
    /// The CLI argument formatter for e.g. opening a specific line in a file.
    cli_arg_supplier: CliArgumentSupplier,
    /// The exuctable to invoke to start the program.
    pub executable: ExecutableProgram,
    /// The category of the program.
    pub category: ProgramCategory,
    /// Associated program extensions.
    pub extensions: Option<Vec<String>>,
}

impl ProgramSpec {
    /// True if this is a GUI editor.
    pub fn is_gui_editor(&self) -> bool {
        self.category == ProgramCategory::Editor && !self.requires_terminal()
    }

    /// True if this program requires control over the current terminal to execute.
    pub fn requires_terminal(&self) -> bool {
        match &self.executable {
            ExecutableProgram::PathExecutable(PathExecutable { requires_tty, .. }) => *requires_tty,
            #[cfg(target_os = "macos")]
            ExecutableProgram::MacosApplication(_) => false,
        }
    }
}

/// An executable that can be invoked by name or path.
#[derive(Clone, Debug, PartialEq)]
pub struct PathExecutable {
    /// Name of the executable on the PATH, or a qualified path to it.
    pub name_or_path: String,
    /// Whether the program requires an attached TTY or not.
    ///
    /// If this is true, it means that this program cannot be launched reliably from a GUI client,
    /// and also needs the TUI to suspend in order for the editor to take over the terminal.
    pub requires_tty: bool,
}

/// The executable to invoke for a program.
#[derive(Clone, Debug, PartialEq)]
pub enum ExecutableProgram {
    /// A program that can be executed by name or path.
    PathExecutable(PathExecutable),
    /// A macOS application installed s.t. it has a bundle identifier.
    #[cfg(target_os = "macos")]
    MacosApplication(MacosApplication),
}

impl From<UserDefinedExecutableProgram> for ExecutableProgram {
    fn from(value: UserDefinedExecutableProgram) -> Self {
        match value {
            UserDefinedExecutableProgram::PathExecutable(path_executable) => {
                Self::PathExecutable(path_executable.into())
            }
            #[cfg(target_os = "macos")]
            UserDefinedExecutableProgram::MacosApplication(macos_app) => {
                Self::MacosApplication(macos_app.into())
            }
        }
    }
}

impl From<UserDefinedPathExecutable> for PathExecutable {
    fn from(value: UserDefinedPathExecutable) -> Self {
        Self {
            name_or_path: value.name_or_path,
            requires_tty: value.requires_terminal,
        }
    }
}

#[cfg(target_os = "macos")]
impl From<UserDefinedMacosApplication> for MacosApplication {
    fn from(value: UserDefinedMacosApplication) -> Self {
        Self {
            bundle_identifier: value.bundle_id,
            cli_wrapper_path: value.cli_wrapper_path,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum CliArgumentSupplier {
    VSCodeLike,
    Zed,
    Neovim,
    Sublime,
    Custom(CustomCliArgumentSupplier),
    #[cfg(all(target_os = "macos", not(debug_assertions)))]
    #[allow(dead_code)]
    Xcode,
    /// For programs that don't support any special "open at" semantics
    #[allow(dead_code)]
    Default,
}

impl CliArgumentSupplier {
    /// Add argument(s) to `cmd` to open the file on the specific line.
    fn open_at_line<'a>(&self, cmd: &'a mut Command, path: &Path, line_nr: u32) -> &'a mut Command {
        match self {
            Self::VSCodeLike => cmd.arg("--goto").arg(self.path_with_line_nr(path, line_nr)),
            Self::Zed => cmd.arg(self.path_with_line_nr(path, line_nr)),
            Self::Sublime => cmd.arg(self.path_with_line_nr(path, line_nr)),
            #[cfg(all(target_os = "macos", not(debug_assertions)))]
            Self::Xcode => cmd.arg("--line").arg(line_nr.to_string()).arg(path),
            Self::Neovim => cmd.arg(format!("+{line_nr}")).arg(path),
            Self::Custom(custom) => custom.open_at_line(cmd, path, line_nr),
            Self::Default => cmd.arg(path),
        };

        cmd
    }

    /// Add argument(s) to `cmd` to open the file.
    fn open<'a>(&self, cmd: &'a mut Command, path: &Path) -> &'a mut Command {
        match self {
            Self::Custom(custom) => custom.open(cmd, path),
            _ => cmd.arg(path),
        };
        cmd
    }

    /// Add argument(s) to `cmd` to open all files.
    fn open_many<'a, P: AsRef<Path>>(
        &self,
        cmd: &'a mut Command,
        paths: &NonEmpty<P>,
    ) -> &'a mut Command {
        match self {
            Self::Custom(custom) => custom.open_many(cmd, paths),
            _ => {
                for path in paths {
                    cmd.arg(path.as_ref());
                }
                cmd
            }
        }
    }

    fn path_with_line_nr(&self, path: &Path, line_nr: u32) -> OsString {
        let mut arg = path.as_os_str().to_owned();
        arg.push(":");
        arg.push(line_nr.to_string());
        arg
    }
}

#[derive(Clone, Debug, PartialEq)]
struct CustomCliArgumentSupplier {
    /// Arguments to pass to the executable when invoked to open a file.
    ///
    /// Recognized placeholders:
    ///
    /// * [`FILEPATH_PLACEHOLDER`] is substituted for the filepath
    ///
    /// TODO should not assume utf8 for args
    open_args: Option<Vec<String>>,
    /// Arguments to pass to the executable when invoked to open at a specific line.
    ///
    /// Recognized placeholders:
    ///
    /// * [`FILEPATH_PLACEHOLDER`] is substituted for the filepath
    /// * [`LINE_NUMBER_PLACEHOLDER`] is substituted for the line number
    ///
    /// TODO should not assume utf8 for args
    open_at_line_args: Option<Vec<String>>,
}

impl CustomCliArgumentSupplier {
    fn open_at_line<'a>(&self, cmd: &'a mut Command, path: &Path, line_nr: u32) -> &'a mut Command {
        let Some(open_at_line_args) = &self.open_at_line_args else {
            return self.open(cmd, path);
        };

        for arg in open_at_line_args {
            // TODO should not assume utf8 for path
            cmd.arg(
                arg.replace(FILEPATH_PLACEHOLDER, &path.to_string_lossy())
                    .replace(LINE_NUMBER_PLACEHOLDER, &line_nr.to_string()),
            );
        }
        cmd
    }

    fn open<'a>(&self, cmd: &'a mut Command, path: &Path) -> &'a mut Command {
        let Some(open_args) = &self.open_args else {
            cmd.arg(path);
            return cmd;
        };

        for arg in open_args {
            // TODO should not assume utf8 for path
            cmd.arg(arg.replace(FILEPATH_PLACEHOLDER, &path.to_string_lossy()));
        }
        cmd
    }

    fn open_many<'a, P: AsRef<Path>>(
        &self,
        cmd: &'a mut Command,
        paths: &NonEmpty<P>,
    ) -> &'a mut Command {
        let Some(open_args) = &self.open_args else {
            return CliArgumentSupplier::Default.open_many(cmd, paths);
        };

        for arg in open_args {
            if arg.contains(FILEPATH_PLACEHOLDER) {
                for path in paths {
                    cmd.arg(arg.replace(FILEPATH_PLACEHOLDER, &path.as_ref().to_string_lossy()));
                }
            } else {
                cmd.arg(arg);
            }
        }

        cmd
    }
}

/// The built-in supported programs.
///
/// IMPORTANT: Platform-specific programs are not allowed in tests, as it makes any tests that
/// snapshot the list platform-dependent. Ensure that `not(debug_assertions)` is specified for any
/// program that is in any way dependent on platform.
pub(crate) static PROGRAMS: LazyLock<Vec<ProgramSpec>> = LazyLock::new(|| {
    Vec::from([
        ProgramSpec {
            id: "nvim".into(),
            name: "Neovim".into(),
            cli_arg_supplier: CliArgumentSupplier::Neovim,
            executable: ExecutableProgram::PathExecutable(PathExecutable {
                name_or_path: "nvim".into(),
                requires_tty: true,
            }),
            category: ProgramCategory::Editor,
            extensions: None,
        },
        ProgramSpec {
            id: "cursor".into(),
            name: "Cursor".into(),
            cli_arg_supplier: CliArgumentSupplier::VSCodeLike,
            #[cfg(not(target_os = "macos"))]
            executable: ExecutableProgram::PathExecutable(PathExecutable {
                #[cfg(target_os = "linux")]
                name_or_path: "cursor".into(),
                #[cfg(target_os = "windows")]
                name_or_path: "Cursor.exe".into(),
                requires_tty: false,
            }),
            #[cfg(target_os = "macos")]
            executable: ExecutableProgram::MacosApplication(MacosApplication {
                // This looks insane but it's actually the correct bundle ID, see https://forum.cursor.com/t/cursor-bundle-identifier/779
                bundle_identifier: "com.todesktop.230313mzl4w4u92".into(),
                cli_wrapper_path: Some("Contents/Resources/app/bin/cursor".into()),
            }),
            category: ProgramCategory::Editor,
            extensions: None,
        },
        ProgramSpec {
            id: "sublime".into(),
            name: "Sublime Text".into(),
            cli_arg_supplier: CliArgumentSupplier::Sublime,
            #[cfg(not(target_os = "macos"))]
            executable: ExecutableProgram::PathExecutable(PathExecutable {
                #[cfg(target_os = "linux")]
                name_or_path: "subl".into(),
                #[cfg(target_os = "windows")]
                name_or_path: "subl.exe".into(),
                requires_tty: false,
            }),
            #[cfg(target_os = "macos")]
            executable: ExecutableProgram::MacosApplication(MacosApplication {
                bundle_identifier: "com.sublimetext.4".into(),
                cli_wrapper_path: Some("Contents/SharedSupport/bin/subl".into()),
            }),
            category: ProgramCategory::Editor,
            extensions: None,
        },
        ProgramSpec {
            id: "vscode".into(),
            name: "VS Code".into(),
            cli_arg_supplier: CliArgumentSupplier::VSCodeLike,
            #[cfg(not(target_os = "macos"))]
            executable: ExecutableProgram::PathExecutable(PathExecutable {
                #[cfg(target_os = "linux")]
                name_or_path: "code".into(),
                #[cfg(target_os = "windows")]
                name_or_path: "code.exe".into(),
                requires_tty: false,
            }),
            #[cfg(target_os = "macos")]
            executable: ExecutableProgram::MacosApplication(MacosApplication {
                bundle_identifier: "com.microsoft.VSCode".into(),
                cli_wrapper_path: Some("Contents/Resources/app/bin/code".into()),
            }),
            category: ProgramCategory::Editor,
            extensions: None,
        },
        #[cfg(all(target_os = "macos", not(debug_assertions)))]
        ProgramSpec {
            id: "xcode".into(),
            name: "Xcode".into(),
            cli_arg_supplier: CliArgumentSupplier::Xcode,
            executable: ExecutableProgram::MacosApplication(MacosApplication {
                bundle_identifier: "com.apple.dt.Xcode".into(),
                cli_wrapper_path: Some("Contents/Developer/usr/bin/xed".into()),
            }),
            category: ProgramCategory::Editor,
            extensions: None,
        },
        ProgramSpec {
            id: "zed".into(),
            name: "Zed".into(),
            cli_arg_supplier: CliArgumentSupplier::Zed,
            #[cfg(not(target_os = "macos"))]
            executable: ExecutableProgram::PathExecutable(PathExecutable {
                #[cfg(target_os = "linux")]
                name_or_path: "zed".into(),
                #[cfg(target_os = "windows")]
                name_or_path: "zed.exe".into(),
                requires_tty: false,
            }),
            #[cfg(target_os = "macos")]
            executable: ExecutableProgram::MacosApplication(MacosApplication {
                bundle_identifier: "dev.zed.Zed".into(),
                cli_wrapper_path: Some("Contents/MacOS/cli".into()),
            }),
            category: ProgramCategory::Editor,
            extensions: None,
        },
        #[cfg(debug_assertions)]
        ProgramSpec {
            id: "echo".into(),
            name: "echo".into(),
            cli_arg_supplier: CliArgumentSupplier::Custom(CustomCliArgumentSupplier {
                open_at_line_args: Some(vec![
                    "filepath='{{filepath}}'".into(),
                    "line_number='{{line_number}}'".into(),
                ]),
                open_args: Some(vec!["filepath='{{filepath}}'".into()]),
            }),
            executable: ExecutableProgram::PathExecutable(PathExecutable {
                name_or_path: "echo".into(),
                requires_tty: true,
            }),
            category: ProgramCategory::Test,
            extensions: None,
        },
        #[cfg(debug_assertions)]
        ProgramSpec {
            id: "touch".into(),
            name: "touch".into(),
            cli_arg_supplier: CliArgumentSupplier::Custom(CustomCliArgumentSupplier {
                open_args: Some(vec!["{{filepath}}.touch".into()]),
                open_at_line_args: Some(vec!["{{filepath}}.touch.{{line_number}}".into()]),
            }),
            executable: ExecutableProgram::PathExecutable(PathExecutable {
                name_or_path: "touch".into(),
                requires_tty: true,
            }),
            category: ProgramCategory::Test,
            extensions: None,
        },
        #[cfg(all(target_os = "linux", not(debug_assertions)))]
        ProgramSpec {
            id: "thunar".into(),
            name: "Thunar".into(),
            cli_arg_supplier: CliArgumentSupplier::Default,
            executable: ExecutableProgram::PathExecutable(PathExecutable {
                name_or_path: "thunar".into(),
                requires_tty: false,
            }),
            category: ProgramCategory::FileManager,
            extensions: None,
        },
    ])
});

/// Specification to open.
pub enum OpenSpec {
    /// A single file.
    File(PathBuf),
    /// Multiple files.
    Files(NonEmpty<PathBuf>),
    /// A single file at a specific line.
    FileAtLine(PathBuf, u32),
}

/// Low-level API to open a `path` with a specified `program`.
///
/// # WARNING
/// It is up to the caller to assure that the `path` is safe to open and that the `program` is safe
/// to use. Therefore, this should never be exposed to an untrusted context, such as the GUI
/// renderer.
pub fn open_in_program_unchecked(program: &ProgramSpec, open_spec: OpenSpec) -> anyhow::Result<()> {
    match &program.executable {
        ExecutableProgram::PathExecutable(path_executable) => {
            open_in_path_executable(path_executable, &program.cli_arg_supplier, open_spec)
        }
        #[cfg(target_os = "macos")]
        ExecutableProgram::MacosApplication(macos_application) => {
            open_in_macos_application(macos_application, &program.cli_arg_supplier, open_spec)
        }
    }
}

fn open_in_path_executable(
    path_executable: &PathExecutable,
    cli_arg_supplier: &CliArgumentSupplier,
    open_spec: OpenSpec,
) -> anyhow::Result<()> {
    let mut cmd = Command::new(&path_executable.name_or_path);

    match open_spec {
        OpenSpec::File(path) => cli_arg_supplier.open(&mut cmd, &path),
        OpenSpec::Files(paths) => cli_arg_supplier.open_many(&mut cmd, &paths),
        OpenSpec::FileAtLine(path, line_nr) => {
            cli_arg_supplier.open_at_line(&mut cmd, &path, line_nr)
        }
    };

    if path_executable.requires_tty {
        cmd.stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .stdin(Stdio::inherit())
            .status()?;
    } else {
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
        spawn_and_reap(
            cmd,
            &path_executable.name_or_path,
            &path_executable.name_or_path,
        )?;
    }

    Ok(())
}

/// A canonically installed macOS application with a bundle ID and an optional CLI wrapper.
#[cfg(target_os = "macos")]
#[derive(Clone, Debug, PartialEq)]
pub struct MacosApplication {
    /// macOS bundle identifier for the application.
    pub bundle_identifier: String,
    /// Location of the CLI wrapper inside the application bundle, if it exists.
    pub cli_wrapper_path: Option<String>,
}

#[cfg(target_os = "macos")]
impl MacosApplication {
    #[cfg(target_os = "macos")]
    fn resolve_cli_wrapper_abspath(&self) -> anyhow::Result<PathBuf> {
        let app_dir_path = self.find_app_directory()?;
        let cli_wrapper_path = self.cli_wrapper_path.as_deref().ok_or_else(|| {
            anyhow::anyhow!("No CLI wrapper configured for {}", self.bundle_identifier)
        })?;
        Ok(app_dir_path.join(cli_wrapper_path))
    }

    #[cfg(target_os = "macos")]
    fn find_app_directory(&self) -> anyhow::Result<PathBuf> {
        use objc2_app_kit::NSWorkspace;
        use objc2_foundation::NSString;

        let workspace = NSWorkspace::sharedWorkspace();
        let bundle_identifier = NSString::from_str(&self.bundle_identifier);
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

#[cfg(target_os = "macos")]
fn open_in_macos_application(
    app: &MacosApplication,
    cli_arg_supplier: &CliArgumentSupplier,
    open_spec: OpenSpec,
) -> anyhow::Result<()> {
    match open_spec {
        OpenSpec::FileAtLine(path, line_nr) => match app.resolve_cli_wrapper_abspath() {
            Ok(cli_abspath) => {
                let mut cmd = Command::new(cli_abspath);
                cli_arg_supplier.open_at_line(&mut cmd, &path, line_nr);
                cmd.stdout(Stdio::null()).stderr(Stdio::null());
                spawn_and_reap(cmd, &app.bundle_identifier, &path.to_string_lossy())
            }
            Err(_) => open_macos_application_via_open(app, &NonEmpty::new(path)),
        },
        OpenSpec::File(path) => open_macos_application_via_open(app, &NonEmpty::new(path)),
        OpenSpec::Files(paths) => open_macos_application_via_open(app, &paths),
    }
}

#[cfg(target_os = "macos")]
fn open_macos_application_via_open(
    app: &MacosApplication,
    paths: &NonEmpty<PathBuf>,
) -> anyhow::Result<()> {
    let mut cmd = Command::new("/usr/bin/open");
    cmd.arg("-b").arg(&app.bundle_identifier);

    for path in paths {
        cmd.arg(path);
    }

    let status = cmd.stdout(Stdio::null()).stderr(Stdio::null()).status()?;

    if !status.success() {
        anyhow::bail!(
            "failed to open {paths:?} with app bundle identifier '{}'",
            app.bundle_identifier
        );
    }

    Ok(())
}

/// A serializable form of [`ProgramSpec`] for user defined programs.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDefinedProgramSpec {
    /// Identifier used to refer to the program.
    ///
    /// If left empty, the ID is derived from [`Self::name`] instead.
    pub id: Option<String>,
    /// The display name of the program.
    pub name: Option<String>,
    /// The exuctable to invoke to start the program.
    pub executable: Option<UserDefinedExecutableProgram>,
    /// The kind of the program.
    pub category: Option<UserDefinedProgramCategory>,
    /// Arguments to pass to the executable when invoked to open a file.
    ///
    /// Recognized placeholders:
    ///
    /// * [`FILEPATH_PLACEHOLDER`] is substituted for the filepath
    pub open_args: Option<Vec<String>>,
    /// Arguments to pass to the executable when invoked to open at a specific line.
    ///
    /// Recognized placeholders:
    ///
    /// * [`FILEPATH_PLACEHOLDER`] is substituted for the filepath
    /// * [`LINE_NUMBER_PLACEHOLDER`] is substituted for the line number
    pub open_at_line_args: Option<Vec<String>>,
    /// File extensions to associate with this program.
    pub extensions: Option<Vec<String>>,
}

impl UserDefinedProgramSpec {
    /// Try to transform this specification into a valid [`ProgramSpec`].
    pub fn try_into_program_spec(self) -> anyhow::Result<ProgramSpec> {
        let extensions = self.extensions.map(|extensions| {
            extensions
                .into_iter()
                .map(|ext| ext.strip_prefix('.').unwrap_or(&ext).to_owned())
                .collect()
        });

        if let Some(id) = &self.id
            && let Some(builtin) = PROGRAMS.iter().find(|p| &p.id == id)
        {
            // This is an override for a builtin - merge!
            let cli_arg_supplier = if self.open_args.is_some() || self.open_at_line_args.is_some() {
                CliArgumentSupplier::Custom(CustomCliArgumentSupplier {
                    open_args: self.open_args,
                    open_at_line_args: self.open_at_line_args,
                })
            } else {
                builtin.cli_arg_supplier.clone()
            };

            Ok(ProgramSpec {
                id: id.clone(),
                name: self.name.unwrap_or_else(|| builtin.name.clone()),
                executable: self
                    .executable
                    .map(Into::into)
                    .unwrap_or_else(|| builtin.executable.clone()),
                category: self
                    .category
                    .map(Into::into)
                    .unwrap_or_else(|| builtin.category.clone()),
                cli_arg_supplier,
                extensions,
            })
        } else {
            let (name, id) = match (self.name, self.id) {
                (Some(name), Some(id)) => (name, id),
                (Some(name), None) => (name.clone(), name),
                (None, Some(id)) => (id.clone(), id),
                (None, None) => anyhow::bail!("id or name must be specified"),
            };

            let Some(executable) = self.executable else {
                anyhow::bail!("executable must be specified for non built-ins")
            };

            Ok(ProgramSpec {
                id,
                name,
                executable: executable.into(),
                category: self.category.map(Into::into).unwrap_or_default(),
                cli_arg_supplier: CliArgumentSupplier::Custom(CustomCliArgumentSupplier {
                    open_args: self.open_args,
                    open_at_line_args: self.open_at_line_args,
                }),
                extensions,
            })
        }
    }
}

/// The executable to invoke for a program.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum UserDefinedExecutableProgram {
    /// A program that can be executed by name or path.
    PathExecutable(UserDefinedPathExecutable),
    /// A macOS application installed s.t. it has a bundle identifier.
    #[cfg(target_os = "macos")]
    MacosApplication(UserDefinedMacosApplication),
}

/// A user defined executable.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDefinedPathExecutable {
    /// Name of the executable on the PATH, or a qualified path to it.
    pub name_or_path: String,
    /// Whether the program requires an attached terminal or not.
    ///
    /// If this is true, it means that this program cannot be launched reliably from a GUI client,
    /// and also needs the TUI to suspend in order for the editor to take over the terminal.
    pub requires_terminal: bool,
}

/// Program category.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UserDefinedProgramCategory {
    /// A text editor/IDE.
    Editor,
    /// A file manager.
    FileManager,
    /// Anything else.
    Other,
}

/// A canonically installed macOS application with a bundle ID and an optional CLI wrapper.
#[cfg(target_os = "macos")]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDefinedMacosApplication {
    /// macOS bundle identifier for the application.
    pub bundle_id: String,
    /// Location of the CLI wrapper inside the application bundle, if it exists.
    pub cli_wrapper_path: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn user_defined_macos_application_deserializes_into_program_spec() {
        let spec: UserDefinedProgramSpec = serde_json::from_str(
            r#"{
                "name": "Visual Studio Code",
                "executable": {
                    "type": "macosApplication",
                    "bundleId": "com.microsoft.VSCode",
                    "cliWrapperPath": "Contents/Resources/app/bin/code"
                },
                "category": "editor"
            }"#,
        )
        .expect("macOS application should deserialize");

        let spec: ProgramSpec = spec.try_into_program_spec().unwrap();
        assert_eq!(
            spec,
            ProgramSpec {
                id: "Visual Studio Code".into(),
                name: "Visual Studio Code".into(),
                cli_arg_supplier: CliArgumentSupplier::Custom(CustomCliArgumentSupplier {
                    open_args: None,
                    open_at_line_args: None,
                }),
                executable: ExecutableProgram::MacosApplication(MacosApplication {
                    bundle_identifier: "com.microsoft.VSCode".into(),
                    cli_wrapper_path: Some("Contents/Resources/app/bin/code".into()),
                }),
                category: ProgramCategory::Editor,
                extensions: None,
            },
            "JSON should convert to expected internal program specification"
        );
    }

    #[test]
    fn user_defined_override_for_builtin_executable_merges_with_builtin() {
        let vscode_override_spec: UserDefinedProgramSpec = serde_json::from_str(
            r#"{
                "id": "vscode",
                "executable": {
                    "type": "pathExecutable",
                    "nameOrPath": "/overridden/path",
                    "requiresTerminal": false
                }
            }"#,
        )
        .expect("must deserialize");

        let builtin_vscode = PROGRAMS.iter().find(|p| p.id == "vscode").unwrap();

        let spec: ProgramSpec = vscode_override_spec.try_into_program_spec().unwrap();
        assert_eq!(
            spec,
            ProgramSpec {
                executable: ExecutableProgram::PathExecutable(PathExecutable {
                    name_or_path: "/overridden/path".into(),
                    requires_tty: false
                }),
                ..builtin_vscode.clone()
            }
        )
    }

    #[test]
    fn user_defined_override_for_builtin_category_merges_with_builtin() {
        let vscode_override_spec: UserDefinedProgramSpec = serde_json::from_str(
            r#"{
                "id": "vscode",
                "category": "fileManager"
            }"#,
        )
        .expect("must deserialize");

        let builtin_vscode = PROGRAMS.iter().find(|p| p.id == "vscode").unwrap();

        let spec: ProgramSpec = vscode_override_spec.try_into_program_spec().unwrap();
        assert_eq!(
            spec,
            ProgramSpec {
                category: ProgramCategory::FileManager,
                ..builtin_vscode.clone()
            }
        )
    }

    #[test]
    fn user_defined_override_for_builtin_extensions_merges_with_builtin() {
        let vscode_override_spec: UserDefinedProgramSpec = serde_json::from_str(
            r#"{
                "id": "vscode",
                "extensions": ["txt"]
            }"#,
        )
        .expect("must deserialize");

        let builtin_vscode = PROGRAMS.iter().find(|p| p.id == "vscode").unwrap();

        let spec: ProgramSpec = vscode_override_spec.try_into_program_spec().unwrap();
        assert_eq!(
            spec,
            ProgramSpec {
                extensions: Some(vec!["txt".into()]),
                ..builtin_vscode.clone()
            }
        )
    }

    #[test]
    fn user_defined_override_for_builtin_extensions_strips_periods_from_extensions() {
        let vscode_override_spec: UserDefinedProgramSpec = serde_json::from_str(
            r#"{
                "id": "vscode",
                "extensions": [".txt"]
            }"#,
        )
        .expect("must deserialize");

        let builtin_vscode = PROGRAMS.iter().find(|p| p.id == "vscode").unwrap();

        let spec: ProgramSpec = vscode_override_spec.try_into_program_spec().unwrap();
        assert_eq!(
            spec,
            ProgramSpec {
                extensions: Some(vec!["txt".into()]),
                ..builtin_vscode.clone()
            }
        )
    }
}
