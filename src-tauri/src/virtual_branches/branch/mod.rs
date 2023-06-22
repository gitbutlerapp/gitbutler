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
    pub fn normalize(&self) -> Ownership {
        let mut ranges = self.ranges.clone();
        ranges.sort_by(|a, b| a.start().cmp(b.start()));
        ranges.dedup();
        Ownership {
            file_path: self.file_path.clone(),
            ranges,
        }
    }

    // return a copy of self, with another ranges added
    pub fn plus(&self, another: &Ownership) -> Ownership {
        if !self.file_path.eq(&another.file_path) {
            return self.clone();
        }

        let mut ranges = self.ranges.clone();
        ranges.extend(another.ranges.clone());

        Ownership {
            file_path: self.file_path.clone(),
            ranges,
        }
        .normalize()
    }

    // returns a copy of self, with another ranges removed
    // if all of the ranges are removed, return None
    pub fn minus(&self, another: &Ownership) -> Option<Ownership> {
        if !self.contains(another) {
            return Some(self.clone());
        }

        let mut ranges = self.ranges.clone();
        for range in &another.ranges {
            ranges = ranges
                .iter()
                .flat_map(
                    |r: &ops::RangeInclusive<usize>| -> Vec<ops::RangeInclusive<usize>> {
                        if r.eq(range) {
                            vec![]
                        } else {
                            vec![r.clone()]
                        }
                    },
                )
                .collect();
        }

        if ranges.is_empty() {
            None
        } else {
            Some(Ownership {
                file_path: self.file_path.clone(),
                ranges,
            })
        }
    }

    fn contains(&self, another: &Ownership) -> bool {
        if self.file_path != another.file_path {
            return false;
        }

        if self.ranges.is_empty() {
            return true;
        }

        for range in &self.ranges {
            for another_range in &another.ranges {
                if range.contains(another_range.start()) || range.contains(another_range.end()) {
                    return true;
                }
            }
        }

        false
    }

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
        if start > end {
            Err(anyhow!("invalid range: {}", s))
        } else {
            Ok(start..=end)
        }
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
    fn parse_ownership_invalid_range_2() {
        let ownership = Ownership::parse_string("foo/bar.rs:1-2,6-5");
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

    #[test]
    fn test_normalize() {
        vec![
            ("file.txt:1-10", "file.txt:1-10"),
            ("file.txt:1-10,15-16", "file.txt:1-10,15-16"),
            ("file.txt:1-10,10-15,15-16", "file.txt:1-10,15-16"),
            ("file.txt:1-10,5-12", "file.txt:1-10,5-12"),
            ("file.txt:15-16,1-10", "file.txt:1-10,15-16"),
        ]
        .into_iter()
        .map(|(a, expected)| {
            (
                Ownership::parse_string(a).unwrap(),
                Ownership::parse_string(expected).unwrap(),
            )
        })
        .for_each(|(a, expected)| {
            let got = a.normalize();
            assert_eq!(
                got, expected,
                "normalize {} expected {}, got {}",
                a, expected, got
            );
        });
    }

    #[test]
    fn test_plus() {
        vec![
            ("file.txt:1-10", "another.txt:1-5", "file.txt:1-10"),
            ("file.txt:1-10", "file.txt:1-5", "file.txt:1-10,1-5"),
            ("file.txt:1-10", "file.txt:12-15", "file.txt:1-10,12-15"),
            (
                "file.txt:1-10",
                "file.txt:8-15,20-25",
                "file.txt:1-10,8-15,20-25",
            ),
            ("file.txt:1-10", "file.txt:10-15", "file.txt:1-10,10-15"),
            ("file.txt:5-10", "file.txt:1-5", "file.txt:1-5,5-10"),
            ("file.txt:1-10", "file.txt:1-10", "file.txt:1-10"),
            ("file.txt:5-10", "file.txt:2-7", "file.txt:2-7,5-10"),
            ("file.txt:5-10", "file.txt:7-12", "file.txt:5-10,7-12"),
        ]
        .into_iter()
        .map(|(a, b, expected)| {
            (
                Ownership::parse_string(a).unwrap(),
                Ownership::parse_string(b).unwrap(),
                Ownership::parse_string(expected).unwrap(),
            )
        })
        .for_each(|(a, b, expected)| {
            let got = a.plus(&b);
            assert_eq!(
                got, expected,
                "{} plus {}, expected {}, got {}",
                a, b, expected, got
            );
        });
    }

    #[test]
    fn test_minus() {
        vec![
            ("file.txt:1-10", "another.txt:1-5", Some("file.txt:1-10")),
            ("file.txt:1-10", "file.txt:1-5", Some("file.txt:1-10")),
            ("file.txt:1-10", "file.txt:11-15", Some("file.txt:1-10")),
            ("file.txt:1-10", "file.txt:1-10", None),
            (
                "file.txt:1-10,11-15",
                "file.txt:11-15",
                Some("file.txt:1-10"),
            ),
            (
                "file.txt:1-10,11-15,15-17",
                "file.txt:1-10,15-17",
                Some("file.txt:11-15"),
            ),
        ]
        .into_iter()
        .map(|(a, b, expected)| {
            (
                Ownership::parse_string(a).unwrap(),
                Ownership::parse_string(b).unwrap(),
                expected.map(|s| Ownership::parse_string(s).unwrap()),
            )
        })
        .for_each(|(a, b, expected)| {
            let got = a.minus(&b);
            assert_eq!(
                got, expected,
                "{} minus {}, expected {:?}, got {:?}",
                a, b, expected, got
            );
        });
    }

    #[test]
    fn test_contains() {
        vec![
            ("file.txt", "another.txt", false),
            ("file.txt", "file.txt", true),
            ("file.txt:1-10", "file.txt:11-20", false),
            ("file.txt:1-10", "file.txt:1-5", true),
            ("file.txt:1-10", "file.txt:5-10", true),
            ("file.txt:1-10", "file.txt:1-10", true),
            ("file.txt:1-10", "file.txt:2-7", true),
            ("file.txt:3-5", "file.txt:1-10", false),
            ("file.txt:1-10", "another.txt:1-10", false),
            ("file.txt:1-10", "file.txt:8-15,20-25", true),
        ]
        .into_iter()
        .map(|(a, b, expected)| {
            (
                Ownership::parse_string(a).unwrap(),
                Ownership::parse_string(b).unwrap(),
                expected,
            )
        })
        .for_each(|(a, b, expected)| {
            assert_eq!(
                a.contains(&b),
                expected,
                "{} contains {}, expected {}",
                a,
                b,
                expected
            );
        });
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
