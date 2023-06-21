mod reader;
mod writer;

pub use reader::BranchReader as Reader;
pub use writer::BranchWriter as Writer;

use std::{cmp, fmt, ops, path};

use anyhow::{anyhow, Context, Result};

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Ownership {
    pub file_path: path::PathBuf,
    pub ranges: Vec<ops::RangeInclusive<usize>>,
}

impl From<&String> for Ownership {
    fn from(value: &String) -> Self {
        Self {
            file_path: value.into(),
            ranges: vec![],
        }
    }
}

impl From<&str> for Ownership {
    fn from(value: &str) -> Self {
        Self {
            file_path: value.into(),
            ranges: vec![],
        }
    }
}

impl From<String> for Ownership {
    fn from(value: String) -> Self {
        Self {
            file_path: value.into(),
            ranges: vec![],
        }
    }
}

impl cmp::PartialOrd for Ownership {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.file_path.partial_cmp(&other.file_path)
    }
}

impl cmp::Ord for Ownership {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.file_path.cmp(&other.file_path)
    }
}

impl Ownership {
    fn parse_range(s: &str) -> Result<ops::RangeInclusive<usize>> {
        let mut range = s.split('-');
        if range.clone().count() != 2 {
            return Err(anyhow!("invalid range: {}", s));
        }
        let start = range
            .next()
            .unwrap()
            .parse::<usize>()
            .context(format!("failed to parse start of range: {}", s))?;
        let end = range
            .next()
            .unwrap()
            .parse::<usize>()
            .context(format!("failed to parse end of range: {}", s))?;
        Ok(start..=end)
    }

    pub fn parse_string(s: &str) -> Result<Self> {
        let mut parts = s.split(':');
        let file_path = parts.next().unwrap();
        let ranges = match parts.next() {
            Some(raw_ranges) => raw_ranges
                .split(',')
                .map(Self::parse_range)
                .collect::<Result<Vec<ops::RangeInclusive<usize>>>>(),
            None => Ok(vec![]),
        }
        .context(format!("failed to parse ownership ranges: {}", s))?;
        Ok(Self {
            file_path: path::PathBuf::from(file_path),
            ranges,
        })
    }
}

#[cfg(test)]
mod ownership_tests {
    use super::*;

    #[test]
    fn parse_ownership() {
        let ownership = Ownership::parse_string("foo/bar.rs:1-2,4-5").unwrap();
        assert_eq!(
            ownership,
            Ownership {
                file_path: path::PathBuf::from("foo/bar.rs"),
                ranges: vec![1..=2, 4..=5]
            }
        );
    }

    #[test]
    fn parse_ownership_no_ranges() {
        let ownership = Ownership::parse_string("foo/bar.rs").unwrap();
        assert_eq!(
            ownership,
            Ownership {
                file_path: path::PathBuf::from("foo/bar.rs"),
                ranges: vec![]
            }
        );
    }

    #[test]
    fn parse_ownership_invalid_range() {
        let ownership = Ownership::parse_string("foo/bar.rs:1-2,4-5-6");
        assert!(ownership.is_err());
    }

    #[test]
    fn ownership_to_from_string() {
        let ownership = Ownership {
            file_path: path::PathBuf::from("foo/bar.rs"),
            ranges: vec![1..=2, 4..=5],
        };
        assert_eq!(ownership.to_string(), "foo/bar.rs:1-2,4-5".to_string());
        assert_eq!(
            Ownership::parse_string(&ownership.to_string()).unwrap(),
            ownership
        );
    }

    #[test]
    fn ownership_to_from_string_no_ranges() {
        let ownership = Ownership {
            file_path: path::PathBuf::from("foo/bar.rs"),
            ranges: vec![],
        };
        assert_eq!(ownership.to_string(), "foo/bar.rs".to_string());
        assert_eq!(
            Ownership::parse_string(&ownership.to_string()).unwrap(),
            ownership
        );
    }
}

impl fmt::Display for Ownership {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        if self.ranges.is_empty() {
            write!(f, "{}", self.file_path.to_str().unwrap())
        } else {
            write!(
                f,
                "{}:{}",
                self.file_path.to_str().unwrap(),
                self.ranges
                    .iter()
                    .map(|r| format!("{}-{}", r.start(), r.end()))
                    .collect::<Vec<String>>()
                    .join(",")
            )
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Branch {
    pub id: String,
    pub name: String,
    pub applied: bool,
    pub upstream: String,
    pub created_timestamp_ms: u128,
    pub updated_timestamp_ms: u128,
    pub tree: git2::Oid, // last git tree written to a session, or merge base tree if this is new. use this for delta calculation from the session data
    pub head: git2::Oid,
    pub ownership: Vec<Ownership>,
}

impl TryFrom<&dyn crate::reader::Reader> for Branch {
    type Error = crate::reader::Error;

    fn try_from(reader: &dyn crate::reader::Reader) -> Result<Self, Self::Error> {
        let id = reader.read_string("id").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("id: {}", e),
            ))
        })?;
        let name = reader.read_string("meta/name").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/name: {}", e),
            ))
        })?;
        let applied = reader.read_bool("meta/applied").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/applied: {}", e),
            ))
        })?;
        let upstream = reader.read_string("meta/upstream").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/upstream: {}", e),
            ))
        })?;
        let tree = reader.read_string("meta/tree").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/tree: {}", e),
            ))
        })?;
        let head = reader.read_string("meta/head").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/head: {}", e),
            ))
        })?;
        let created_timestamp_ms = reader.read_u128("meta/created_timestamp_ms").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/created_timestamp_ms: {}", e),
            ))
        })?;
        let updated_timestamp_ms = reader.read_u128("meta/updated_timestamp_ms").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/updated_timestamp_ms: {}", e),
            ))
        })?;

        let ownership_string = reader.read_string("meta/ownership").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/ownership: {}", e),
            ))
        })?;
        let ownership = ownership_string
            .lines()
            .map(Ownership::parse_string)
            .collect::<Result<Vec<Ownership>>>()
            .map_err(|e| {
                crate::reader::Error::IOError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("meta/ownership: {}", e),
                ))
            })?;

        Ok(Self {
            id,
            name,
            applied,
            upstream,
            tree: git2::Oid::from_str(&tree).unwrap(),
            head: git2::Oid::from_str(&head).unwrap(),
            created_timestamp_ms,
            updated_timestamp_ms,
            ownership,
        })
    }
}
