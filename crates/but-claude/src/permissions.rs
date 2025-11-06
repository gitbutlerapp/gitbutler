#![allow(unused)]

use anyhow::Result;
use gix::{
    bstr::BStr,
    glob::wildmatch::{self, Mode},
};

#[derive(Debug, Clone)]
enum Permission {
    Bash(BashPattern),
    Read(PathPattern),
    Edit(PathPattern),
    WebFetch(UrlPattern),
    Mcp(McpPattern),
    Other(FullMatchPattern),
}

#[derive(Debug, Clone)]
struct BashPattern {
    base: String,
    /// If this is true, the match must be exact, otherwise a partial match is permitted
    exact: bool,
}

impl BashPattern {
    fn matches(&self, command: &str) -> bool {
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
#[derive(Debug, Clone)]
struct PathPattern {
    /// Absolute Glob string
    ///
    /// We are safe to use a string here since CC only works with utf-8
    /// compatible paths.
    pattern: String,
    /// Determines how the pattern should be serialized.
    ///
    /// CTO: In my mind, I've gone back and forth on whether this should be
    /// here, but I _do_ think that `orig == serialize(deserialize(orig))` is an
    /// important property to have.
    kind: PathPatternKind,
}

/// What kind of pattern to serialize as.
#[derive(Debug, Clone)]
enum PathPatternKind {
    HomeRelative,
    Absolute,
    SettingsRelative,
    CwdRelative,
}

impl PathPattern {
    /// Takes an absolute path as a string
    fn matches<'a>(&self, path: impl Into<&'a BStr>) -> bool {
        gix::glob::wildmatch(
            self.pattern.as_str().into(),
            path.into(),
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
#[derive(Debug, Clone)]
enum UrlPattern {
    FullMatch(String),
    Domain(String),
}

impl UrlPattern {
    fn matches(&self, url: &str) -> Result<bool> {
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
#[derive(Debug, Clone)]
struct McpPattern {
    pattern: String,
}

impl McpPattern {
    fn matches(&self, name: &str) -> bool {
        name.starts_with(&self.pattern)
    }
}

#[derive(Debug, Clone)]
struct FullMatchPattern {
    pattern: String,
}

impl FullMatchPattern {
    fn matches(&self, target: &str) -> bool {
        self.pattern == target
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

        #[test]
        fn matches_glob() {
            let pattern = PathPattern {
                pattern: "/foo/**".into(),
                // This is only relevant for serialization
                kind: PathPatternKind::HomeRelative,
            };

            assert!(pattern.matches("/foo/qux/"));
            assert!(pattern.matches("/foo/qux"));
            assert!(pattern.matches("/foO/qUx/baz"));
            assert!(!pattern.matches("/fooo/qux/baz"));
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
