use std::path::{Component, Path, PathBuf};

/// Normalize a path to remove any `.` and `..` components
/// and standardize the path separator to the system's default.
///
/// This trait is automatically implemented for anything convertible
/// to a `&Path` (via `AsRef<Path>`).
pub trait Normalize {
    /// Normalize a path to remove any `.` and `..` components
    /// and standardize the path separator to the system's default.
    fn normalize(&self) -> PathBuf;
}

impl<P: AsRef<Path>> Normalize for P {
    fn normalize(&self) -> PathBuf {
        // Note: Copied from Cargo's codebase:
        //       https://github.com/rust-lang/cargo/blob/2e4cfc2b7d43328b207879228a2ca7d427d188bb/src/cargo/util/paths.rs#L65-L90
        //       License: MIT OR Apache-2.0 (this function only)
        //
        //       Small modifications made by GitButler.

        let path = self.as_ref();
        let mut components = path.components().peekable();
        let mut ret = if let Some(c @ Component::Prefix(..)) = components.peek().copied() {
            components.next();
            PathBuf::from(c.as_os_str())
        } else {
            PathBuf::new()
        };

        for component in components {
            match component {
                Component::Prefix(..) => unreachable!(),
                Component::RootDir => {
                    ret.push(component.as_os_str());
                }
                Component::CurDir => {}
                Component::ParentDir => {
                    ret.pop();
                }
                Component::Normal(c) => {
                    ret.push(c);
                }
            }
        }
        ret
    }
}
