use anyhow::{Context, Result};
use gix::glob::wildmatch::Mode;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Context for serializing permission patterns
#[derive(Debug, Clone)]
pub struct SerializationContext {
    pub home_path: PathBuf,
    pub project_path: PathBuf,
    pub global_claude_dir: PathBuf,
    pub for_global: bool,
}

impl SerializationContext {
    pub fn new(
        home_path: impl Into<PathBuf>,
        project_path: impl Into<PathBuf>,
        global_claude_dir: impl Into<PathBuf>,
        for_global: bool,
    ) -> Self {
        Self {
            home_path: home_path.into(),
            project_path: project_path.into(),
            global_claude_dir: global_claude_dir.into(),
            for_global,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashPattern {
    base: String,
    /// If this is true, the match must be exact, otherwise a partial match is permitted
    exact: bool,
}

impl BashPattern {
    pub fn new_exact(command: String) -> Self {
        Self {
            base: command,
            exact: true,
        }
    }

    pub fn serialize(&self) -> String {
        if self.exact {
            self.base.to_owned()
        } else {
            format!("{}:*", self.base)
        }
    }

    pub fn matches(&self, command: &str) -> bool {
        if self.exact {
            self.base == command
        } else {
            command.starts_with(&self.base)
        }
    }
}

/// Represents a CC path pattern
///
/// There are the following kinds CC supports
/// - Relative to home
///     - IE: Edit(~/Sherman/CURRENT_ENV)
/// - Absolute
///     - IE: Edit(/Users/calebowens/Sherman/CURRENT_ENV)
/// - Relative to settings _or_ project
///     - IE in `$HOME/.claude/settings.json: Edit(/../Sherman/CURRENT_ENV)
///     - IE in `$PROJECT/.claude/settings.json: Edit(/CURRENT_ENV)
/// - Relative to CWD
///     - Assuming CWD == $PROJECT: Edit(CURRENT_ENV)
///
/// Claude always gives us an absolute path so we should match against that.
///
/// Our representation of a path pattern will be an absolute glob.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathPattern {
    /// Absolute glob path
    ///
    /// We use PathBuf here for type safety. CC only works with utf-8
    /// compatible paths, so we'll convert to strings when needed for matching.
    pattern: PathBuf,
    /// Determines how the pattern should be serialized.
    ///
    /// CTO: In my mind, I've gone back and forth on whether this should be
    /// here, but I _do_ think that `orig == serialize(deserialize(orig))` is an
    /// important property to have.
    kind: PathPatternKind,
}

impl PathPattern {
    pub fn new(path: impl Into<PathBuf>, kind: PathPatternKind) -> Self {
        Self {
            pattern: path.into(),
            kind,
        }
    }

    pub fn serialize(&self, ctx: &SerializationContext) -> Result<String> {
        match self.kind {
            PathPatternKind::Absolute => Ok(format!(
                "/{}",
                self.pattern
                    .to_str()
                    .context("Path contains invalid UTF-8")?
            )),
            PathPatternKind::HomeRelative => {
                let stripped = self
                    .pattern
                    .strip_prefix(&ctx.home_path)
                    .context("Can't strip home")?;
                Ok(format!(
                    "~/{}",
                    stripped.to_str().context("Path contains invalid UTF-8")?
                ))
            }
            PathPatternKind::SettingsRelative => {
                let stripped = if ctx.for_global {
                    self.pattern
                        .strip_prefix(&ctx.global_claude_dir)
                        .context("Can't strip global claude dir")?
                } else {
                    self.pattern
                        .strip_prefix(&ctx.project_path)
                        .context("Can't strip project path")?
                };
                Ok(format!(
                    "/{}",
                    stripped.to_str().context("Path contains invalid UTF-8")?
                ))
            }
            PathPatternKind::CwdRelative => {
                let stripped = self
                    .pattern
                    .strip_prefix(&ctx.home_path)
                    .context("Can't strip home")?;
                Ok(stripped
                    .to_str()
                    .context("Path contains invalid UTF-8")?
                    .to_owned())
            }
        }
    }
}

/// What kind of pattern to serialize as.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PathPatternKind {
    HomeRelative,
    Absolute,
    SettingsRelative,
    CwdRelative,
}

impl PathPattern {
    /// Takes an absolute path
    pub fn matches(&self, path: &Path) -> bool {
        // Convert both to strings for glob matching
        let pattern_str = match self.pattern.to_str() {
            Some(s) => s,
            None => return false, // Invalid UTF-8 in pattern, can't match
        };
        let path_str = match path.to_str() {
            Some(s) => s,
            None => return false, // Invalid UTF-8 in path, can't match
        };

        gix::glob::wildmatch(
            pattern_str.into(),
            path_str.into(),
            Mode::NO_MATCH_SLASH_LITERAL | Mode::IGNORE_CASE,
        )
    }
}

/// CC specifies in their documentation two types of permission for "WebFetch".
/// This is either a full match, or just the "domain". It's not entirly clear
/// what constitutes the "domain", so I've let a 3rd party library decide.
///
/// In practice, I'm not entirly sure if WebFetch actually requires
/// permissions...
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UrlPattern {
    FullMatch(String),
    Domain(String),
}

impl UrlPattern {
    pub fn full_match(url: String) -> Self {
        Self::FullMatch(url)
    }

    pub fn serialize(&self) -> String {
        match self {
            Self::FullMatch(a) => a.to_owned(),
            Self::Domain(a) => format!("domain:{a}"),
        }
    }

    pub fn matches(&self, url: &str) -> Result<bool> {
        match self {
            Self::FullMatch(pattern) => Ok(url == pattern),
            Self::Domain(pattern) => {
                let parsed = url::Url::parse(url)?;
                Ok(parsed.domain() == Some(pattern))
            }
        }
    }
}

/// Matching for an MCP server or server tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPattern {
    pattern: String,
}

impl McpPattern {
    pub fn new(pattern: String) -> Self {
        Self { pattern }
    }

    pub fn serialize(&self) -> String {
        self.pattern.to_owned()
    }

    pub fn matches(&self, name: &str) -> bool {
        name.starts_with(&self.pattern)
    }
}

#[cfg(test)]
mod test {
    mod bash {
        use crate::permissions::BashPattern;

        #[test]
        fn full_matches() {
            let full_pattern = BashPattern {
                base: "rm -rf /".into(),
                exact: true,
            };

            assert!(full_pattern.matches("rm -rf /"));
            assert!(!full_pattern.matches("rm -rf /usr"));
            assert!(!full_pattern.matches("rm -r /"));
            assert!(!full_pattern.matches("rm foo"));
        }

        #[test]
        fn partial_matches() {
            let full_pattern = BashPattern {
                base: "rm -rf /".into(),
                exact: false,
            };

            assert!(full_pattern.matches("rm -rf /"));
            assert!(full_pattern.matches("rm -rf /usr"));
            assert!(!full_pattern.matches("rm -r /"));
            assert!(!full_pattern.matches("other -rf /"));
        }
    }

    mod path {
        use crate::permissions::{PathPattern, PathPatternKind};
        use std::path::Path;

        #[test]
        fn matches_glob() {
            let pattern = PathPattern {
                pattern: "/foo/**".into(),
                // This is only relevant for serialization
                kind: PathPatternKind::HomeRelative,
            };

            assert!(pattern.matches(Path::new("/foo/qux/")));
            assert!(pattern.matches(Path::new("/foo/qux")));
            assert!(pattern.matches(Path::new("/foO/qUx/baz")));
            assert!(!pattern.matches(Path::new("/fooo/qux/baz")));
        }
    }

    mod url_pattern {
        use crate::permissions::UrlPattern;
        use anyhow::Result;

        #[test]
        fn full_matches() -> Result<()> {
            let pattern = UrlPattern::FullMatch("https://asdfasdf.example.com?foo=2".into());

            assert!(pattern.matches("https://asdfasdf.example.com?foo=2")?);
            assert!(!pattern.matches("https://asdfasdf.example.com?foo=3")?);

            Ok(())
        }

        #[test]
        fn domain_matches() -> Result<()> {
            let pattern = UrlPattern::Domain("asdfasdf.example.com".into());

            assert!(pattern.matches("https://asdfasdf.example.com?foo=2")?);
            assert!(pattern.matches("https://asdfasdf.example.com?foo=3")?);
            assert!(!pattern.matches("https://qux.example.com?foo=3")?);

            Ok(())
        }
    }
}
