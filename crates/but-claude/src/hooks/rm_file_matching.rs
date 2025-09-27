use std::path::Path;

/// Matches a path that _could_ have been affected by the RM.
pub struct RmMatcher {
    cwd: String,
    patterns: Vec<String>,
}

impl RmMatcher {
    /// Takes an RM command like:
    /// - `rm -r foo/bar /tmp/asdf/**/bar/*`
    /// - `rm "/foo/bar baz"
    fn create(command: &str, cwd: &str) -> anyhow::Result<RmMatcher> {
        let paths = command
            .split(" ")
            .skip(1)
            .filter(|arg| !arg.starts_with("-"));

        let recursive = command
            .split(" ")
            .find(|arg| arg.starts_with("-") && arg.contains("r"));

        let patterns = paths.map(|path| {
            let path = Path::new(path);
            if path.is_absolute() {
                path.to_string_lossy().to_string()
            } else {
                Path::new(cwd).join(path).to_string_lossy().to_string()
            }
        });

        todo!()
    }
}
