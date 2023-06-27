use std::{fmt, path, vec};

use anyhow::{Context, Result};

use super::hunk::Hunk;

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Ownership {
    pub file_path: path::PathBuf,
    pub hunks: Vec<Hunk>,
}

impl TryFrom<&String> for Ownership {
    type Error = anyhow::Error;
    fn try_from(value: &String) -> std::result::Result<Self, Self::Error> {
        Self::parse_string(value)
    }
}

impl TryFrom<&str> for Ownership {
    type Error = anyhow::Error;
    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        Self::parse_string(value)
    }
}

impl TryFrom<String> for Ownership {
    type Error = anyhow::Error;
    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        Self::parse_string(&value)
    }
}

impl Ownership {
    pub fn normalize(&self) -> Ownership {
        let mut ranges = self.hunks.clone();
        ranges.sort_by(|a, b| a.start().cmp(b.start()));
        ranges.dedup();
        Ownership {
            file_path: self.file_path.clone(),
            hunks: ranges,
        }
    }

    // return a copy of self, with another ranges added
    pub fn plus(&self, another: &Ownership) -> Ownership {
        if !self.file_path.eq(&another.file_path) {
            return self.clone();
        }

        if self.hunks.is_empty() {
            // full ownership + partial ownership = full ownership
            return self.clone();
        }

        if another.hunks.is_empty() {
            // partial ownership + full ownership = full ownership
            return another.clone();
        }

        let mut ranges = self.hunks.clone();
        ranges.extend(another.hunks.clone());

        Ownership {
            file_path: self.file_path.clone(),
            hunks: ranges,
        }
        .normalize()
    }

    // returns a copy of self, with another ranges removed
    // if all of the ranges are removed, return None
    pub fn minus(&self, another: &Ownership) -> Option<Ownership> {
        if !self.file_path.eq(&another.file_path) {
            return Some(self.clone());
        }

        if another.hunks.is_empty() {
            // any ownership - full ownership = empty ownership
            return None;
        }

        if self.hunks.is_empty() {
            // full ownership - partial ownership = full ownership, since we don't know all the
            // hunks.
            return Some(self.clone());
        }

        let mut ranges = self.hunks.clone();
        for range in &another.hunks {
            ranges = ranges
                .iter()
                .flat_map(|r: &Hunk| -> Vec<Hunk> {
                    if r.eq(range) {
                        vec![]
                    } else {
                        vec![r.clone()]
                    }
                })
                .collect();
        }

        if ranges.is_empty() {
            None
        } else {
            Some(Ownership {
                file_path: self.file_path.clone(),
                hunks: ranges,
            })
        }
    }

    pub fn contains(&self, another: &Ownership) -> bool {
        if self.file_path != another.file_path {
            return false;
        }

        if self.hunks.is_empty() {
            // full ownership
            return true;
        }

        if another.hunks.is_empty() {
            // empty ownership
            return false;
        }

        another
            .hunks
            .iter()
            .map(|range| self.hunks.iter().find(|r| r.eq(&range)))
            .all(|x| x.is_some())
    }

    pub fn parse_string(s: &str) -> Result<Self> {
        let mut parts = s.split(':');
        let file_path = parts.next().unwrap();
        let ranges = match parts.next() {
            Some(raw_ranges) => raw_ranges
                .split(',')
                .map(Hunk::try_from)
                .collect::<Result<Vec<Hunk>>>(),
            None => Ok(vec![]),
        }
        .context(format!("failed to parse ownership ranges: {}", s))?;
        Ok(Self {
            file_path: path::PathBuf::from(file_path),
            hunks: ranges,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ownership() {
        let ownership = Ownership::parse_string("foo/bar.rs:1-2,4-5").unwrap();
        assert_eq!(
            ownership,
            Ownership {
                file_path: path::PathBuf::from("foo/bar.rs"),
                hunks: vec![(1..=2).into(), (4..=5).into()]
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
                hunks: vec![]
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
            hunks: vec![(1..=2).into(), (4..=5).into()],
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
            hunks: vec![],
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
            ("file.txt:1-10,10-15,15-16", "file.txt:1-10,10-15,15-16"),
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
            ("file.txt:1-10", "file.txt", "file.txt"),
            ("file.txt", "file.txt:1-10", "file.txt"),
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
            ("file.txt:1-10", "file.txt", None),
            ("file.txt", "file.txt", None),
            ("file.txt", "file.txt:1-10", Some("file.txt")),
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
            ("file.txt", "file.txt:1-10", true),
            ("file.txt:1-10", "file.txt", false),
            ("file.txt:1-10", "file.txt:11-20", false),
            ("file.txt:1-10", "file.txt:1-5", false),
            ("file.txt:1-10", "file.txt:5-10", false),
            ("file.txt:1-10", "file.txt:1-10", true),
            ("file.txt:1-10", "file.txt:2-7", false),
            ("file.txt:3-5", "file.txt:1-10", false),
            ("file.txt:1-10", "another.txt:1-10", false),
            ("file.txt:1-10", "file.txt:1-10,20-25", false),
            ("file.txt:1-10,11-15", "file.txt:11-15", true),
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
        if self.hunks.is_empty() {
            write!(f, "{}", self.file_path.to_str().unwrap())
        } else {
            write!(
                f,
                "{}:{}",
                self.file_path.to_str().unwrap(),
                self.hunks
                    .iter()
                    .map(|r| r.to_string())
                    .collect::<Vec<String>>()
                    .join(",")
            )
        }
    }
}
