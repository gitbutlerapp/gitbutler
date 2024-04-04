use core::fmt;

/// An error that can occur while parsing a refspec from a string.
#[derive(Debug, PartialEq, Eq, Clone, thiserror::Error)]
pub enum Error {
    /// Encountered an unexpected character when parsing a [`RefSpec`] from a string.
    #[error("unexpected character {0:?} (offset {1})")]
    UnexpectedChar(char, usize),
}

/// A Git [refspec](https://git-scm.com/book/en/v2/Git-Internals-The-Refspec).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct RefSpec {
    /// If `true`, will update the ref upon a fetch or push even if it is not a fast-forward.
    pub update_non_fastforward: bool,
    /// The source refspec.
    pub source: Option<String>,
    /// The destination refspec.
    pub destination: Option<String>,
}

impl RefSpec {
    /// Sets the `update_non_fastforward` flag
    #[inline]
    pub fn with_update_non_fastforward(mut self, update_non_fastforward: bool) -> Self {
        self.update_non_fastforward = update_non_fastforward;
        self
    }

    /// Sets the `source` refspec
    #[inline]
    pub fn with_source(mut self, source: Option<String>) -> Self {
        self.source = source;
        self
    }

    /// Sets the `destination` refspec
    #[inline]
    pub fn with_destination(mut self, destination: Option<String>) -> Self {
        self.destination = destination;
        self
    }

    /// Parses a refspec from a string.
    pub fn parse<S: AsRef<str>>(refspec: S) -> Result<Self, Error> {
        let s = refspec.as_ref();
        let mut refspec = Self::default();

        let mut offset = 0;

        let s = if let Some(stripped) = s.strip_prefix('+') {
            refspec.update_non_fastforward = true;
            offset += 1;
            stripped
        } else {
            s
        };

        let mut split = s.split(':');

        if let Some(first) = split.next() {
            offset += first.len();
            let first = first.trim();

            if !first.is_empty() {
                refspec.source = Some(first.to_owned());
            }
        }

        if let Some(second) = split.next() {
            offset += second.len() + 1;

            let second = second.trim();

            if second.is_empty() {
                refspec.destination = None;
            } else {
                refspec.destination = Some(second.to_owned());
            }
        } else {
            refspec.destination.clone_from(&refspec.source);
        }

        if split.next().is_some() {
            return Err(Error::UnexpectedChar(':', offset));
        }

        Ok(refspec)
    }
}

impl fmt::Display for RefSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.update_non_fastforward {
            f.write_str("+")?;
        }
        if let Some(source) = &self.source {
            f.write_str(source)?;
        }
        f.write_str(":")?;
        if let Some(destination) = &self.destination {
            f.write_str(destination)?;
        }
        Ok(())
    }
}

impl<S: AsRef<str>, D: AsRef<str>> From<(S, D)> for RefSpec {
    fn from((source, destination): (S, D)) -> Self {
        Self {
            source: Some(source.as_ref().to_owned()),
            destination: Some(destination.as_ref().to_owned()),
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_source_dest() {
        assert_eq!(
            RefSpec::parse("refs/heads/*:refs/remotes/origin/*").unwrap(),
            RefSpec {
                update_non_fastforward: false,
                source: Some("refs/heads/*".to_owned()),
                destination: Some("refs/remotes/origin/*".to_owned()),
            }
        );
    }

    #[test]
    fn test_parse_source_dest_force() {
        assert_eq!(
            RefSpec::parse("+refs/heads/*:refs/remotes/origin/*").unwrap(),
            RefSpec {
                update_non_fastforward: true,
                source: Some("refs/heads/*".to_owned()),
                destination: Some("refs/remotes/origin/*".to_owned()),
            }
        );
    }

    #[test]
    fn test_parse_invalid_third_refspec() {
        assert_eq!(
            RefSpec::parse("refs/heads/*:refs/remotes/origin/*:refs/remotes/upstream/*")
                .unwrap_err(),
            Error::UnexpectedChar(':', 34)
        );
    }

    #[test]
    fn test_parse_single_colon() {
        assert_eq!(
            RefSpec::parse(":").unwrap(),
            RefSpec {
                update_non_fastforward: false,
                source: None,
                destination: None,
            }
        );
    }

    #[test]
    fn test_parse_single_colon_force() {
        assert_eq!(
            RefSpec::parse("+:").unwrap(),
            RefSpec {
                update_non_fastforward: true,
                source: None,
                destination: None,
            }
        );
    }

    #[test]
    fn test_parse_empty() {
        assert_eq!(
            RefSpec::parse("").unwrap(),
            RefSpec {
                update_non_fastforward: false,
                source: None,
                destination: None,
            }
        );
    }

    #[test]
    fn test_parse_empty_force() {
        assert_eq!(
            RefSpec::parse("+").unwrap(),
            RefSpec {
                update_non_fastforward: true,
                source: None,
                destination: None,
            }
        );
    }

    #[test]
    fn test_parse_single() {
        assert_eq!(
            RefSpec::parse("refs/heads/*").unwrap(),
            RefSpec {
                update_non_fastforward: false,
                source: Some("refs/heads/*".to_owned()),
                destination: Some("refs/heads/*".to_owned()),
            }
        );
    }

    #[test]
    fn test_parse_delete() {
        assert_eq!(
            RefSpec::parse(":refs/heads/experimental").unwrap(),
            RefSpec {
                update_non_fastforward: false,
                source: None,
                destination: Some("refs/heads/experimental".to_owned()),
            }
        );
    }

    #[test]
    fn test_parse_single_force() {
        assert_eq!(
            RefSpec::parse("+refs/heads/*").unwrap(),
            RefSpec {
                update_non_fastforward: true,
                source: Some("refs/heads/*".to_owned()),
                destination: Some("refs/heads/*".to_owned()),
            }
        );
    }

    #[test]
    fn test_parse_delete_force() {
        assert_eq!(
            RefSpec::parse("+:refs/heads/experimental").unwrap(),
            RefSpec {
                update_non_fastforward: true,
                source: None,
                destination: Some("refs/heads/experimental".to_owned()),
            }
        );
    }

    #[test]
    fn test_parse_name() {
        assert_eq!(
            RefSpec::parse("master").unwrap(),
            RefSpec {
                update_non_fastforward: false,
                source: Some("master".to_owned()),
                destination: Some("master".to_owned()),
            }
        );
    }

    #[test]
    fn test_parse_name_force() {
        assert_eq!(
            RefSpec::parse("+master").unwrap(),
            RefSpec {
                update_non_fastforward: true,
                source: Some("master".to_owned()),
                destination: Some("master".to_owned()),
            }
        );
    }

    #[test]
    fn test_parse_source_only() {
        assert_eq!(
            RefSpec::parse("refs/heads/*:").unwrap(),
            RefSpec {
                update_non_fastforward: false,
                source: Some("refs/heads/*".to_owned()),
                destination: None,
            }
        );
    }

    #[test]
    fn format_empty() {
        assert_eq!(
            RefSpec {
                update_non_fastforward: false,
                source: None,
                destination: None,
            }
            .to_string(),
            ":".to_owned()
        );
    }

    #[test]
    fn format_empty_force() {
        assert_eq!(
            RefSpec {
                update_non_fastforward: true,
                source: None,
                destination: None,
            }
            .to_string(),
            "+:".to_owned()
        );
    }

    #[test]
    fn format_source_only() {
        assert_eq!(
            RefSpec {
                update_non_fastforward: false,
                source: Some("refs/heads/*".to_owned()),
                destination: None,
            }
            .to_string(),
            "refs/heads/*:".to_owned()
        );
    }

    #[test]
    fn format_source_only_force() {
        assert_eq!(
            RefSpec {
                update_non_fastforward: true,
                source: Some("refs/heads/*".to_owned()),
                destination: None,
            }
            .to_string(),
            "+refs/heads/*:".to_owned()
        );
    }

    #[test]
    fn format_source_dest() {
        assert_eq!(
            RefSpec {
                update_non_fastforward: false,
                source: Some("refs/heads/*".to_owned()),
                destination: Some("refs/remotes/origin/*".to_owned()),
            }
            .to_string(),
            "refs/heads/*:refs/remotes/origin/*".to_owned()
        );
    }

    #[test]
    fn format_source_dest_force() {
        assert_eq!(
            RefSpec {
                update_non_fastforward: true,
                source: Some("refs/heads/*".to_owned()),
                destination: Some("refs/remotes/origin/*".to_owned()),
            }
            .to_string(),
            "+refs/heads/*:refs/remotes/origin/*".to_owned()
        );
    }

    #[test]
    fn format_dest_only() {
        assert_eq!(
            RefSpec {
                update_non_fastforward: false,
                source: None,
                destination: Some("refs/heads/*".to_owned()),
            }
            .to_string(),
            ":refs/heads/*".to_owned()
        );
    }

    #[test]
    fn format_dest_only_force() {
        assert_eq!(
            RefSpec {
                update_non_fastforward: true,
                source: None,
                destination: Some("refs/heads/*".to_owned()),
            }
            .to_string(),
            "+:refs/heads/*".to_owned()
        );
    }

    #[test]
    fn test_tuple() {
        assert_eq!(
            RefSpec::from(("refs/heads/*", "refs/remotes/origin/*")),
            RefSpec {
                update_non_fastforward: false,
                source: Some("refs/heads/*".to_owned()),
                destination: Some("refs/remotes/origin/*".to_owned()),
            }
        );
    }
}
